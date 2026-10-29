//! Tauri commands of the `translation` module.

use tauri::{AppHandle, State};
use tauri_plugin_clipboard_manager::ClipboardExt;

use super::mode_detect;
use super::model::{Mode, TranslationRequest, TranslationResult};
use super::providers::ProviderStatus;
use super::providers::claude_code;
use super::providers::ollama::OllamaProvider;
use super::service::TranslationService;
use crate::error::{AppError, AppResult};
use crate::settings::SettingsStore;

/// Picks Translate or Correct for the popup; `last` is used when the text is too short to tell.
#[tauri::command]
pub fn detect_mode(text: String, last: Mode) -> Mode {
    mode_detect::detect_mode(&text, last)
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
pub fn copy_result(app: AppHandle, text: String, history_id: Option<i64>) -> AppResult<()> {
    if text.trim().is_empty() {
        return Err(AppError::InvalidInput("There is nothing to copy".into()));
    }
    app.clipboard()
        .write_text(text)
        .map_err(|e| AppError::Io(format!("Can't write to the clipboard: {e}")))?;
    // T28: mark the history entry `history_id` as accepted with target `clipboard`.
    let _ = history_id;
    Ok(())
}
