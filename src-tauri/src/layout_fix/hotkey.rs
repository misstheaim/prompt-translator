//! `layout_fix::hotkey`: the layout-fix hotkey (plan D36–D38, T42). It replaces the selected text,
//! or the wrong-layout words before the cursor, with the same keys typed on the other keyboard layout
//! (`ghbdtn` → `привет`), then switches the input language. No LLM, no popup, no history.
//!
//! [`run`] holds the decision logic; every OS call goes through the small [`Desktop`] trait, whose
//! Win32 implementation uses the helpers in `system::input` (shared with the popup's auto-copy):
//! 1. The foreground window is a terminal, or `autoCopySelection` is off: convert the clipboard
//!    text in place and show a notification; the user pastes. No keys are sent.
//! 2. Otherwise (after keeping a held Alt/Win from opening a menu on release) wait for the
//!    hotkey's modifiers to be released (or stop), save the clipboard text,
//!    send Ctrl+C and wait for the clipboard to change.
//! 3. Nothing copied: send Ctrl+Shift+Left to select the previous word and copy again. Then keep
//!    adding the word to the left (Ctrl+Shift+Left + copy, the new word is the difference) while
//!    the words look typed in the wrong layout or are too short to tell
//!    ([`is_wrong_layout`]); stop at the first word that looks right, at the line or field start
//!    (the selection doesn't grow), at a line break or after [`MAX_WORDS`] words. Undecided words
//!    at the left edge with no wrong-layout word beyond them are not converted. If the selection
//!    now holds words that are not to be converted, shrink it with Ctrl+Shift+Right (copying
//!    after each press) until only whitespace is left before the part to convert; whatever can't
//!    be dropped that way is pasted back unchanged.
//! 4. Convert; nothing to convert means stop.
//! 5. Paste the result with Ctrl+V, wait [`PASTE_DELAY`], restore the saved clipboard text.
//! 6. Switch the input language to the target layout, if that setting is on.
//!
//! Every chord is sent only while the window is still in front and no modifier is down; a copy
//! that fails or a selection that changes unexpectedly while growing or shrinking ends the run
//! without pasting (the text stays selected).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tauri::{AppHandle, Manager, Runtime};

use crate::settings::SettingsStore;
use crate::system::input::{
    self, COPY_TIMEOUT, ClipboardBusy, MODIFIER_RELEASE_TIMEOUT, SETTLE_TIME,
};

use super::convert::{Direction, convert, detect_direction, is_wrong_layout};

/// With no selection, the most words the selection grows to (Ctrl+Shift+Left presses, the first
/// one included).
pub const MAX_WORDS: usize = 15;

