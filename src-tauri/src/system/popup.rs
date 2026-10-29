//! `system::popup`: the hotkey popup window.
//!
//! The `popup` window (from `popup.html`) is created hidden at startup: frameless,
//! always-on-top, out of the taskbar, about 520x360. [`show`] centers it on the monitor under the
//! cursor and focuses it; losing focus hides it again ([`on_window_event`]).
//!
//! The popup's JS gets the captured text through the `popup://source` event, and sends
//! `main://open-entry` when the user picks "Open in main window"; [`init`] shows the main window
//! for that event (the main window's JS loads the entry).

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Listener, LogicalSize, Manager, PhysicalPosition, Runtime, WebviewUrl,
    WebviewWindowBuilder, Window, WindowEvent,
};

use crate::translation::model::Mode;

pub const POPUP_WINDOW: &str = "popup";
/// Rust → popup: the text captured when the hotkey fired.
pub const SOURCE_EVENT: &str = "popup://source";
/// Popup → main window: show this input and result.
pub const OPEN_ENTRY_EVENT: &str = "main://open-entry";

const POPUP_WIDTH: f64 = 520.0;
const POPUP_HEIGHT: f64 = 360.0;

/// Payload of `popup://source`. Empty `text` means nothing usable was captured.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopupSource {
    pub text: String,
    pub mode: Mode,
}

/// Creates the hidden popup window and wires `main://open-entry` to the main window.
pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
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
