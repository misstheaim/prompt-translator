//! `system::popup`: the hotkey popup window.
//!
//! The `popup` window (from `popup.html`) is created hidden at startup: frameless,
//! always-on-top, out of the taskbar, about 520x360. [`show`] centers it on the monitor under the
//! cursor and focuses it; losing focus hides it again ([`on_window_event`]).
//!
//! The popup's JS gets the captured text through the `popup://source` event, and sends
//! `main://open-entry` when the user picks "Open in main window"; [`init`] shows the main window
//! for that event (the main window's JS loads the entry).

use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Listener, Manager, PhysicalPosition, PhysicalSize, Runtime, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, Window, WindowEvent,
};

use crate::settings::{PopupBounds, SETTINGS_CHANGED_EVENT, SettingsStore};
use crate::translation::model::Mode;

pub const POPUP_WINDOW: &str = "popup";
/// Rust → popup: the text captured when the hotkey fired.
pub const SOURCE_EVENT: &str = "popup://source";
/// Popup → main window: show this input and result.
pub const OPEN_ENTRY_EVENT: &str = "main://open-entry";

const POPUP_WIDTH: f64 = 520.0;
const POPUP_HEIGHT: f64 = 360.0;
const POPUP_MIN_WIDTH: f64 = 420.0;
const POPUP_MIN_HEIGHT: f64 = 280.0;
/// How long a move/resize must rest before its bounds are saved.
const SAVE_DEBOUNCE: Duration = Duration::from_millis(600);

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
        .min_inner_size(POPUP_MIN_WIDTH, POPUP_MIN_HEIGHT)
        .resizable(true)
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
        WindowEvent::Moved(_) | WindowEvent::Resized(_) => schedule_save_bounds(window),
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

/// Bumped on every move/resize; the save thread only writes if no newer event arrived.
static BOUNDS_GENERATION: AtomicU64 = AtomicU64::new(0);
/// The rectangle [`place`] last applied, so the events it causes aren't saved as a user move.
static APPLIED: Mutex<Option<PopupBounds>> = Mutex::new(None);

fn current_bounds<R: Runtime>(window: &Window<R>) -> Option<PopupBounds> {
    let pos = window.outer_position().ok()?;
    let size = window.outer_size().ok()?;
    Some(PopupBounds {
        x: pos.x,
        y: pos.y,
        width: size.width,
        height: size.height,
    })
}

/// After the user stops moving or resizing the popup, saves its bounds (debounced).
fn schedule_save_bounds<R: Runtime>(window: &Window<R>) {
    // Moves while hidden come from our own positioning (or minimise artefacts).
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let generation = BOUNDS_GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let window = window.clone();
    std::thread::spawn(move || {
        std::thread::sleep(SAVE_DEBOUNCE);
        if BOUNDS_GENERATION.load(Ordering::SeqCst) != generation
            || !window.is_visible().unwrap_or(false)
        {
            return;
        }
        let Some(bounds) = current_bounds(&window) else {
            return;
        };
        if APPLIED.lock().is_ok_and(|a| *a == Some(bounds)) {
            return;
        }
        let app = window.app_handle();
        let Some(store) = app.try_state::<SettingsStore>() else {
            return;
        };
        let mut settings = store.get();
        if settings.popup_bounds == Some(bounds) {
            return;
        }
        settings.popup_bounds = Some(bounds);
        match store.save(settings) {
            Ok(saved) => {
                let _ = app.emit(SETTINGS_CHANGED_EVENT, &saved);
            }
            Err(e) => log::warn!("Could not save the popup bounds: {e}"),
        }
    });
}

/// Whether at least half of `rect` lies on the work area of a connected monitor.
fn mostly_on_screen<R: Runtime>(app: &AppHandle<R>, rect: &PopupBounds) -> bool {
    let Ok(monitors) = app.available_monitors() else {
        return false;
    };
    let (rx0, ry0) = (i64::from(rect.x), i64::from(rect.y));
    let (rx1, ry1) = (rx0 + i64::from(rect.width), ry0 + i64::from(rect.height));
    let total = i64::from(rect.width) * i64::from(rect.height);
    monitors.iter().any(|m| {
        let a = m.work_area();
        let (ax0, ay0) = (i64::from(a.position.x), i64::from(a.position.y));
        let (ax1, ay1) = (
            ax0 + i64::from(a.size.width),
            ay0 + i64::from(a.size.height),
        );
        let w = (rx1.min(ax1) - rx0.max(ax0)).max(0);
        let h = (ry1.min(ay1) - ry0.max(ay0)).max(0);
        total > 0 && w * h * 2 >= total
    })
}

/// Puts the (hidden) popup at its saved bounds when they're still on screen; otherwise centres
/// it on the work area of the monitor under the cursor, keeping the saved size.
fn place<R: Runtime>(app: &AppHandle<R>, window: &WebviewWindow<R>) {
    let saved = app
        .try_state::<SettingsStore>()
        .and_then(|s| s.get().popup_bounds)
        .filter(|b| b.width > 0 && b.height > 0);

    if let Some(bounds) = saved.filter(|b| mostly_on_screen(app, b)) {
        let _ = window.set_size(PhysicalSize::new(bounds.width, bounds.height));
        let _ = window.set_position(PhysicalPosition::new(bounds.x, bounds.y));
        remember_applied(window);
        return;
    }

    // Monitor under the cursor, falling back to the primary one.
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else {
        return;
    };
    let scale = monitor.scale_factor();
    let area = monitor.work_area();
    let (width, height) = match saved {
        Some(b) => (b.width as i32, b.height as i32),
        None => (
            (POPUP_WIDTH * scale).round() as i32,
            (POPUP_HEIGHT * scale).round() as i32,
        ),
    };
    let x = area.position.x + (area.size.width as i32 - width) / 2;
    let y = area.position.y + (area.size.height as i32 - height) / 2;
    let _ = window.set_position(PhysicalPosition::new(x, y));
    // Re-apply the size: moving between monitors with different scaling can change it.
    let _ = window.set_size(PhysicalSize::new(width as u32, height as u32));
    remember_applied(window);
}

fn remember_applied<R: Runtime>(window: &WebviewWindow<R>) {
    if let (Ok(pos), Ok(size), Ok(mut applied)) =
        (window.outer_position(), window.outer_size(), APPLIED.lock())
    {
        *applied = Some(PopupBounds {
            x: pos.x,
            y: pos.y,
            width: size.width,
            height: size.height,
        });
    }
}

/// Places the popup (saved bounds, else centred on the monitor under the cursor), shows and focuses it, then sends it
/// `popup://source`. The event goes out after the window is shown so its JS is awake.
pub fn show<R: Runtime>(app: &AppHandle<R>, source: PopupSource) {
    let Some(window) = app.get_webview_window(POPUP_WINDOW) else {
        log::warn!("The popup window doesn't exist");
        return;
    };

    place(app, &window);

    if let Err(e) = window.show() {
        log::warn!("Could not show the popup: {e}");
        return;
    }
    let _ = window.set_focus();

    if let Err(e) = app.emit_to(POPUP_WINDOW, SOURCE_EVENT, source) {
        log::warn!("Could not send {SOURCE_EVENT}: {e}");
    }
}
