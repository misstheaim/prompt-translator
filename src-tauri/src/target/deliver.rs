//! `target::deliver` (T39, plan D47): puts accepted text into the input of a bound terminal
//! window, following the recipe of spike T58 (`docs/spikes/session-paste.md`, "Recipe for the
//! app").
//!
//! The terminal is brought to the front (restored if minimised), its bound Windows Terminal tab
//! is selected through UI Automation, and the text is pasted once: Ctrl+V for Windows Terminal,
//! the console's own Edit > Paste (`WM_COMMAND 0xFFF1`) for a classic console, which takes
//! single-line text only. The clipboard text from before is put back afterwards. **Enter is never
//! sent**: the user presses it in Claude.
//!
//! Safety checks, each one a refusal (the caller then copies the text instead):
//! - the window still exists, has the bound pid and host class, and isn't a translator run's;
//! - no modifier key or Enter is held, and the keyboard has been quiet for [`QUIET_INPUT`], so
//!   typing in flight can't mix into Claude's input;
//! - the window really is in front and the bound tab selected right before the paste.
//!
//! Delivery must follow the user's own action in the app (their Accept click or Enter): then
//! plain `SetForegroundWindow` is allowed. From anywhere else Windows refuses and the caller
//! falls back to the clipboard.

#[cfg(windows)]
use std::time::Duration;

use super::TerminalTarget;
use crate::translation::providers::claude_code::TranslatorPids;

/// How long the keys of the user's Accept (Enter, a modifier) may stay down.
#[cfg(windows)]
const KEY_RELEASE_TIMEOUT: Duration = Duration::from_millis(500);
/// How long a minimised window may take to come back.
#[cfg(windows)]
const RESTORE_TIMEOUT: Duration = Duration::from_millis(1000);
/// How long `SetForegroundWindow` may take to show in `GetForegroundWindow`.
#[cfg(windows)]
const FOREGROUND_TIMEOUT: Duration = Duration::from_millis(500);
/// How long the bound tab may take to report itself selected.
#[cfg(windows)]
const SELECT_TIMEOUT: Duration = Duration::from_millis(500);
/// Pause after focus and tab switch so the terminal routes keyboard input to the new tab.
#[cfg(windows)]
const SETTLE: Duration = Duration::from_millis(100);
/// No keyboard or mouse input for this long before the paste.
#[cfg(windows)]
const QUIET_INPUT: Duration = Duration::from_millis(300);
/// How long to wait for the keyboard to become quiet.
#[cfg(windows)]
const QUIET_TIMEOUT: Duration = Duration::from_millis(1500);
/// Time for the terminal to read the clipboard after the paste (the layout fix's delay).
#[cfg(windows)]
const PASTE_DELAY: Duration = Duration::from_millis(200);

/// Whether the text (after trimming) starts with `!`, `/` or `#`: in Claude Code's input these
/// run a shell command, a slash command or write a memory note (D35), so the user confirms
/// first.
pub fn needs_confirmation(text: &str) -> bool {
    text.trim_start().starts_with(['!', '/', '#'])
}

/// The text as it is pasted: line breaks as `\n`, trailing white space removed (a paste ending
/// in a line break could act as Enter in a terminal without bracketed paste).
fn normalise(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end()
        .to_string()
}

/// Pastes `text` into the target's input without sending it. `Err` holds the reason, worded for
/// the user, when nothing was pasted.
pub fn paste(
    translators: &TranslatorPids,
    target: &TerminalTarget,
    text: &str,
) -> Result<(), String> {
    let text = normalise(text);
    if text.is_empty() {
        return Err("There is no text to paste".into());
    }
    imp::paste(translators, target, &text)
}

#[cfg(windows)]
mod imp {
    use std::thread::sleep;

    use super::{
        FOREGROUND_TIMEOUT, KEY_RELEASE_TIMEOUT, PASTE_DELAY, QUIET_INPUT, QUIET_TIMEOUT,
        RESTORE_TIMEOUT, SELECT_TIMEOUT, SETTLE,
    };
    use crate::error::AppError;
    use crate::system::input::{self, Window};
    use crate::system::uia::{Tab, Uia};
    use crate::target::sessions;
    use crate::target::{TerminalHost, TerminalTarget};
    use crate::translation::providers::claude_code::TranslatorPids;

    const GONE: &str = "The terminal window has closed";

