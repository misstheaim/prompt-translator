//! `layout_fix::hotkey`: the layout-fix hotkey (plan D36–D38, T42). It replaces the selected text,
//! or the word before the cursor, with the same keys typed on the other keyboard layout
//! (`ghbdtn` → `привет`), then switches the input language. No LLM, no popup, no history.
//!
//! [`run`] holds the decision logic; every OS call goes through the small [`Desktop`] trait, whose
//! Win32 implementation uses the helpers in `system::input` (shared with the popup's auto-copy):
//! 1. The foreground window is a terminal, or `autoCopySelection` is off: convert the clipboard
//!    text in place and show a notification; the user pastes. No keys are sent.
//! 2. Otherwise (after keeping a held Alt/Win from opening a menu on release) wait for the
//!    hotkey's modifiers to be released (or stop), save the clipboard text,
//!    send Ctrl+C and wait for the clipboard to change.
//! 3. Nothing copied: send Ctrl+Shift+Left to select the previous word and copy again.
//! 4. Convert; nothing to convert means stop.
//! 5. Paste the result with Ctrl+V, wait [`PASTE_DELAY`], restore the saved clipboard text.
//! 6. Switch the input language to the target layout, if that setting is on.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::settings::SettingsStore;
use crate::system::input::{
    self, COPY_TIMEOUT, ClipboardBusy, MODIFIER_RELEASE_TIMEOUT, SETTLE_TIME,
};

use super::convert::{Direction, convert, detect_direction};

/// How long the target app gets to read the pasted text before the saved clipboard text comes
/// back; apps read the clipboard asynchronously after Ctrl+V.
pub const PASTE_DELAY: Duration = Duration::from_millis(200);

/// Notification after converting the clipboard in place (terminal or auto-copy off).
pub const PASTE_HINT: &str = "Layout fixed: paste with Ctrl+V";

/// Notification when the clipboard holds nothing to convert (terminal or auto-copy off).
pub const COPY_FIRST_HINT: &str = "Copy some text first";

/// Identifies a foreground window for the duration of one run.
pub type WindowId = usize;

/// The OS operations the layout fix needs, so [`run`] stays free of Win32 calls.
pub trait Desktop {
    /// The foreground window, `None` when there is none.
    fn foreground_window(&self) -> Option<WindowId>;
    /// Whether `window` is a terminal (Ctrl+C there interrupts the running command).
    fn is_terminal(&self, window: WindowId) -> bool;
    /// Keeps the release of a held Alt/Win key from opening a menu; no other effect.
    fn mask_modifier_release(&self);
    /// Waits until no modifier key is down; `false` if one is still held after `timeout`.
    fn wait_for_modifiers_released(&self, timeout: Duration) -> bool;
    /// `window` is still in the foreground and no modifier is down, so a chord can be sent.
    fn ready_to_send(&self, window: WindowId) -> bool;
    /// The clipboard text: `Ok(None)` when it holds no text, `Err` when it can't be opened.
    fn read_clipboard(&self) -> Result<Option<String>, ClipboardBusy>;
    /// Replaces the clipboard with `text` (or empties it); `hidden` keeps it out of clipboard
    /// history and monitors. `false` on failure.
    fn write_clipboard(&self, text: Option<&str>, hidden: bool) -> bool;
    /// The clipboard sequence number (changes on every clipboard write).
    fn clipboard_sequence(&self) -> u32;
    /// Waits until the sequence number differs from `before`; `false` after `timeout`.
    fn wait_for_clipboard_change(&self, before: u32, timeout: Duration) -> bool;
    /// The last copy came from an editor that copies the whole line when nothing is selected.
    fn copied_without_selection(&self) -> bool;
    fn send_copy(&self) -> bool;
    fn send_select_previous_word(&self) -> bool;
    fn send_paste(&self) -> bool;
    fn sleep(&self, duration: Duration);
    /// Switches `window`'s input language to the layout `direction` converted to, if installed.
    fn switch_input_language(&self, window: WindowId, direction: Direction);
    fn notify(&self, message: &str);
}

/// Settings that steer one run.
#[derive(Debug, Clone, Copy)]
pub struct Options {
    pub auto_copy_selection: bool,
    pub switch_input_language: bool,
}

/// How a run ended (logged; never contains the user's text).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Terminal or auto-copy off: the clipboard text was converted in place.
    ClipboardConverted(Direction),
    /// Terminal or auto-copy off, and the clipboard held nothing to convert.
    ClipboardEmpty,
    /// The hotkey's modifiers were still held; no keys were sent.
    ModifiersHeld,
    /// The clipboard couldn't be opened or written.
    ClipboardBusy,
    /// Another window came to the front, or a modifier was pressed, before a chord was sent.
    FocusChanged,
    /// Windows blocked the synthetic input (e.g. the foreground app runs elevated).
    InputBlocked,
    /// No selection, and selecting the previous word copied nothing either.
    NothingSelected,
    /// The copied text has no direction, or converting it changes nothing.
    NothingToConvert,
    /// The text was replaced; `word_selected` when the previous word was selected by the app.
    Replaced {
        direction: Direction,
        word_selected: bool,
    },
}

