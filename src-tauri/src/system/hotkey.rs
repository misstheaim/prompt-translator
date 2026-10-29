//! `system::hotkey`: the global hotkey that opens the popup.
//!
//! `settings.hotkey` (`Ctrl+Alt+Space`: modifiers Ctrl, Alt, Shift, Super, then one key; the
//! format the settings screen's capture field saves) is registered with
//! `tauri-plugin-global-shortcut` at startup and again whenever `settings://changed` fires with a
//! different value. A registration failure (hotkey taken, bad string) is kept in [`HotkeyState`],
//! sent as `hotkey://error` and returned by the `hotkey_error` command, so the main window can
//! show a banner with a hint to pick another combination.

use std::str::FromStr;
use std::sync::Mutex;

use tauri::{AppHandle, Emitter, Listener, Manager, Runtime};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};

use crate::settings::{SETTINGS_CHANGED_EVENT, SettingsStore};
use crate::translation::{mode_detect, model::Mode};

use super::clipboard;
use super::popup::{self, PopupSource};

/// Rust → main window: the registration error message, or `null` once the hotkey works.
pub const HOTKEY_ERROR_EVENT: &str = "hotkey://error";

#[derive(Default)]
struct Inner {
    /// The hotkey string last applied (registered or failed), to skip no-op settings saves.
    applied: Option<String>,
    registered: Option<Shortcut>,
    error: Option<String>,
}

/// Managed state: what is registered and the last registration error.
#[derive(Default)]
pub struct HotkeyState(Mutex<Inner>);

impl HotkeyState {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn error(&self) -> Option<String> {
        self.lock().error.clone()
    }
}

/// The global-shortcut plugin, ready to register with `Builder::plugin`. Used from Rust only.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(on_shortcut)
        .build()
}

/// Registers `settings.hotkey` now and re-registers on every `settings://changed`.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    app.manage(HotkeyState::default());
    apply(app);
    let handle = app.clone();
    app.listen(SETTINGS_CHANGED_EVENT, move |_| apply(&handle));
}

/// Makes the registered shortcut match `settings.hotkey`.
fn apply<R: Runtime>(app: &AppHandle<R>) {
    let (Some(settings), Some(state)) = (
        app.try_state::<SettingsStore>(),
        app.try_state::<HotkeyState>(),
    ) else {
        return;
    };
    let wanted = settings.get().hotkey.trim().to_string();

    let mut inner = state.lock();
    if inner.applied.as_deref() == Some(wanted.as_str()) {
        return;
    }
    let shortcuts = app.global_shortcut();
    if let Some(old) = inner.registered.take()
        && let Err(e) = shortcuts.unregister(old)
    {
        log::warn!("Could not unregister the old hotkey: {e}");
    }
    inner.applied = Some(wanted.clone());

    let result = Shortcut::from_str(&wanted)
        .map_err(|e| format!("\"{wanted}\" isn't a valid hotkey ({e})."))
        .and_then(|shortcut| {
            shortcuts
                .register(shortcut)
                .map(|()| shortcut)
                .map_err(|e| format!("Could not register \"{wanted}\": {e}."))
        });
    let error = match result {
        Ok(shortcut) => {
            log::info!("Hotkey {wanted} registered");
            inner.registered = Some(shortcut);
            None
        }
        Err(message) => {
            log::warn!("{message}");
            Some(format!(
                "{message} The popup hotkey is off. Choose another combination in Settings."
            ))
        }
    };
    inner.error = error.clone();
    drop(inner);
    let _ = app.emit(HOTKEY_ERROR_EVENT, error);
}

/// Plugin handler: the popup hotkey fired.
fn on_shortcut<R: Runtime>(app: &AppHandle<R>, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let is_popup_hotkey = app
        .try_state::<HotkeyState>()
        .is_some_and(|s| s.lock().registered.as_ref() == Some(shortcut));
    if !is_popup_hotkey {
        return;
    }
    let app = app.clone();
    // Capturing may wait (auto-copy), so it must not block the event loop.
    tauri::async_runtime::spawn_blocking(move || {
        let text = capture_source(&app);
        let last = app
            .try_state::<SettingsStore>()
            .map(|s| s.get().last_popup_mode)
            .unwrap_or(Mode::Correct);
        let mode = mode_detect::detect_mode(&text, last);
        popup::show(&app, PopupSource { text, mode });
    });
}

/// The text to translate: the foreground app's selection when `autoCopySelection` is on and it
/// could be copied (plan D5, [`clipboard::copy_selection`]), otherwise the clipboard. Empty or
/// non-text content gives "". Blocks for up to about a second, so it runs off the event loop.
fn capture_source<R: Runtime>(app: &AppHandle<R>) -> String {
    let auto_copy = app
        .try_state::<SettingsStore>()
        .is_some_and(|s| s.get().auto_copy_selection);
    if auto_copy && let Some(text) = clipboard::copy_selection() {
        return text;
    }
    app.clipboard().read_text().unwrap_or_default()
}
