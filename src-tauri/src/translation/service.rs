//! `TranslationService`: runs one request through any provider (plan §4.1, T12).
//!
//! Flow of [`TranslationService::translate`]: validate the request → load the settings and the
//! preset → resolve the language pair (the request's, or the settings') → build the prompt → pick the provider by kind → run it under the provider's timeout
//! and the request's cancellation token → parse its answer → check technical tokens → result
//! with `duration_ms`.
//!
//! The service is built from plain parts (a [`SettingsStore`], a [`PresetStore`] and the
//! providers), so it works without a Tauri `AppHandle`: the app wraps it in managed state, and
//! the eval binary (T15) can build its own.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use super::language::LanguagePair;
use super::model::{ProviderKind, TranslationRequest, TranslationResult};
use super::prompt_builder::{self, BuiltPrompt};
use super::{output_parser, token_check};
use crate::error::{AppError, AppResult};
use crate::history::HistoryDb;
use crate::presets::{Preset, PresetStore};
use crate::settings::{Settings, SettingsStore};

/// The longest source text accepted, in characters. The prompt builder's context-budget
/// estimate (see `prompt_builder`) assumes this limit.
pub const MAX_SOURCE_CHARS: usize = 8000;

/// The raw answer of a provider, before parsing.
#[derive(Debug, Clone)]
pub enum RawOutput {
    /// A text answer (Ollama's `response`, the mock): parsed with [`output_parser::parse`].
    Text(String),
    /// An already-parsed JSON answer (Claude's structured output): parsed with
    /// [`output_parser::parse_value`].
    Json(Value),
}

/// What a provider returns for one prompt.
#[derive(Debug, Clone)]
pub struct ProviderOutput {
    pub raw: RawOutput,
    /// The model that answered, shown in the result and saved in history.
    pub model: String,
}

/// One way to run a prompt: Ollama, Claude Code or the mock.
#[async_trait]
pub trait TranslationProvider: Send + Sync {
    fn kind(&self) -> ProviderKind;

    /// How long the service waits for [`complete`](Self::complete) (the provider's timeout
    /// setting).
    fn timeout(&self, settings: &Settings) -> Duration;

    /// Runs the prompt. `settings` is the snapshot the service loaded for this request, so a
    /// provider reads its endpoint, model and so on from it rather than keeping its own copy.
    /// The provider should stop its work and return [`AppError::Cancelled`] when `cancel`
    /// fires; the token also fires when the service's timeout expires, so a provider can
    /// clean up (for example kill a child process) in both cases.
    async fn complete(
        &self,
        prompt: &BuiltPrompt,
        settings: &Settings,
        cancel: CancellationToken,
    ) -> AppResult<ProviderOutput>;
}

/// Runs translation requests; one instance lives in the app's managed state.
pub struct TranslationService {
    settings: SettingsStore,
    presets: PresetStore,
    providers: Vec<Arc<dyn TranslationProvider>>,
    /// `None` when the history database couldn't be opened: nothing is saved then.
    history: Option<HistoryDb>,
    /// Running requests by `request_id`. Every entry is removed when its request finishes.
    cancels: Mutex<HashMap<String, CancellationToken>>,
}

impl TranslationService {
    pub fn new(
        settings: SettingsStore,
        presets: PresetStore,
        providers: Vec<Arc<dyn TranslationProvider>>,
        history: Option<HistoryDb>,
    ) -> Self {
        Self {
            settings,
            presets,
            providers,
            history,
            cancels: Mutex::new(HashMap::new()),
        }
    }

    /// Runs one request. Errors: `InvalidInput` (empty or too long text, unknown preset,
    /// unavailable provider, duplicate `request_id`), `Cancelled`, `Timeout`, the parser's
    /// `EmptyOutput` / `MalformedOutput`, and whatever the provider returns.
    /// The request counts as made in the main window (the eval binary uses it).
    pub async fn translate(&self, req: TranslationRequest) -> AppResult<TranslationResult> {
        self.translate_from(req, RunOrigin::Main).await
    }

    /// Like [`translate`](Self::translate), for a request made in `origin` (which decides
    /// whether the result is saved to history).
    pub async fn translate_from(
        &self,
        req: TranslationRequest,
        origin: RunOrigin,
    ) -> AppResult<TranslationResult> {
        self.run(req, self.settings.get(), origin).await
    }

    /// Like [`translate`](Self::translate), but with the given settings instead of the saved
    /// ones (the setup benchmark runs a model that isn't the selected one yet).
    pub async fn translate_with(
        &self,
        req: TranslationRequest,
        settings: Settings,
    ) -> AppResult<TranslationResult> {
        self.run(req, settings, RunOrigin::Main).await
    }