    pub fn paste(
        translators: &TranslatorPids,
        target: &TerminalTarget,
        text: &str,
    ) -> Result<(), String> {
        let conhost = target.host == TerminalHost::Conhost;
        // conhost turns line breaks into Enter presses, which would submit the lines one by one.
        if conhost && text.contains('\n') {
            return Err(
                "A classic console window can't take several lines safely (line breaks would act \
                 as Enter)"
                    .into(),
            );
        }
        if target.host == TerminalHost::WindowsTerminal && target.tab_runtime_id.is_none() {
            return Err("The bound Windows Terminal tab couldn't be identified".into());
        }

        // The window is still the bound one, a terminal, and not a translator run.
        sessions::check(translators, target).map_err(|e| match e {
            AppError::SessionClosed => GONE.to_string(),
            other => other.to_string(),
        })?;
        let window = Window::from_id(target.hwnd as usize);

        // One synthetic-input + clipboard sequence at a time.
        let _guard = input::lock();

        if !input::wait_until(KEY_RELEASE_TIMEOUT, keys_up) {
            return Err("A key was still held down".into());
        }
        still_bound(window, target)?;

        if window.is_minimised() {
            window.restore();
            if !input::wait_until(RESTORE_TIMEOUT, || !window.is_minimised()) {
                return Err("The terminal window couldn't be restored".into());
            }
        }
        window.set_foreground();
        if !input::wait_until(FOREGROUND_TIMEOUT, || window.is_foreground()) {
            return Err("The terminal couldn't be brought to the front".into());
        }

        // The bound tab (Windows Terminal), by RuntimeId. UI Automation lives on this thread
        // only, for the length of the selection and the last check.
        let uia = match target.tab_runtime_id {
            Some(_) => Some(Uia::new().map_err(|e| format!("UI Automation is unavailable: {e}"))?),
            None => None,
        };
        let tab = match (&uia, &target.tab_runtime_id) {
            (Some(uia), Some(id)) => Some(find_tab(uia, window, id)?),
            _ => None,
        };
        if let Some(tab) = &tab
            && tab.is_selected() != Some(true)
        {
            tab.select()
                .map_err(|e| format!("The bound tab couldn't be selected: {e}"))?;
            if !input::wait_until(SELECT_TIMEOUT, || tab.is_selected() == Some(true)) {
                return Err("The bound tab couldn't be selected".into());
            }
        }
        sleep(SETTLE);

        // Last checks right before the paste.
        if !input::wait_until(KEY_RELEASE_TIMEOUT, keys_up) {
            return Err("A key was held down".into());
        }
        let quiet_ms = QUIET_INPUT.as_millis() as u32;
        if !input::wait_until(QUIET_TIMEOUT, || {
            input::idle_millis().is_some_and(|idle| idle >= quiet_ms)
        }) {
            return Err("The keyboard was in use".into());
        }
        if !window.is_foreground() {
            return Err("The terminal lost the focus".into());
        }
        if let Some(tab) = &tab
            && tab.is_selected() != Some(true)
        {
            return Err("The bound tab is no longer selected".into());
        }
        drop(tab);
        drop(uia);

        write_and_paste(window, conhost, text)
    }

    fn keys_up() -> bool {
        !input::any_modifier_down() && !input::enter_down()
    }

    /// The window still exists with the bound pid.
    fn still_bound(window: Window, target: &TerminalTarget) -> Result<(), String> {
        if window.exists() && window.pid() == target.pid {
            Ok(())
        } else {
            Err(GONE.into())
        }
    }

    fn find_tab(uia: &Uia, window: Window, id: &str) -> Result<Tab, String> {
        uia.find_tab(window.id(), id)
            .map_err(|e| format!("The terminal's tabs couldn't be read: {e}"))?
            .ok_or_else(|| "The bound tab has closed".to_string())
    }

    /// Saves the clipboard text, puts `text` there (hidden from clipboard history), pastes once,
    /// and puts the saved text back unless the clipboard changed meanwhile.
    fn write_and_paste(window: Window, conhost: bool, text: &str) -> Result<(), String> {
        let saved =
            input::read_text().map_err(|_| "The clipboard is in use by another app".to_string())?;
        // The Windows clipboard convention for line breaks.
        let clipboard_text = text.replace('\n', "\r\n");
        if !input::write_text(Some(&clipboard_text), true) {
            input::write_text(saved.as_deref(), true);
            return Err("The clipboard is in use by another app".into());
        }
        let written = input::sequence_number();

        // Claude Code collapses a multiline paste into a placeholder, so a text with more than
        // one line is pasted twice.
        let pastes = if text.contains('\n') { 2 } else { 1 };
        for n in 0..pastes {
            if n > 0 {
                sleep(PASTE_DELAY);
                // The same checks as before the first paste.
                let stop = if !window.is_foreground() {
                    Some("The terminal lost the focus")
                } else if !keys_up() {
                    Some("The keyboard was in use")
                } else {
                    None
                };
                if let Some(reason) = stop {
                    if input::sequence_number() == written {
                        input::write_text(saved.as_deref(), true);
                    }
                    return Err(reason.into());
                }
            }
            if conhost {
                window.send_conhost_paste();
            } else if !input::send_ctrl_v() {
                input::write_text(saved.as_deref(), true);
                return Err(
                    "Windows blocked the keystroke (is the terminal running as administrator?)"
                        .into(),
                );
            }
        }

        sleep(PASTE_DELAY);
        if input::sequence_number() != written {
            log::info!("Delivery: the clipboard changed after the paste; not restored");
            return Ok(());
        }
        if !input::write_text(saved.as_deref(), true) {
            log::warn!("Delivery: couldn't restore the previous clipboard text");
        }
        Ok(())
    }
}

#[cfg(not(windows))]
mod imp {
    use super::{TerminalTarget, TranslatorPids};

    pub fn paste(_: &TranslatorPids, _: &TerminalTarget, _: &str) -> Result<(), String> {
        Err("Sending to a terminal needs Windows".into())
    }
}