/// One layout-fix run (see the module docs). Blocks for up to about a second.
pub fn run<D: Desktop>(desktop: &D, options: Options) -> Outcome {
    let window = match desktop.foreground_window() {
        Some(window) if options.auto_copy_selection && !desktop.is_terminal(window) => window,
        _ => return convert_clipboard(desktop),
    };

    // Not for terminals: they get no synthetic keys at all.
    desktop.mask_modifier_release();
    if !desktop.wait_for_modifiers_released(MODIFIER_RELEASE_TIMEOUT) {
        return Outcome::ModifiersHeld;
    }
    // Save before copying; without a readable clipboard nothing could be restored.
    let Ok(saved) = desktop.read_clipboard() else {
        return Outcome::ClipboardBusy;
    };
    let saved = Saved {
        text: saved,
        sequence: desktop.clipboard_sequence(),
    };

    let (text, word_selected) = match copy_text(desktop, window) {
        Ok(Some(text)) => (text, false),
        Ok(None) => {
            if !desktop.ready_to_send(window) {
                return saved.restore(desktop, Outcome::FocusChanged);
            }
            if !desktop.send_select_previous_word() {
                return saved.restore(desktop, Outcome::InputBlocked);
            }
            match copy_text(desktop, window) {
                Ok(Some(text)) => (text, true),
                Ok(None) => return saved.restore(desktop, Outcome::NothingSelected),
                Err(outcome) => return saved.restore(desktop, outcome),
            }
        }
        Err(outcome) => return saved.restore(desktop, outcome),
    };

    let Some((direction, converted)) = conversion(&text) else {
        return saved.restore(desktop, Outcome::NothingToConvert);
    };
    if !desktop.write_clipboard(Some(&converted), true) {
        return saved.restore(desktop, Outcome::ClipboardBusy);
    }
    if !desktop.ready_to_send(window) {
        return saved.restore(desktop, Outcome::FocusChanged);
    }
    if !desktop.send_paste() {
        return saved.restore(desktop, Outcome::InputBlocked);
    }
    desktop.sleep(PASTE_DELAY);
    let outcome = saved.restore(
        desktop,
        Outcome::Replaced {
            direction,
            word_selected,
        },
    );

    if options.switch_input_language {
        desktop.switch_input_language(window, direction);
    }
    outcome
}

/// Terminal or auto-copy off: convert the clipboard text in place (no restore) and tell the user.
fn convert_clipboard<D: Desktop>(desktop: &D) -> Outcome {
    let text = desktop.read_clipboard().ok().flatten().unwrap_or_default();
    let Some((direction, converted)) = conversion(&text) else {
        desktop.notify(COPY_FIRST_HINT);
        return Outcome::ClipboardEmpty;
    };
    if !desktop.write_clipboard(Some(&converted), false) {
        return Outcome::ClipboardBusy;
    }
    desktop.notify(PASTE_HINT);
    Outcome::ClipboardConverted(direction)
}

/// The direction and converted text, or `None` when there is no direction or nothing changes.
fn conversion(text: &str) -> Option<(Direction, String)> {
    let direction = detect_direction(text)?;
    let converted = convert(text, direction);
    (converted != text).then_some((direction, converted))
}

/// Sends Ctrl+C to `window` and returns the copied text: `Ok(None)` when nothing was selected
/// (the clipboard didn't change, the copy was an editor's whole-line copy, or it was empty).
fn copy_text<D: Desktop>(desktop: &D, window: WindowId) -> Result<Option<String>, Outcome> {
    // While we waited, a modifier may have been pressed again or another window (possibly a
    // terminal) may have come to the front.
    if !desktop.ready_to_send(window) {
        return Err(Outcome::FocusChanged);
    }
    let before = desktop.clipboard_sequence();
    if !desktop.send_copy() {
        return Err(Outcome::InputBlocked);
    }
    if !desktop.wait_for_clipboard_change(before, COPY_TIMEOUT) {
        return Ok(None);
    }
    desktop.sleep(SETTLE_TIME);
    if desktop.copied_without_selection() {
        return Ok(None);
    }
    Ok(desktop
        .read_clipboard()
        .ok()
        .flatten()
        .filter(|text| !text.is_empty()))
}

/// The user's clipboard text from before the run, and the sequence number right after saving.
struct Saved {
    text: Option<String>,
    sequence: u32,
}

