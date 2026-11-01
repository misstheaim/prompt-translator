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

/// Minimum inner size of the main window in normal mode (TRANS-015). Keep in step with
/// `minWidth`/`minHeight` in `tauri.conf.json`, which apply from startup.
const NORMAL_MIN_SIZE: (f64, f64) = (640.0, 480.0);
/// Minimum inner size in compact mode.
const COMPACT_MIN_SIZE: (f64, f64) = (320.0, 200.0);
/// Inner size the button gives the window when it turns compact mode on.
const COMPACT_SIZE: (f64, f64) = (420.0, 300.0);

/// The logical size the window had before the compact button resized it.
static REMEMBERED_SIZE: std::sync::Mutex<Option<(f64, f64)>> = std::sync::Mutex::new(None);

/// Switches the main window between its normal and compact limits (TRANS-015).
///
/// With `resize` (the compact button) turning compact on remembers the current size and shrinks
/// the window to the compact size, and turning it off restores the remembered size. Without
/// `resize` (the window was dragged to or from its minimum height) only the minimum size changes.
/// Leaving compact mode always makes sure the window is at least the normal minimum.
#[tauri::command]
pub fn set_compact_window(app: AppHandle, compact: bool, resize: bool) -> AppResult<()> {
    use tauri::LogicalSize;

    let window = app
        .get_webview_window("main")
        .ok_or_else(|| AppError::Io("The main window isn't available".into()))?;
    let fail = |e: tauri::Error| AppError::Io(format!("Can't resize the window: {e}"));
    let current = || -> AppResult<(f64, f64)> {
        let size = window
            .inner_size()
            .map_err(fail)?
            .to_logical::<f64>(window.scale_factor().map_err(fail)?);
        Ok((size.width, size.height))
    };
    let mut remembered = REMEMBERED_SIZE.lock().unwrap_or_else(|e| e.into_inner());

    if compact {
        if resize {
            if window.is_maximized().unwrap_or(false) {
                window.unmaximize().map_err(fail)?;
            }
            *remembered = Some(current()?);
        }
        window
            .set_min_size(Some(LogicalSize::new(
                COMPACT_MIN_SIZE.0,
                COMPACT_MIN_SIZE.1,
            )))
            .map_err(fail)?;
        if resize {
            window
                .set_size(LogicalSize::new(COMPACT_SIZE.0, COMPACT_SIZE.1))
                .map_err(fail)?;
        }
    } else {
        window
            .set_min_size(Some(LogicalSize::new(NORMAL_MIN_SIZE.0, NORMAL_MIN_SIZE.1)))
            .map_err(fail)?;
        let (w, h) = remembered
            .take()
            .unwrap_or_else(|| current().unwrap_or(NORMAL_MIN_SIZE));
        let (cw, ch) = current()?;
        let target = if resize {
            (w.max(NORMAL_MIN_SIZE.0), h.max(NORMAL_MIN_SIZE.1))
        } else {
            (cw.max(NORMAL_MIN_SIZE.0), ch.max(NORMAL_MIN_SIZE.1))
        };
        if (target.0 - cw).abs() > 0.5 || (target.1 - ch).abs() > 0.5 {
            window
                .set_size(LogicalSize::new(target.0, target.1))
                .map_err(fail)?;
        }
    }
    Ok(())
}

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

/// Popup Enter (POPUP-013): hides the popup and pastes `text` over the selection in the source
/// window, then marks the history entry accepted. If pasting isn't possible the text is copied
/// instead and a notification says so; only a failed copy is an error (the popup is shown again).
#[tauri::command]
pub async fn replace_selection(
    app: AppHandle,
    text: String,
    history_id: Option<i64>,
) -> AppResult<()> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    use tauri_plugin_notification::NotificationExt;

    if text.trim().is_empty() {
        return Err(AppError::InvalidInput("There is nothing to paste".into()));
    }
    super::popup::hide(&app);
    let worker = app.clone();
    let worker_text = text.clone();
    let pasted = tokio::task::spawn_blocking(move || {
        super::replace::replace_selection(&worker, &worker_text)
    })
    .await
    .map_err(|e| AppError::Io(format!("The paste failed: {e}")))?;

    if let Err(failure) = pasted {
        log::info!("Replace failed ({failure:?}); copying instead");
        if let Err(e) = app.clipboard().write_text(text) {
            super::popup::show_again(&app);
            return Err(AppError::Io(format!("Can't write to the clipboard: {e}")));
        }
        let title = app
            .config()
            .product_name
            .clone()
            .unwrap_or_else(|| "Prompt Translator".into());
        if let Err(e) = app
            .notification()
            .builder()
            .title(title)
            .body(super::replace::FALLBACK_HINT)
            .show()
        {
            log::warn!("Could not show a notification: {e}");
        }
    }
    if let Some(id) = history_id {
        crate::history::commands::mark_accepted(
            &app,
            id,
            crate::history::AcceptedTarget::Clipboard,
        )
        .await;
    }
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
