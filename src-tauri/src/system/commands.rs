//! Tauri commands of the `system` module.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use ts_rs::TS;

use super::{paths, uninstall};
use crate::error::{AppError, AppResult};
use crate::settings::SettingsStore;
use crate::translation::providers::ollama::OllamaProvider;

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

/// Version and the two app folders (Settings, About tab).
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AppInfo {
    pub version: String,
    /// Settings and presets.
    pub config_dir: String,
    /// History, logs and temp files.
    pub data_dir: String,
}

#[tauri::command]
pub fn app_info(app: AppHandle) -> AppResult<AppInfo> {
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        config_dir: paths::config_dir()?.to_string_lossy().into_owned(),
        data_dir: paths::local_dir()?.to_string_lossy().into_owned(),
    })
}

/// Which app folder `open_folder` opens.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppFolder {
    Config,
    Data,
}

/// Opens the config or the data folder in Explorer.
#[tauri::command]
pub fn open_folder(app: AppHandle, folder: AppFolder) -> AppResult<()> {
    let dir = match folder {
        AppFolder::Config => paths::config_dir()?,
        AppFolder::Data => paths::local_dir()?,
    };
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::Io(format!("Can't open the folder: {e}")))
}

const UNLOAD_TIMEOUT: Duration = Duration::from_secs(3);
const UNLOAD_ON_EXIT_TIMEOUT: Duration = Duration::from_secs(2);

/// Asks Ollama to unload the configured model. `true` when it was unloaded, `false` when nothing
/// was loaded or Ollama isn't running (not an error).
#[tauri::command]
pub async fn unload_model(settings: tauri::State<'_, SettingsStore>) -> AppResult<bool> {
    OllamaProvider::new()
        .unload(&settings.get(), UNLOAD_TIMEOUT)
        .await
}

/// Best-effort unload for the app's exit path: at most about 2 s, never fails or panics.
pub fn unload_model_on_exit(app: &AppHandle) {
    let Some(store) = app.try_state::<SettingsStore>() else {
        return;
    };
    let settings = store.get();
    let result = tauri::async_runtime::block_on(async {
        tokio::time::timeout(
            UNLOAD_ON_EXIT_TIMEOUT,
            OllamaProvider::new().unload(&settings, UNLOAD_ON_EXIT_TIMEOUT),
        )
        .await
    });
    match result {
        Ok(Ok(true)) => log::info!("Unloaded the model on exit"),
        Ok(Ok(false)) => {}
        Ok(Err(e)) => log::info!("Couldn't unload the model on exit: {e}"),
        Err(_) => log::info!("Unloading the model on exit timed out"),
    }
}

/// Whether Settings, About can offer "Uninstall" (an installed build has an uninstaller).
#[tauri::command]
pub fn uninstaller_available() -> bool {
    uninstall::uninstaller_path().is_some()
}

/// Starts the uninstaller (detached) and exits the app.
#[tauri::command]
pub fn run_uninstaller(app: AppHandle) -> AppResult<()> {
    uninstall::run_uninstaller()?;
    app.exit(0);
    Ok(())
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
