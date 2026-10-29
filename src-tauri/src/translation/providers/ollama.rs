//! `ollama` provider: runs a prompt on a local Ollama model through `POST /api/chat`
//! (plan T14), and reports Ollama's state through `GET /api/tags`.
//!
//! The request always carries `options.num_ctx` (Ollama's default context is smaller than the
//! prompt and silently drops the oldest tokens, i.e. the system prompt) and `think: false`
//! (thinking models such as Qwen 3.5 are 2-3x slower otherwise; other models ignore it). A prompt
//! whose estimated size doesn't fit in `num_ctx` is refused before it is sent.

use std::time::Duration;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::translation::model::{Mode, ProviderKind};
use crate::translation::prompt_builder::{BuiltPrompt, output_schema};
use crate::translation::service::{ProviderOutput, RawOutput, TranslationProvider};

const TEMPERATURE: f64 = 0.2;
const STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const ERROR_EXCERPT_CHARS: usize = 300;

/// Conservative characters per token: Latin text is about 4, Cyrillic about 2.5-3.
const SYSTEM_CHARS_PER_TOKEN: usize = 4;
const USER_CHARS_PER_TOKEN: usize = 3;

/// Ollama's state as shown in Settings and the setup wizard.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct OllamaStatus {
    pub reachable: bool,
    /// Names of the installed models (empty when Ollama is unreachable).
    pub models: Vec<String>,
    pub selected_model_installed: bool,
}

pub struct OllamaProvider {
    client: reqwest::Client,
}

impl OllamaProvider {
    pub fn new() -> Self {
        // No client-wide timeout: the service enforces the configured one on `complete`.
        Self {
            client: reqwest::Client::new(),
        }
    }

    /// `GET /api/tags` with a 2 s timeout. Never fails: any problem means "not reachable".
    pub async fn status(&self, settings: &Settings) -> OllamaStatus {
        let url = format!("{}/api/tags", endpoint(settings));
        let models = match self.fetch_models(&url).await {
            Ok(models) => models,
            Err(e) => {
                log::info!("Ollama status check failed: {e}");
                return OllamaStatus {
                    reachable: false,
                    models: Vec::new(),
                    selected_model_installed: false,
                };
            }
        };
        let selected_model_installed = models.iter().any(|m| same_model(m, &settings.ollama.model));
        OllamaStatus {
            reachable: true,
            models,
            selected_model_installed,
        }
    }

    async fn fetch_models(&self, url: &str) -> Result<Vec<String>, reqwest::Error> {
        let body: Value = self
            .client
            .get(url)
            .timeout(STATUS_TIMEOUT)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        Ok(body
            .get("models")
            .and_then(Value::as_array)
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| m.get("name").and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default())
    }
}

impl Default for OllamaProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TranslationProvider for OllamaProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Ollama
    }

    fn timeout(&self, settings: &Settings) -> Duration {
        Duration::from_secs(u64::from(settings.ollama.timeout_secs))
    }

    async fn complete(
        &self,
        prompt: &BuiltPrompt,
        settings: &Settings,
        cancel: CancellationToken,
    ) -> AppResult<ProviderOutput> {
        check_fits(prompt, settings.ollama.num_ctx)?;
        let model = settings.ollama.model.clone();
        let body = request_body(prompt, settings);
        let url = format!("{}/api/chat", endpoint(settings));

        let call = async {
            let response = self
                .client
                .post(&url)
                .json(&body)
                .send()
                .await
                .map_err(map_send_error)?;
            let status = response.status();
            let text = response.text().await.map_err(map_send_error)?;
            if !status.is_success() {
                return Err(map_http_error(status.as_u16(), &text, &model));
            }
            parse_chat_response(&text)
        };
        let content = tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(AppError::Cancelled),
            result = call => result?,
        };
        Ok(ProviderOutput {
            raw: RawOutput::Text(content),
            model,
        })
    }
}

fn endpoint(settings: &Settings) -> &str {
    settings.ollama.endpoint.trim().trim_end_matches('/')
}

/// The `/api/chat` request body.
fn request_body(prompt: &BuiltPrompt, settings: &Settings) -> Value {
    json!({
        "model": settings.ollama.model,
        "messages": [
            { "role": "system", "content": prompt.system },
            { "role": "user", "content": prompt.user },
        ],
        "stream": false,
        "think": false,
        "format": output_schema(prompt.mode),
        "keep_alive": settings.ollama.keep_alive,
        "options": {
            "temperature": TEMPERATURE,
            "num_ctx": settings.ollama.num_ctx,
        },
    })
}

/// Output tokens per character of the user message, by mode (measured in the T13 spike:
/// about 0.3 for Translate, 0.6 for Improve, 1.25 for Correct).
fn answer_tokens_per_char(mode: Mode) -> f64 {
    match mode {
        Mode::Translate => 0.3,
        Mode::Improve => 0.6,
        Mode::Correct => 1.25,
    }
}

/// Estimated tokens of the prompt plus the answer.
fn estimate_tokens(prompt: &BuiltPrompt) -> usize {
    let system = prompt.system.chars().count();
    let user = prompt.user.chars().count();
    let input = system / SYSTEM_CHARS_PER_TOKEN + user / USER_CHARS_PER_TOKEN;
    let answer = (user as f64 * answer_tokens_per_char(prompt.mode)).ceil() as usize;
    input + answer
}

/// Refuses a prompt that would overflow `num_ctx`: Ollama would silently truncate it.
fn check_fits(prompt: &BuiltPrompt, num_ctx: u32) -> AppResult<()> {
    let estimated = estimate_tokens(prompt);
    if estimated > num_ctx as usize {
        log::info!("Prompt refused: estimated {estimated} tokens, num_ctx {num_ctx}");
        return Err(AppError::InvalidInput(
            "The text is too long for the local model in this mode. Shorten it, split it into \
             parts, switch to Translate mode, or use Claude Code."
                .into(),
        ));
    }
    Ok(())
}

fn map_send_error(e: reqwest::Error) -> AppError {
    if e.is_connect() {
        AppError::OllamaUnreachable
    } else {
        AppError::Io(format!("Ollama request failed: {e}"))
    }
}

fn map_http_error(status: u16, body: &str, model: &str) -> AppError {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("error").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| body.trim().to_owned());
    let lowered = message.to_lowercase();
    if status == 404 || (lowered.contains("model") && lowered.contains("not found")) {
        return AppError::ModelMissing(model.to_owned());
    }
    let excerpt: String = message.chars().take(ERROR_EXCERPT_CHARS).collect();
    AppError::Io(format!("Ollama returned HTTP {status}: {excerpt}"))
}

fn parse_chat_response(text: &str) -> AppResult<String> {
    let value: Value = serde_json::from_str(text)
        .map_err(|_| AppError::MalformedOutput(text.chars().take(2000).collect()))?;
    value
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| AppError::MalformedOutput(text.chars().take(2000).collect()))
}

/// `qwen3.5:9b` matches itself; a name without a tag means `:latest`.
fn same_model(installed: &str, wanted: &str) -> bool {
    fn norm(name: &str) -> String {
        let name = name.trim();
        if name.contains(':') {
            name.to_owned()
        } else {
            format!("{name}:latest")
        }
    }
    norm(installed) == norm(wanted)
}
