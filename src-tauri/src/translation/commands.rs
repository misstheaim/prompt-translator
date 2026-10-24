//! Tauri commands of the `translation` module.

use super::mode_detect;
use super::model::Mode;

/// Picks Translate or Correct for the popup; `last` is used when the text is too short to tell.
#[tauri::command]
pub fn detect_mode(text: String, last: Mode) -> Mode {
    mode_detect::detect_mode(&text, last)
}