    async fn run(
        &self,
        req: TranslationRequest,
        settings: Settings,
        origin: RunOrigin,
    ) -> AppResult<TranslationResult> {
        let started = Instant::now();
        validate(&req)?;
        // Registered first, so a cancel that arrives while the prompt is being prepared still
        // counts. The guard removes the token however this function ends.
        let registration = self.register(&req.request_id)?;
        let cancel = registration.token.clone();

        let preset = self.presets.get(&req.preset_id).ok_or_else(|| {
            AppError::InvalidInput(format!("The preset \"{}\" doesn't exist", req.preset_id))
        })?;
        let provider = self.provider(req.provider, &settings)?;
        let pair = LanguagePair::resolve(
            req.native_language.as_deref(),
            req.target_language.as_deref(),
            &settings,
        )?;
        let prompt = prompt_builder::build(
            req.mode,
            &preset,
            &req.source_text,
            pair,
            req.purpose.unwrap_or_default(),
        );
        let timeout = provider.timeout(&settings);

        let output = tokio::select! {
            biased;
            () = cancel.cancelled() => Err(AppError::Cancelled),
            answer = tokio::time::timeout(
                timeout,
                provider.complete(&prompt, &settings, cancel.clone()),
            ) => match answer {
                Ok(answer) => answer,
                Err(_elapsed) => {
                    // Tell the provider to clean up whatever it started.
                    cancel.cancel();
                    Err(AppError::Timeout)
                }
            },
        };
        let output = match output {
            Ok(output) => output,
            Err(e) => {
                log::info!(
                    "Translation via {:?} failed after {} ms: {:?}",
                    req.provider,
                    started.elapsed().as_millis(),
                    e.kind()
                );
                return Err(e);
            }
        };

        let parsed = match output.raw {
            RawOutput::Text(raw) => output_parser::parse(&raw, req.mode),
            RawOutput::Json(value) => output_parser::parse_value(value, req.mode),
        }?;
        let issues = output_parser::validate_issues(
            parsed.issues,
            &req.source_text,
            &parsed.output,
            req.mode,
        );
        let token_warnings = token_check::check_tokens(&req.source_text, &parsed.output);

        let mut result = TranslationResult {
            history_id: None,
            output: parsed.output,
            issues,
            token_warnings,
            provider: req.provider,
            model: output.model,
            duration_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        };
        if should_save(&settings, &preset, origin) {
            self.on_result(&req, pair, &mut result);
        }
        log::info!(
            "Translation ({:?}) via {:?} finished in {} ms",
            req.mode,
            req.provider,
            result.duration_ms
        );
        Ok(result)
    }

    /// Cancels a running request; does nothing when no request has this id (it already
    /// finished).
    pub fn cancel(&self, request_id: &str) {
        if let Some(token) = self.lock_cancels().get(request_id) {
            token.cancel();
        }
    }

    /// The number of running requests (their cancellation tokens in the registry).
    pub fn running_count(&self) -> usize {
        self.lock_cancels().len()
    }

    /// History hook (T28): called with a successful result that [`should_save`] allows, before
    /// it is returned.
    ///
    /// Saves the entry and sets `result.history_id`. A history failure must not fail the
    /// translation: log it and leave `history_id` as `None`. The entry stores the language pair
    /// the request used.
    fn on_result(
        &self,
        req: &TranslationRequest,
        pair: LanguagePair,
        result: &mut TranslationResult,
    ) {
        let Some(history) = &self.history else {
            return;
        };
        let saved = history
            .conn()
            .and_then(|conn| crate::history::repo::insert(&conn, result, req, pair));
        match saved {
            Ok(id) => result.history_id = Some(id),
            Err(e) => log::warn!("Couldn't save the translation to history: {e:?}"),
        }
    }

    /// The provider for `kind`, if it is registered and allowed.
    fn provider(
        &self,
        kind: ProviderKind,
        settings: &Settings,
    ) -> AppResult<Arc<dyn TranslationProvider>> {
        if kind == ProviderKind::Mock && !(cfg!(debug_assertions) || settings.dev_mock_provider) {
            return Err(AppError::InvalidInput(
                "The mock provider is only available in development builds".into(),
            ));
        }
        self.providers
            .iter()
            .find(|provider| provider.kind() == kind)
            .cloned()
            .ok_or_else(|| {
                AppError::InvalidInput(format!("The {} provider isn't available", name(kind)))
            })
    }

    fn register(&self, request_id: &str) -> AppResult<Registration<'_>> {
        let mut cancels = self.lock_cancels();
        if cancels.contains_key(request_id) {
            return Err(AppError::InvalidInput(
                "A request with this id is already running".into(),
            ));
        }
        let token = CancellationToken::new();
        cancels.insert(request_id.to_string(), token.clone());
        Ok(Registration {
            cancels: &self.cancels,
            request_id: request_id.to_string(),
            token,
        })
    }

    fn lock_cancels(&self) -> MutexGuard<'_, HashMap<String, CancellationToken>> {
        lock(&self.cancels)
    }
}

/// Where a request was made (T64): popup results are saved to history only when
/// `savePopupRuns` is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOrigin {
    /// The main window (and everything outside the app's windows, such as the eval binary).
    Main,
    /// The hotkey popup.
    Popup,
}

/// Whether a result is saved to history (HISTORY-007): history is on, the preset allows it, and
/// the run is from the main window or popup runs are saved too.
fn should_save(settings: &Settings, preset: &Preset, origin: RunOrigin) -> bool {
    settings.history_enabled
        && preset.save_to_history
        && (origin == RunOrigin::Main || settings.save_popup_runs)
}

/// A request's entry in the cancellation registry; dropping it removes the entry.
struct Registration<'a> {
    cancels: &'a Mutex<HashMap<String, CancellationToken>>,
    request_id: String,
    token: CancellationToken,
}

impl Drop for Registration<'_> {
    fn drop(&mut self) {
        lock(self.cancels).remove(&self.request_id);
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn validate(req: &TranslationRequest) -> AppResult<()> {
    if req.request_id.trim().is_empty() {
        return Err(AppError::InvalidInput("The request id is missing".into()));
    }
    if req.source_text.trim().is_empty() {
        return Err(AppError::InvalidInput("Enter some text first".into()));
    }
    if req.source_text.chars().count() > MAX_SOURCE_CHARS {
        return Err(AppError::InvalidInput(format!(
            "The text is too long: at most {MAX_SOURCE_CHARS} characters"
        )));
    }
    Ok(())
}

fn name(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Ollama => "Ollama",
        ProviderKind::ClaudeCode => "Claude Code",
        ProviderKind::Mock => "mock",
    }
}