/// Nothing left of a line break joins the converted words.
const LINE_BREAKS: [char; 2] = ['\n', '\r'];

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
    /// Ctrl+Shift+Left: moves the selection's moving end one word left.
    fn send_select_previous_word(&self) -> bool;
    /// Ctrl+Shift+Right: moves the selection's moving end one word right.
    fn send_select_next_word(&self) -> bool;
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
    /// While the selection was grown or shrunk word by word, a copy failed or the selection
    /// changed in an unexpected way; nothing was pasted.
    SelectionChanged,
    /// The copied text has no direction, or converting it changes nothing.
    NothingToConvert,
    /// The text was replaced. `words_selected`: how many words the run selected itself
    /// (Ctrl+Shift+Left presses); 0 when the user had selected the text.
    Replaced {
        direction: Direction,
        words_selected: usize,
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

    let selection = match copy_text(desktop, window) {
        Ok(Some(text)) => Selection {
            text,
            tail_start: 0,
            words: 0,
        },
        Ok(None) => match select_words(desktop, window) {
            Ok(Some(selection)) => selection,
            Ok(None) => return saved.restore(desktop, Outcome::NothingSelected),
            Err(outcome) => return saved.restore(desktop, outcome),
        },
        Err(outcome) => return saved.restore(desktop, outcome),
    };

    let tail_len = selection.text.len() - selection.tail_start;
    let Some((direction, converted_tail)) = conversion(&selection.text[selection.tail_start..])
    else {
        return saved.restore(desktop, Outcome::NothingToConvert);
    };
    let selected = if needs_shrinking(&selection.text, tail_len) {
        match drop_leading_words(desktop, window, &selection, tail_len) {
            Ok(selected) => selected,
            Err(outcome) => return saved.restore(desktop, outcome),
        }
    } else {
        selection.text.clone()
    };
    // `selected` is a suffix of the grown selection holding the whole tail, so this is the start
    // of the tail in it (a char boundary): what stays selected before it is pasted back as is.
    let kept = &selected[..selected.len() - tail_len];
    let converted = format!("{kept}{converted_tail}");
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
            words_selected: selection.words,
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

/// The selected text and the part of it to convert.
struct Selection {
    text: String,
    /// Byte offset in `text` where the part to convert starts (0: all of it).
    tail_start: usize,
    /// Words the run selected itself (Ctrl+Shift+Left presses); 0 for the user's own selection.
    words: usize,
}

/// No selection: selects the word before the cursor, then keeps adding the word to its left
/// while [`wrong_layout_tail`] asks for more, up to [`MAX_WORDS`] words. `Ok(None)` when even the
/// first word copied nothing.
fn select_words<D: Desktop>(desktop: &D, window: WindowId) -> Result<Option<Selection>, Outcome> {
    if !desktop.ready_to_send(window) {
        return Err(Outcome::FocusChanged);
    }
    if !desktop.send_select_previous_word() {
        return Err(Outcome::InputBlocked);
    }
    let Some(mut text) = copy_text(desktop, window)? else {
        return Ok(None);
    };
    let mut words = 1;
    // A first word that already spans a line break is converted whole, as before.
    if text.contains(LINE_BREAKS) {
        return Ok(Some(Selection {
            text,
            tail_start: 0,
            words,
        }));
    }

    let mut tail = wrong_layout_tail(&text);
    while tail.extend && words < MAX_WORDS {
        if !desktop.ready_to_send(window) {
            return Err(Outcome::FocusChanged);
        }
        if !desktop.send_select_previous_word() {
            return Err(Outcome::InputBlocked);
        }
        // From here on the selection is only known from what Ctrl+C returns.
        let Some(grown) = copy_text(desktop, window)? else {
            return Err(Outcome::SelectionChanged);
        };
        if grown == text {
            // The line or field start: nothing more to the left.
            break;
        }
        if !grown.ends_with(text.as_str()) {
            return Err(Outcome::SelectionChanged);
        }
        text = grown;
        words += 1;
        tail = wrong_layout_tail(&text);
    }
    Ok(Some(Selection {
        tail_start: tail.start,
        text,
        words,
    }))
}

/// Whether the selection holds more than spaces before its last `tail_len` bytes (the part to
/// convert): words, or a line break, that should not be pasted over.
fn needs_shrinking(selected: &str, tail_len: usize) -> bool {
    selected[..selected.len() - tail_len]
        .chars()
        .any(|c| !c.is_whitespace() || LINE_BREAKS.contains(&c))
}

/// Shrinks the grown selection from the left, one Ctrl+Shift+Right at a time, until only spaces
/// are left before its last `tail_len` bytes; returns the text then selected (a suffix of
/// `selection.text` that still holds the whole tail). Apps differ in where Ctrl+Right stops
/// (VS Code stops at word ends), so the selection is copied after every press; if a press goes
/// past the start of the tail, one Ctrl+Shift+Left takes it back and what is left stays selected.
fn drop_leading_words<D: Desktop>(
    desktop: &D,
    window: WindowId,
    selection: &Selection,
    tail_len: usize,
) -> Result<String, Outcome> {
    let mut selected = selection.text.clone();
    // It never takes more presses than the words that were added.
    for _ in 1..selection.words {
        if !needs_shrinking(&selected, tail_len) {
            break;
        }
        if !desktop.ready_to_send(window) {
            return Err(Outcome::FocusChanged);
        }
        if !desktop.send_select_next_word() {
            return Err(Outcome::InputBlocked);
        }
        let Some(shrunk) = copy_text(desktop, window)? else {
            return Err(Outcome::SelectionChanged);
        };
        if !selected.ends_with(shrunk.as_str()) {
            return Err(Outcome::SelectionChanged);
        }
        if shrunk.len() < tail_len {
            // Past the start of the tail (VS Code, after a line break, skips to the end of the
            // line's first word): take one word back.
            if !desktop.ready_to_send(window) {
                return Err(Outcome::FocusChanged);
            }
            if !desktop.send_select_previous_word() {
                return Err(Outcome::InputBlocked);
            }
            return match copy_text(desktop, window)? {
                Some(regrown)
                    if regrown.len() >= tail_len && selection.text.ends_with(regrown.as_str()) =>
                {
                    Ok(regrown)
                }
                _ => Err(Outcome::SelectionChanged),
            };
        }
        if shrunk.len() == selected.len() {
            // Ctrl+Shift+Right didn't move: paste the rest back unchanged.
            break;
        }
        selected = shrunk;
    }
    Ok(selected)
}

/// Where the part to convert starts in a selection grown word by word, and whether to add the
/// next word to the left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Tail {
    /// Byte offset of the first word to convert (the text length when there is no word).
    start: usize,
    extend: bool,
}

/// Judges the whitespace-separated words of `selection` from right to left (LAYOUT-005): the
/// rightmost word, the one the user asked for, is always converted; a word to its left joins
/// when it looks typed in the wrong layout in the same direction ([`is_wrong_layout`]); words too
/// short to tell join only when a wrong-layout word lies beyond them. The first word that looks
/// right, or wrong the other way round, ends it; so does a line break.
fn wrong_layout_tail(selection: &str) -> Tail {
    let floor = selection.rfind(LINE_BREAKS).map_or(0, |i| i + 1);
    let mut words = word_spans(&selection[floor..])
        .into_iter()
        .rev()
        .map(|(offset, word)| (floor + offset, word));
    let Some((mut start, last)) = words.next() else {
        return Tail {
            start: selection.len(),
            extend: false,
        };
    };
    let mut direction = detect_direction(last);
    for (offset, word) in words {
        match is_wrong_layout(word) {
            None => {}
            Some(true) if direction.is_none() || detect_direction(word) == direction => {
                direction = detect_direction(word);
                start = offset;
            }
            Some(_) => {
                return Tail {
                    start,
                    extend: false,
                };
            }
        }
    }
    Tail {
        start,
        extend: floor == 0,
    }
}

/// The whitespace-separated words of `text` with their byte offsets.
fn word_spans(text: &str) -> Vec<(usize, &str)> {
    let mut spans = Vec::new();
    let mut word_start = None;
    for (i, c) in text.char_indices() {
        match (c.is_whitespace(), word_start) {
            (true, Some(start)) => {
                spans.push((start, &text[start..i]));
                word_start = None;
            }
            (false, None) => word_start = Some(i),
            _ => {}
        }
    }
    if let Some(start) = word_start {
        spans.push((start, &text[start..]));
    }
    spans
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

        fn send_select_next_word(&self) -> bool {
            input::send_select_next_word()
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
