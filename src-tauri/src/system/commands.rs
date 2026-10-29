//! Tauri commands of the `system` module.

use tauri::AppHandle;
use tauri_plugin_dialog::DialogExt;

use crate::error::{AppError, AppResult};

/// Round-trip check between the UI and the Rust core.
#[tauri::command]
pub fn ping() -> String {
    "pong".to_string()
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
