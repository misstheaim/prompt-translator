//! `system::replace`: Enter in the popup replaces the selection in the source app (POPUP-013).
//!
//! At capture the hotkey keeps the source window ([`SourceWindow`]) when the text came from an
//! editable selection. [`replace_selection`] then hides the popup, brings that window back to the
//! front, puts the result on the clipboard (hidden from clipboard history), sends Ctrl+V and puts
//! the previous clipboard text back after [`PASTE_DELAY`] (only if the clipboard didn't change
//! meanwhile, as in the layout fix). Any failure falls back to a plain copy plus a notification.

use std::sync::Mutex;

use tauri::{AppHandle, Manager, Runtime};

/// The window the popup's text was selected in, kept for "Enter replaces". `None` when replacing
/// isn't possible (read-only, unknown or clipboard source).
#[derive(Default)]
pub struct SourceWindow(Mutex<Option<usize>>);

impl SourceWindow {
    pub fn set(&self, window: Option<usize>) {
        *self.0.lock().unwrap_or_else(|e| e.into_inner()) = window;
    }

    fn get(&self) -> Option<usize> {
        *self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Notification text when the paste failed and the result was copied instead.
pub const FALLBACK_HINT: &str = "Couldn't paste: the result was copied instead";

/// Why a replace didn't happen (logged; the caller falls back to copying).
#[derive(Debug)]
pub enum Failure {
    /// No editable source window was recorded.
    NoSource,
    /// The window is gone, or didn't come to the front.
    Window,
    /// Modifier keys stayed down, or the input was blocked.
    Input,
    /// The clipboard couldn't be read or written.
    Clipboard,
}

/// Pastes `text` over the selection in the recorded source window. Blocking.
pub fn replace_selection<R: Runtime>(app: &AppHandle<R>, text: &str) -> Result<(), Failure> {
    let window = app
        .try_state::<SourceWindow>()
        .and_then(|state| state.get())
        .ok_or(Failure::NoSource)?;
    replace_in(window, text)
}

#[cfg(windows)]
fn replace_in(window: usize, text: &str) -> Result<(), Failure> {
    use std::thread::sleep;
    use std::time::Duration;

    use super::input::{self, Window};
    use crate::layout_fix::hotkey::PASTE_DELAY;

    const FOCUS_TIMEOUT: Duration = Duration::from_millis(1000);

    let _guard = input::lock();
    let window = Window::from_id(window);
    if !window.exists() || !window.is_visible() {
        return Err(Failure::Window);
    }
    if window.is_minimised() {
        window.restore();
    }
    window.set_foreground();
    if !input::wait_until(FOCUS_TIMEOUT, || window.is_foreground()) {
        return Err(Failure::Window);
    }
    // The Enter that accepted the result may still be down for a moment.
    input::wait_until(input::MODIFIER_RELEASE_TIMEOUT, || !input::enter_down());
    if !input::wait_for_modifiers_released(input::MODIFIER_RELEASE_TIMEOUT) {
        return Err(Failure::Input);
    }

    let saved = input::read_text().map_err(|_| Failure::Clipboard)?;
    if !input::write_text(Some(text), true) {
        return Err(Failure::Clipboard);
    }
    let written = input::sequence_number();
    if !window.is_foreground() || input::any_modifier_down() {
        restore(saved.as_deref(), written);
        return Err(Failure::Window);
    }
    if !input::send_ctrl_v() {
        restore(saved.as_deref(), written);
        return Err(Failure::Input);
    }
    sleep(PASTE_DELAY);
    restore(saved.as_deref(), written);
    Ok(())
}

/// Puts `saved` back when the clipboard still holds what we wrote (sequence `written`).
#[cfg(windows)]
fn restore(saved: Option<&str>, written: u32) {
    use super::input;

    if input::sequence_number() == written && !input::write_text(saved, true) {
        log::warn!("Replace: could not restore the previous clipboard text");
    }
}

#[cfg(not(windows))]
fn replace_in(_window: usize, _text: &str) -> Result<(), Failure> {
    Err(Failure::NoSource)
}
