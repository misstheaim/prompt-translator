//! `system::hotkey`: the global hotkeys, the one that opens the popup and the layout fix's.
//!
//! `settings.hotkey` (popup) and `settings.layoutFix.hotkey` (layout fix, only while
//! `layoutFix.enabled`) use the format the settings screen's capture field saves
//! (`Ctrl+Alt+Space`: modifiers Ctrl, Alt, Shift, Super, then one key). They are registered with
//! `tauri-plugin-global-shortcut` at startup and again whenever `settings://changed` fires with a
//! different value; the handler dispatches by which shortcut fired. Registration failures (hotkey
//! taken, bad string) are kept in [`HotkeyState`], sent as `hotkey://error` (both hotkeys' messages
//! in one string) and returned by the `hotkey_error` command, so the main window can show a banner
//! with a hint to pick another combination.

use std::str::FromStr;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Listener, Manager, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::layout_fix;
use crate::settings::{PopupStartMode, SETTINGS_CHANGED_EVENT, Settings, SettingsStore};
use crate::translation::{language::LanguagePair, mode_detect, model::Mode};

use super::clipboard;
use super::popup::{self, PopupLanguagePair, PopupSource, SourceKind};

/// Rust → main window: the registration error message, or `null` once the hotkeys work.
pub const HOTKEY_ERROR_EVENT: &str = "hotkey://error";

/// One global hotkey and its registration state.
#[derive(Default)]
struct Slot {
    /// The hotkey string last applied (registered or failed), `None` while the slot is off; used
    /// to skip no-op settings saves.
    applied: Option<String>,
    registered: Option<Shortcut>,
    error: Option<String>,
}

#[derive(Default)]
struct Inner {
    popup: Slot,
    layout_fix: Slot,
}

impl Inner {
    /// Both slots' errors as one message, `None` when both work.
    fn error(&self) -> Option<String> {
        let messages: Vec<&str> = [&self.popup.error, &self.layout_fix.error]
            .into_iter()
            .flatten()
            .map(String::as_str)
            .collect();
        (!messages.is_empty()).then(|| messages.join(" "))
    }
}

/// Managed state: what is registered and the last registration errors.
#[derive(Default)]
pub struct HotkeyState(Mutex<Inner>);

impl HotkeyState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn error(&self) -> Option<String> {
        self.lock().error()
    }
}

/// The global-shortcut plugin, ready to register with `Builder::plugin`. Used from Rust only.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(on_shortcut)
        .build()
}

/// Registers the hotkeys now and re-registers on every `settings://changed`.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(HotkeyState::default());
    apply(app);
    let handle = app.clone();
    app.listen(SETTINGS_CHANGED_EVENT, move |_| apply(&handle));
}

/// What each slot should have registered: the popup hotkey always, the layout-fix hotkey only
/// while the tool is enabled.
fn wanted(settings: &Settings) -> (Option<String>, Option<String>) {
    let popup = Some(settings.hotkey.trim().to_string());
    let layout_fix = settings
        .layout_fix
        .enabled
        .then(|| settings.layout_fix.hotkey.trim().to_string());
    (popup, layout_fix)
}

/// Makes the registered shortcuts match the settings, touching only the slots that changed.
fn apply<R: Runtime>(app: &AppHandle<R>) {
    let (Some(settings), Some(state)) = (
        app.try_state::<SettingsStore>(),
        app.try_state::<HotkeyState>(),
    ) else {
        return;
    };
    let (popup_wanted, layout_fix_wanted) = wanted(&settings.get());

    let mut guard = state.lock();
    // Reborrow once so the two slots can be borrowed mutably at the same time.
    let inner = &mut *guard;
    let popup_changed = inner.popup.applied != popup_wanted;
    let layout_fix_changed = inner.layout_fix.applied != layout_fix_wanted;
    if !popup_changed && !layout_fix_changed {
        return;
    }

    // Unregister every changed slot before registering, so two hotkeys can swap combinations.
    let shortcuts = app.global_shortcut();
    for (changed, slot) in [
        (popup_changed, &mut inner.popup),
        (layout_fix_changed, &mut inner.layout_fix),
    ] {
        if changed
            && let Some(old) = slot.registered.take()
            && let Err(e) = shortcuts.unregister(old)
        {
            log::warn!("Could not unregister the old hotkey: {e}");
        }
    }
    if popup_changed {
        register(
            app,
            &mut inner.popup,
            popup_wanted,
            "The popup hotkey is off.",
        );
    }
    if layout_fix_changed {
        register(
            app,
            &mut inner.layout_fix,
            layout_fix_wanted,
            "The layout fix hotkey is off.",
        );
    }

    let error = inner.error();
    drop(guard);
    let _ = app.emit(HOTKEY_ERROR_EVENT, error);
}

