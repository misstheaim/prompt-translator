//! Tauri commands of the `system` module.

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::error::{AppError, AppResult};

/// Round-trip check between the UI and the Rust core.
#[tauri::command]
pub fn ping() -> String {
    "pong".to_string()
}

/// The current hotkey registration error, if any (the main window asks once at startup; later
/// changes arrive as `hotkey://error`).
#[tauri::command]
pub fn hotkey_error(state: tauri::State<'_, super::hotkey::HotkeyState>) -> Option<String> {
    state.error()
}

/// The only URLs `open_url` opens: the Ollama download page, the Claude Code install docs and
/// this repo's releases (plan §4.6). A URL matches when it equals an entry or continues it with
/// a `/`, `?` or `#`.
const ALLOWED_URLS: &[&str] = &[
    "https://ollama.com/download",
    "https://docs.claude.com/en/docs/claude-code/setup",
    "https://github.com/misstheaim/prompt-translator/releases",
];

fn is_allowed_url(url: &str) -> bool {
    ALLOWED_URLS.iter().any(|allowed| {
        url.strip_prefix(allowed)
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(['/', '?', '#']))
    })
}

/// Opens an allow-listed URL in the default browser; anything else is `InvalidInput`.
#[tauri::command]
pub fn open_url(app: AppHandle, url: String) -> AppResult<()> {
    if !is_allowed_url(&url) {
        return Err(AppError::InvalidInput(
            "This link can't be opened from the app".into(),
        ));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::Io(format!("Can't open the link: {e}")))
}

/// Native file picker (Settings, "Browse" for the claude.exe path). `filter` lists the allowed
/// extensions without dots, e.g. `["exe", "cmd"]`; `None` allows any file. Returns `None` when
/// the user cancels.
#[tauri::command]
pub async fn pick_file(app: AppHandle, filter: Option<Vec<String>>) -> AppResult<Option<String>> {
    let picked = tokio::task::spawn_blocking(move || {
        let mut dialog = app.dialog().file();
        if let Some(exts) = filter.as_ref().filter(|e| !e.is_empty()) {
            let exts: Vec<&str> = exts.iter().map(String::as_str).collect();
            dialog = dialog.add_filter("Files", &exts);
        }
        dialog.blocking_pick_file()
    })
    .await
    .map_err(|e| AppError::Io(format!("The file dialog failed: {e}")))?;
    picked
        .map(|p| {
            p.into_path()
                .map(|p| p.to_string_lossy().into_owned())
                .map_err(|e| AppError::Io(format!("Can't read the chosen path: {e}")))
        })
        .transpose()
}
