//! Update check and install via tauri-plugin-updater (plan T32).
//!
//! The endpoint, public key and `installMode` are in `tauri.conf.json` (`plugins.updater`). On
//! Windows the plugin runs the NSIS installer in passive mode and exits the app; the installer
//! relaunches the new version itself (`restart_after_install` defaults to true), so no process
//! plugin is needed. The plugin is registered from Rust only: the webview gets no updater
//! permission.

use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::settings::SettingsStore;

/// Emitted once at startup when a newer release exists (payload: [`UpdateInfo`]).
pub const AVAILABLE_EVENT: &str = "update://available";
/// Emitted while `install_update` downloads (payload: [`UpdateProgress`]).
pub const PROGRESS_EVENT: &str = "update://progress";
/// The startup check waits this long so it never competes with the app's own startup work.
const STARTUP_CHECK_DELAY: Duration = Duration::from_secs(10);
/// Progress events are sent at most this often (the final one always is).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);

/// Result of an update check. `version` and `notes` are set only when `available`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateInfo {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub version: Option<String>,
    /// The release notes (Markdown), if the release has any.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub notes: Option<String>,
}

/// Payload of `update://progress`: bytes downloaded so far, the total when the server sent it.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateProgress {
    #[ts(type = "number")]
    pub downloaded_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub total_bytes: Option<u64>,
    /// True on the last event: the download is done and the installer is starting.
    pub finished: bool,
}

fn update_error(e: tauri_plugin_updater::Error) -> AppError {
    AppError::Io(format!("Update failed: {e}"))
}

/// Asks the update endpoint for a newer release.
async fn check(app: &AppHandle) -> AppResult<Option<tauri_plugin_updater::Update>> {
    let updater = app.updater().map_err(update_error)?;
    updater.check().await.map_err(update_error)
}

fn to_info(update: Option<&tauri_plugin_updater::Update>) -> UpdateInfo {
    match update {
        Some(u) => UpdateInfo {
            available: true,
            version: Some(u.version.clone()),
            notes: u.body.clone().filter(|b| !b.trim().is_empty()),
        },
        None => UpdateInfo {
            available: false,
            version: None,
            notes: None,
        },
    }
}

/// Manual check: failures (offline, no release yet) are returned so the UI can show them.
#[tauri::command]
pub async fn check_update(app: AppHandle) -> AppResult<UpdateInfo> {
    Ok(to_info(check(&app).await?.as_ref()))
}

/// Downloads the newest release (progress as `update://progress`) and installs it. On success
/// the installer takes over and the app exits, so the call normally never returns.
#[tauri::command]
pub async fn install_update(app: AppHandle) -> AppResult<()> {
    let update = check(&app)
        .await?
        .ok_or_else(|| AppError::InvalidInput("You're already on the latest version".into()))?;

    let mut downloaded: u64 = 0;
    let mut last_sent: Option<Instant> = None;
    let progress_app = app.clone();
    let finish_app = app.clone();
    let result = update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                if last_sent.is_some_and(|t| t.elapsed() < PROGRESS_INTERVAL) {
                    return;
                }
                last_sent = Some(Instant::now());
                let _ = progress_app.emit(
                    PROGRESS_EVENT,
                    UpdateProgress {
                        downloaded_bytes: downloaded,
                        total_bytes: total,
                        finished: false,
                    },
                );
            },
            move || {
                let _ = finish_app.emit(
                    PROGRESS_EVENT,
                    UpdateProgress {
                        downloaded_bytes: 0,
                        total_bytes: None,
                        finished: true,
                    },
                );
            },
        )
        .await;
    result.map_err(update_error)
}

/// One silent check 10 s after startup, when `autoCheckUpdates` is on. A newer release is
/// announced with `update://available`; nothing is installed and errors are only logged.
pub fn spawn_startup_check(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(STARTUP_CHECK_DELAY).await;
        let enabled = app
            .try_state::<SettingsStore>()
            .is_some_and(|s| s.get().auto_check_updates);
        if !enabled {
            return;
        }
        match check(&app).await {
            Ok(Some(update)) => {
                log::info!("Update {} is available", update.version);
                let _ = app.emit(AVAILABLE_EVENT, to_info(Some(&update)));
            }
            Ok(None) => log::info!("No update available"),
            Err(e) => log::info!("Startup update check failed: {e}"),
        }
    });
}
