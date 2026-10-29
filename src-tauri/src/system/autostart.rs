//! `system::autostart`: start with Windows, hidden in the tray.
//!
//! `tauri-plugin-autostart` (registered from Rust in `lib.rs`, with the argument `--autostart`)
//! writes the Run entry; this module applies `settings.autostart` on startup and whenever
//! `settings://changed` fires.
//!
//! Registry values the plugin (through `auto-launch` 0.6, `WindowsEnableMode::Dynamic`) writes,
//! for the uninstaller (T6, `windows/hooks.nsh`). The value name is the plugin's `app_name`,
//! [`AUTOSTART_NAME`] = `Prompt Translator`:
//! - `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, value `Prompt Translator`, data
//!   `<exe path> --autostart` (the path is NOT quoted by the plugin). The mode tries `HKLM` first
//!   and falls back to `HKCU` on "access denied", so an elevated process writes `HKLM` instead;
//!   the per-user app runs unelevated, so it is `HKCU`. `disable` deletes the value under both.
//! - `HKCU\Software\Microsoft\Windows\CurrentVersion\Explorer\StartupApproved\Run`, value
//!   `Prompt Translator`: set to "enabled" by `enable` only if the key already exists, and never
//!   removed by `disable`. Windows also writes it when the user toggles the entry in Task Manager.
//!   The uninstaller must delete it.

use tauri::{AppHandle, Listener, Manager, Runtime};
use tauri_plugin_autostart::ManagerExt;

use crate::settings::{SETTINGS_CHANGED_EVENT, SettingsStore};

/// The Run value name (the plugin's `app_name`); keep in sync with the uninstaller hook.
pub const AUTOSTART_NAME: &str = "Prompt Translator";
/// Argument of the autostart entry: the app then starts hidden in the tray.
pub const AUTOSTART_ARG: &str = "--autostart";

/// The autostart plugin, ready to register with `Builder::plugin`.
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_autostart::Builder::new()
        .app_name(AUTOSTART_NAME)
        .args([AUTOSTART_ARG])
        .build()
}

/// True when this process was started by the autostart entry.
pub fn launched_by_autostart() -> bool {
    std::env::args().any(|a| a == AUTOSTART_ARG)
}

/// Applies the setting now and again on every `settings://changed`.
pub fn init<R: Runtime>(app: &AppHandle<R>) {
    apply(app);
    let handle = app.clone();
    app.listen(SETTINGS_CHANGED_EVENT, move |_| apply(&handle));
}

/// Makes the startup entry match `settings.autostart`.
fn apply<R: Runtime>(app: &AppHandle<R>) {
    let Some(settings) = app.try_state::<SettingsStore>() else {
        return;
    };
    let wanted = settings.get().autostart;
    let autolaunch = app.autolaunch();
    let current = match autolaunch.is_enabled() {
        Ok(enabled) => enabled,
        Err(e) => {
            log::warn!("Could not read the autostart state: {e}");
            false
        }
    };
    if current == wanted {
        return;
    }
    let result = if wanted {
        autolaunch.enable()
    } else {
        autolaunch.disable()
    };
    match result {
        Ok(()) => log::info!("Autostart {}", if wanted { "enabled" } else { "disabled" }),
        Err(e) => log::warn!("Could not change autostart: {e}"),
    }
}
