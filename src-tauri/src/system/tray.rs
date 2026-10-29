//! `system::tray`: the tray icon, show/hide of the main window, close-to-tray.
//!
//! The app lives in the tray. Left click or the "Open" menu item shows and focuses the `main`
//! window; "Quit" exits. Closing the main window hides it when `closeToTray` is on, otherwise the
//! app exits.

use tauri::{
    AppHandle, Manager, Runtime, Window, WindowEvent,
    menu::{Menu, MenuEvent, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

use crate::settings::SettingsStore;

const MAIN_WINDOW: &str = "main";
const MENU_OPEN: &str = "open";
const MENU_QUIT: &str = "quit";

/// Shows, un-minimizes and focuses the main window. Does nothing if it doesn't exist.
pub fn show_main_window<R: Runtime>(app: &AppHandle<R>) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        return;
    };
    if let Err(e) = window.show() {
        log::warn!("Could not show the main window: {e}");
    }
    // `show` doesn't restore a minimized window.
    let _ = window.unminimize();
    let _ = window.set_focus();
}

/// Creates the tray icon (tooltip "Prompt Translator", menu Open / Quit).
pub fn init<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, MENU_OPEN, "Open", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, MENU_QUIT, "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &quit])?;

    // The monochrome tray variant; the app icon is the fallback if it can't be used.
    let icon = tauri::include_image!("icons/tray.png");

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .tooltip("Prompt Translator")
        .menu(&menu)
        // The menu opens on right click only; left click shows the main window.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                show_main_window(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn on_menu_event<R: Runtime>(app: &AppHandle<R>, event: MenuEvent) {
    match event.id().as_ref() {
        MENU_OPEN => show_main_window(app),
        MENU_QUIT => app.exit(0),
        _ => {}
    }
}

/// Window event handler: closing the main window hides it (`closeToTray`) or exits the app.
pub fn on_window_event<R: Runtime>(window: &Window<R>, event: &WindowEvent) {
    if window.label() != MAIN_WINDOW {
        return;
    }
    if let WindowEvent::CloseRequested { api, .. } = event {
        // Always take over the close: a hidden popup window would keep the app alive otherwise.
        api.prevent_close();
        let app = window.app_handle();
        let close_to_tray = app
            .try_state::<SettingsStore>()
            .is_none_or(|s| s.get().close_to_tray);
        if close_to_tray {
            let _ = window.hide();
        } else {
            app.exit(0);
        }
    }
}
