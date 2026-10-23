//! Tauri commands of the `presets` module.

use tauri::State;

use super::{Preset, PresetStore};
use crate::error::AppResult;

#[tauri::command]
pub fn list_presets(store: State<'_, PresetStore>) -> Vec<Preset> {
    store.list()
}

/// Creates or updates a preset; returns all presets.
#[tauri::command]
pub fn save_preset(store: State<'_, PresetStore>, preset: Preset) -> AppResult<Vec<Preset>> {
    store.save(preset)
}

/// Deletes a preset (never the last one); returns all presets.
#[tauri::command]
pub fn delete_preset(store: State<'_, PresetStore>, id: String) -> AppResult<Vec<Preset>> {
    store.delete(&id)
}

/// Restores the built-in presets, keeping user-created ones; returns all presets.
#[tauri::command]
pub fn reset_presets(store: State<'_, PresetStore>) -> AppResult<Vec<Preset>> {
    store.reset()
}
