//! `system::popup`: the hotkey popup window.
//!
//! The `popup` window (from `popup.html`) is created hidden at startup: frameless,
//! always-on-top, out of the taskbar, about 520x360. [`show`] centers it on the monitor under the
//! cursor and focuses it; losing focus hides it again ([`on_window_event`]).
//!
//! The popup's JS gets the captured text, its starting mode, where it was selected and, for a
//! reverse translation, the language pair through the `popup://source` event, and sends
//! `main://open-entry` when the user picks "Open in main window"; [`init`] shows the main window
//! for that event (the main window's JS loads the entry).

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Listener, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl,
    WebviewWindowBuilder, Window, WindowEvent,
};
use ts_rs::TS;

use crate::translation::model::Mode;

pub const POPUP_WINDOW: &str = "popup";
/// Rust → popup: the text captured when the hotkey fired.
pub const SOURCE_EVENT: &str = "popup://source";
/// Popup → main window: show this input and result.
pub const OPEN_ENTRY_EVENT: &str = "main://open-entry";

const POPUP_WIDTH: f64 = 520.0;
const POPUP_HEIGHT: f64 = 360.0;

/// Where the popup's text was selected (POPUP-012), classified with UI Automation at capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum SourceKind {
    /// An input, text area or editor.
    Editable,
    /// A document, web page or other content that can't be edited.
    ReadOnly,
    /// Couldn't be told: UIA failed or was too slow, a terminal, or the clipboard fallback.
    Unknown,
}

/// The language direction of a reverse translation, as `TranslationRequest` takes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PopupLanguagePair {
    /// ISO 639-1 code of the text's language (the settings' target language).
    pub native_language: String,
    /// ISO 639-1 code of the result's language (the settings' native language).
    pub target_language: String,
}

/// Payload of `popup://source`. Empty `text` means nothing usable was captured.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PopupSource {
    pub text: String,
    pub mode: Mode,
    pub source_kind: SourceKind,
    /// Set only for a reverse translation (read-only text in the target language): the pair
    /// Translate uses instead of the settings'. Improve and Correct always use the settings'.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub language_pair: Option<PopupLanguagePair>,
}

/// Creates the hidden popup window and wires `main://open-entry` to the main window.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    app.manage(super::replace::SourceWindow::default());
    WebviewWindowBuilder::new(app, POPUP_WINDOW, WebviewUrl::App("popup.html".into()))
        .title("Prompt Translator Popup")
        .inner_size(POPUP_WIDTH, POPUP_HEIGHT)
        .resizable(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .maximizable(false)
        .minimizable(false)
        .visible(false)
        .build()?;

    let handle = app.clone();
    app.listen(OPEN_ENTRY_EVENT, move |_| {
        super::tray::show_main_window(&handle);
        // The popup has done its job; it would also hide on blur, this is just immediate.
        hide(&handle);
    });
    Ok(())
}

/// Window event handler: the popup hides when it loses focus, and closing it only hides it.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != POPUP_WINDOW {
        return;
    }
    match event {
        WindowEvent::Focused(false) => {
            let _ = window.hide();
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            let _ = window.hide();
        }
        _ => {}
    }
}

/// Hides the popup (no-op if it doesn't exist).
pub fn hide<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(POPUP_WINDOW) {
        let _ = window.hide();
    }
}

/// Shows and focuses the popup again, without moving it or sending a new source (after a replace
/// that couldn't even copy, so the user sees the error).
pub fn show_again<R: Runtime>(app: &AppHandle<R>) {
    if let Some(window) = app.get_webview_window(POPUP_WINDOW) {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

/// Centers the popup on the monitor under the cursor, shows and focuses it, then sends it
/// `popup://source`. The event goes out after the window is shown so its JS is awake.
pub fn show<R: Runtime>(app: &AppHandle<R>, source: PopupSource) {
    let Some(window) = app.get_webview_window(POPUP_WINDOW) else {
        log::warn!("The popup window doesn't exist");
        return;
    };

    // Monitor under the cursor, falling back to the primary one.
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    if let Some(monitor) = monitor {
        let scale = monitor.scale_factor();
        let area = monitor.work_area();
        let width = (POPUP_WIDTH * scale).round() as i32;
        let height = (POPUP_HEIGHT * scale).round() as i32;
        let x = area.position.x + (area.size.width as i32 - width) / 2;
        let y = area.position.y + (area.size.height as i32 - height) / 2;
        let _ = window.set_position(PhysicalPosition::new(x, y));
        // Re-apply the size: moving between monitors with different scaling can change it.
        let _ = window.set_size(LogicalSize::new(POPUP_WIDTH, POPUP_HEIGHT));
    }

    if let Err(e) = window.show() {
        log::warn!("Could not show the popup: {e}");
        return;
    }
    let _ = window.set_focus();

    if let Err(e) = app.emit_to(POPUP_WINDOW, SOURCE_EVENT, source) {
        log::warn!("Could not send {SOURCE_EVENT}: {e}");
    }
}