/// Registers `wanted` into an empty `slot` (`None`: the slot stays off) and records the result.
fn register<R: Runtime>(app: &AppHandle<R>, slot: &mut Slot, wanted: Option<String>, off: &str) {
    slot.applied = wanted.clone();
    slot.error = None;
    let Some(wanted) = wanted else {
        return;
    };
    let result = Shortcut::from_str(&wanted)
        .map_err(|e| format!("\"{wanted}\" isn't a valid hotkey ({e})."))
        .and_then(|shortcut| {
            app.global_shortcut()
                .register(shortcut)
                .map(|()| shortcut)
                .map_err(|e| format!("Could not register \"{wanted}\": {e}."))
        });
    match result {
        Ok(shortcut) => {
            log::info!("Hotkey {wanted} registered");
            slot.registered = Some(shortcut);
        }
        Err(message) => {
            log::warn!("{message}");
            slot.error = Some(format!(
                "{message} {off} Choose another combination in Settings."
            ));
        }
    }
}

/// Which registered hotkey a shortcut event belongs to.
enum Fired {
    Popup,
    LayoutFix,
}

/// Plugin handler: dispatches by which hotkey fired.
fn on_shortcut<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let fired = app.try_state::<HotkeyState>().and_then(|state| {
        let inner = state.lock();
        if inner.popup.registered.as_ref() == Some(shortcut) {
            Some(Fired::Popup)
        } else if inner.layout_fix.registered.as_ref() == Some(shortcut) {
            Some(Fired::LayoutFix)
        } else {
            None
        }
    });
    match fired {
        Some(Fired::Popup) => open_popup(app),
        Some(Fired::LayoutFix) => layout_fix::hotkey::on_hotkey(app),
        None => {}
    }
}

/// The popup hotkey fired: capture the source text and show the popup.
fn open_popup<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    // Capturing may wait (auto-copy), so it must not block the event loop.
    tauri::async_runtime::spawn_blocking(move || {
        let settings = app
            .try_state::<SettingsStore>()
            .map(|s| s.get())
            .unwrap_or_default();
        let (text, source_kind) = capture_source(&app, &settings);
        let pair = LanguagePair::from_settings(&settings);
        let (mode, language_pair) = start_mode(&text, source_kind, &settings, pair);
        log::debug!("Popup source: {source_kind:?}, mode {mode:?}");
        popup::show(
            &app,
            PopupSource {
                text,
                mode,
                source_kind,
                language_pair,
            },
        );
    });
}

/// The popup's starting mode (POPUP-003), plus the reversed pair for a reverse translation.
///
/// Read-only text is always translated into the opposite language of its first word: text
/// starting in the target language is translated into the native one (the reversed pair goes
/// to the popup), anything else into the target language as usual. This translation is for
/// reading only and never reaches a Claude session. Editable or unknown sources start in the
/// `popupStartMode` setting.
fn start_mode(
    text: &str,
    source_kind: SourceKind,
    settings: &Settings,
    pair: LanguagePair,
) -> (Mode, Option<PopupLanguagePair>) {
    if source_kind == SourceKind::ReadOnly {
        let reversed = mode_detect::starts_in_target(text, pair).then(|| PopupLanguagePair {
            native_language: pair.target.code.to_owned(),
            target_language: pair.native.code.to_owned(),
        });
        return (Mode::Translate, reversed);
    }
    let last = settings.last_popup_mode;
    let mode = match settings.popup_start_mode {
        PopupStartMode::Auto => mode_detect::detect_mode(text, last, pair),
        PopupStartMode::Translate => Mode::Translate,
        PopupStartMode::Improve => Mode::Improve,
        PopupStartMode::Correct => Mode::Correct,
        PopupStartMode::LastUsed => last,
    };
    (mode, None)
}

/// How long the capture waits for the focused element's classification (POPUP-012), counted
/// from the hotkey; it runs while auto-copy waits for the modifiers, so usually costs nothing.
#[cfg(windows)]
const SOURCE_KIND_CAP: std::time::Duration = std::time::Duration::from_millis(100);

/// The text to translate and where it was selected: the foreground app's selection when
/// `autoCopySelection` is on and it could be copied (plan D5, [`clipboard::copy_selection`]),
/// otherwise the clipboard. Empty or non-text content gives "". Blocks for up to about a
/// second, so it runs off the event loop.
///
/// The source kind comes from UI Automation, asked before Ctrl+C is sent. The clipboard
/// fallback (terminal, auto-copy off, nothing selected) is always `Unknown`, and so is a UIA
/// answer that fails or doesn't arrive within [`SOURCE_KIND_CAP`].
fn capture_source<R: Runtime>(app: &AppHandle<R>, settings: &Settings) -> (String, SourceKind) {
    if settings.auto_copy_selection {
        #[cfg(windows)]
        let pending = {
            let started = std::time::Instant::now();
            // A terminal is always `Unknown` (and never gets Ctrl+C), so don't ask UIA there.
            let terminal = super::input::foreground_window().is_none_or(|w| w.is_terminal());
            let receiver = if terminal {
                None
            } else {
                super::uia::spawn_source_kind()
            };
            (started, receiver)
        };
        if let Some(text) = clipboard::copy_selection() {
            #[cfg(windows)]
            let kind = {
                let (started, receiver) = pending;
                let left = SOURCE_KIND_CAP.saturating_sub(started.elapsed());
                receiver
                    .and_then(|r| r.recv_timeout(left).ok())
                    .unwrap_or(SourceKind::Unknown)
            };
            #[cfg(not(windows))]
            let kind = SourceKind::Unknown;
            return (text, kind);
        }
    }
    (
        app.clipboard().read_text().unwrap_or_default(),
        SourceKind::Unknown,
    )
}
