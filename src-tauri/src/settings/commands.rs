//! Tauri commands of the `settings` module.

use tauri::{AppHandle, Emitter, State};

use super::{SETTINGS_CHANGED_EVENT, Settings, SettingsStore};
use crate::error::AppResult;
use crate::translation::model::Mode;

#[tauri::command]
pub fn get_settings(store: State<'_, SettingsStore>) -> Settings {
    store.get()
}

/// Saves the mode the popup ended on to `lastPopupMode`.
#[tauri::command]
pub fn set_last_popup_mode(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    mode: Mode,
) -> AppResult<Settings> {
    let mut settings = store.get();
    settings.last_popup_mode = mode;
    let saved = store.save(settings)?;
    app.emit(SETTINGS_CHANGED_EVENT, &saved)?;
    Ok(saved)
}

/// Validates and saves, then emits `settings://changed` with the saved settings.
#[tauri::command]
pub fn save_settings(
    app: AppHandle,
    store: State<'_, SettingsStore>,
    settings: Settings,
) -> AppResult<Settings> {
    let saved = store.save(settings)?;
    app.emit(SETTINGS_CHANGED_EVENT, &saved)?;
    Ok(saved)
}
