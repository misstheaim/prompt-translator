//! `system::clipboard`: auto-copy of the selection for the popup hotkey (plan D5, T25b).
//!
//! With `autoCopySelection` on, [`copy_selection`] copies what is selected in the foreground app
//! so "select + hotkey" works without a manual Ctrl+C:
//! 1. Terminals (see [`input::is_terminal`]) are skipped: Ctrl+C there interrupts the running
//!    command.
//! 2. It waits until every modifier key is released (at most
//!    [`input::MODIFIER_RELEASE_TIMEOUT`]); if they are still held it gives up. Ctrl+C is never
//!    sent while Shift is down, because Ctrl+Shift+C opens DevTools in Chromium apps.
//! 3. It saves the clipboard text, sends Ctrl+C with `SendInput` and waits (at most
//!    [`input::COPY_TIMEOUT`]) for the clipboard sequence number to change.
//! 4. It reads the new text, then puts the saved text back, marked so that Windows clipboard
//!    history and clipboard monitors ignore the restore.
//!
//! The caller checks the setting. `None` means "use the clipboard as it is": a terminal,
//! modifiers held, the input was blocked, or nothing was selected.
//! IDE-integrated terminals can't be told apart from the editor; Ctrl+C with a selection copies
//! there, which is acceptable.
//!
//! The Win32 helpers live in [`super::input`], shared with the layout fix.

#[cfg(windows)]
use std::thread::sleep;

use super::input;

/// Copies the foreground app's selection (see the module docs). `Some` only when Ctrl+C put
/// non-empty text on the clipboard; the clipboard then holds its previous text again.
pub fn copy_selection() -> Option<String> {
    #[cfg(windows)]
    {
        copy_selection_win()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(windows)]
fn copy_selection_win() -> Option<String> {
    let _guard = input::lock();

    let window = input::foreground_window()?;
    if window.is_terminal() {
        log::debug!(
            "Auto-copy skipped: the foreground window is a terminal ({})",
            window.class_name()
        );
        return None;
    }

    if !input::wait_for_modifiers_released(input::MODIFIER_RELEASE_TIMEOUT) {
        log::info!("Auto-copy skipped: modifier keys still held; using the clipboard");
        return None;
    }

    // Save before copying; without a readable clipboard nothing could be restored.
    let saved = match input::read_text() {
        Ok(saved) => saved,
        Err(input::ClipboardBusy) => {
            log::warn!("Auto-copy skipped: the clipboard is busy");
            return None;
        }
    };
    let before = input::sequence_number();

    // While we waited, a modifier may have been pressed again or another window (possibly a
    // terminal) may have come to the front; the classification above is only for `window`.
    if !window.is_foreground() || input::any_modifier_down() {
        log::debug!("Auto-copy skipped: focus or modifiers changed before Ctrl+C");
        return None;
    }
    if !input::send_ctrl_c() {
        return None;
    }
    if !input::wait_for_sequence_change(before, input::COPY_TIMEOUT) {
        log::debug!("Auto-copy: nothing was copied (no selection); using the clipboard");
        return None;
    }
    sleep(input::SETTLE_TIME);

    let copied = input::read_text().ok().flatten();
    if !input::write_text(saved.as_deref(), true) {
        log::warn!("Auto-copy: could not restore the previous clipboard text");
    }
    copied.filter(|text| !text.is_empty())
}
