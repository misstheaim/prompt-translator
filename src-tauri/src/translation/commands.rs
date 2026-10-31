//! Tauri commands of the `translation` module.

use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::language::{self, Language, LanguagePair};
use super::mode_detect;
use super::model::{Mode, TranslationRequest, TranslationResult};
use super::providers::ProviderStatus;
use super::providers::claude_code;
use super::providers::ollama::OllamaProvider;
use super::service::TranslationService;
use crate::error::{AppError, AppResult};
use crate::history::{self, AcceptedTarget};
use crate::settings::SettingsStore;

/// Picks Translate or Correct for the popup; `last` is used when the text is too short to tell.
/// The language pair is the given one, each missing code taken from the settings (as in
/// `translate`).
#[tauri::command]
pub fn detect_mode(
    settings: State<'_, SettingsStore>,
    text: String,
    last: Mode,
    native_language: Option<String>,
    target_language: Option<String>,
) -> AppResult<Mode> {
    let pair = LanguagePair::resolve(
        native_language.as_deref(),
        target_language.as_deref(),
        &settings.get(),
    )?;
    Ok(mode_detect::detect_mode(&text, last, pair))
}

/// The languages that can be chosen as native or target language, sorted by English name.
#[tauri::command]
pub fn list_languages() -> Vec<Language> {
    language::list()
}

/// Runs one request through its provider; cancel it with `cancel_translation(req.requestId)`.
#[tauri::command]
pub async fn translate(
    service: State<'_, TranslationService>,
    req: TranslationRequest,
) -> AppResult<TranslationResult> {
    service.translate(req).await
}

/// Cancels a running request (no-op when it already finished); it then fails with `Cancelled`.
#[tauri::command]
pub fn cancel_translation(service: State<'_, TranslationService>, request_id: String) {
    service.cancel(&request_id);
}

/// Ollama's and Claude Code's state for the current settings.
#[tauri::command]
pub async fn provider_status(settings: State<'_, SettingsStore>) -> AppResult<ProviderStatus> {
    let settings = settings.get();
    Ok(ProviderStatus {
        ollama: OllamaProvider::new().status(&settings).await,
        claude: Some(claude_code::status(&settings).await),
    })
}

/// Accept with the Clipboard target: writes the text to the clipboard.
#[tauri::command]
pub async fn copy_result(app: AppHandle, text: String, history_id: Option<i64>) -> AppResult<()> {
    if text.trim().is_empty() {
        return Err(AppError::InvalidInput("There is nothing to copy".into()));
    }
    app.clipboard()
        .write_text(text)
        .map_err(|e| AppError::Io(format!("Can't write to the clipboard: {e}")))?;
    if let Some(id) = history_id {
        // A history problem never fails the copy.
        history::commands::mark_accepted(&app, id, AcceptedTarget::Clipboard).await;
    }
    Ok(())
}
