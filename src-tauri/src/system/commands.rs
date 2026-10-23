//! Tauri commands of the `system` module.

/// Round-trip check between the UI and the Rust core.
#[tauri::command]
pub fn ping() -> String {
    "pong".to_string()
}