impl Saved {
    /// Puts the saved text back if the clipboard changed since it was saved (an untouched
    /// clipboard keeps any non-text content), then returns `outcome`.
    fn restore<D: Desktop>(&self, desktop: &D, outcome: Outcome) -> Outcome {
        if desktop.clipboard_sequence() != self.sequence
            && !desktop.write_clipboard(self.text.as_deref(), true)
        {
            log::warn!("Layout fix: could not restore the previous clipboard text");
        }
        outcome
    }
}

/// Set while a run is in progress; further presses are ignored until it ends.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// Clears [`RUNNING`] when the run ends.
struct RunningGuard;

impl Drop for RunningGuard {
    fn drop(&mut self) {
        RUNNING.store(false, Ordering::Release);
    }
}

/// The layout-fix hotkey fired: runs the fix off the event loop, unless one is still running.
pub fn on_hotkey<R: Runtime>(app: &AppHandle<R>) {
    if RUNNING.swap(true, Ordering::AcqRel) {
        log::debug!("Layout fix: still running; hotkey ignored");
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let _running = RunningGuard;
        let Some(settings) = app.try_state::<SettingsStore>().map(|s| s.get()) else {
            return;
        };
        let options = Options {
            auto_copy_selection: settings.auto_copy_selection,
            switch_input_language: settings.layout_fix.switch_input_language,
        };
        // One clipboard sequence at a time, shared with the popup's auto-copy.
        let _input = input::lock();
        let outcome = run_platform(&app, options);
        log::info!("Layout fix: {outcome:?}");
    });
}

#[cfg(windows)]
fn run_platform<R: Runtime>(app: &AppHandle<R>, options: Options) -> Outcome {
    run(&win::WinDesktop { app }, options)
}

#[cfg(not(windows))]
fn run_platform<R: Runtime>(_app: &AppHandle<R>, _options: Options) -> Outcome {
    Outcome::NothingSelected
}

#[cfg(windows)]
mod win {
    use std::time::Duration;

    use tauri::{AppHandle, Runtime};
    use tauri_plugin_notification::NotificationExt;

    use crate::layout_fix::convert::Direction;
    use crate::system::input::{self, ClipboardBusy, Window};

    use super::{Desktop, WindowId};

    /// Primary language IDs (`LANG_RUSSIAN`, `LANG_ENGLISH`; kept local to avoid the
    /// `Win32_System_SystemServices` feature for two constants).
    const LANG_RUSSIAN: u16 = 0x19;
    const LANG_ENGLISH: u16 = 0x09;

    /// [`Desktop`] on Win32 (`system::input`) plus Tauri notifications.
    pub struct WinDesktop<'a, R: Runtime> {
        pub app: &'a AppHandle<R>,
    }

    impl<R: Runtime> Desktop for WinDesktop<'_, R> {
        fn foreground_window(&self) -> Option<WindowId> {
            input::foreground_window().map(Window::id)
        }

        fn is_terminal(&self, window: WindowId) -> bool {
            Window::from_id(window).is_terminal()
        }

        fn mask_modifier_release(&self) {
            input::mask_modifier_release();
        }

        fn wait_for_modifiers_released(&self, timeout: Duration) -> bool {
            input::wait_for_modifiers_released(timeout)
        }

        fn ready_to_send(&self, window: WindowId) -> bool {
            Window::from_id(window).is_foreground() && !input::any_modifier_down()
        }

        fn read_clipboard(&self) -> Result<Option<String>, ClipboardBusy> {
            input::read_text()
        }

        fn write_clipboard(&self, text: Option<&str>, hidden: bool) -> bool {
            input::write_text(text, hidden)
        }

        fn clipboard_sequence(&self) -> u32 {
            input::sequence_number()
        }

        fn wait_for_clipboard_change(&self, before: u32, timeout: Duration) -> bool {
            input::wait_for_sequence_change(before, timeout)
        }

        fn copied_without_selection(&self) -> bool {
            input::clipboard_is_empty_selection_copy()
        }

        fn send_copy(&self) -> bool {
            input::send_ctrl_c()
        }

        fn send_select_previous_word(&self) -> bool {
            input::send_select_previous_word()
        }

        fn send_paste(&self) -> bool {
            input::send_ctrl_v()
        }

        fn sleep(&self, duration: Duration) {
            std::thread::sleep(duration);
        }

        fn switch_input_language(&self, window: WindowId, direction: Direction) {
            let language = match direction {
                Direction::ToRussian => LANG_RUSSIAN,
                Direction::ToEnglish => LANG_ENGLISH,
            };
            Window::from_id(window).switch_input_language(language);
        }

        fn notify(&self, message: &str) {
            let title = self
                .app
                .config()
                .product_name
                .clone()
                .unwrap_or_else(|| "Prompt Translator".into());
            if let Err(e) = self
                .app
                .notification()
                .builder()
                .title(title)
                .body(message)
                .show()
            {
                log::warn!("Layout fix: could not show a notification: {e}");
            }
        }
    }
}
