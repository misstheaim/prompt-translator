pub mod error;
pub mod history;
pub mod layout_fix;
pub mod presets;
pub mod settings;
pub mod setup;
pub mod system;
pub mod target;
pub mod translation;
pub mod updater;

use tauri::Manager;
use tauri_plugin_log::{RotationStrategy, Target, TargetKind};

/// Rotated log files keep at most this many bytes each (1 MB), 5 files in total.
const LOG_FILE_MAX_BYTES: u128 = 1_000_000;
const LOG_FILES_KEPT: usize = 5;

/// Installs the process-wide rustls crypto provider (ring, the one `tauri-plugin-updater` already
/// pulls in). reqwest is built with `rustls-no-provider`, so any `reqwest::Client` panics without
/// it. Call once at the very start of every binary; an "already installed" error is ignored.
pub fn install_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_crypto_provider();
    tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the main window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            system::tray::show_main_window(app);
        }))
        // Registered from Rust only; the webview gets no autostart permission.
        .plugin(system::autostart::plugin())
        // Registered from Rust only (`system::hotkey`); the webview gets no shortcut permission.
        .plugin(system::hotkey::plugin())
        .on_window_event(|window, event| {
            system::tray::on_window_event(window, event);
            system::popup::on_window_event(window, event);
        })
        // Used from Rust only (`copy_result`); the webview gets no clipboard permission.
        .plugin(tauri_plugin_clipboard_manager::init())
        // Used from Rust only (`pick_file`); the webview gets no dialog permission.
        .plugin(tauri_plugin_dialog::init())
        // Used from Rust only (`open_url`, allow-listed URLs); the webview gets no opener permission.
        .plugin(tauri_plugin_opener::init())
        // Used from Rust only (layout-fix hints); the webview gets no notification permission.
        .plugin(tauri_plugin_notification::init())
        // Registered from Rust only (`updater`); the webview gets no updater permission.
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            system::paths::init(app.handle())?;

            // The log plugin needs the logs dir, which needs the app handle, so it is added here.
            let mut targets = vec![Target::new(TargetKind::Folder {
                path: system::paths::logs_dir()?,
                file_name: Some("prompt-translator".into()),
            })];
            if cfg!(debug_assertions) {
                targets.push(Target::new(TargetKind::Stdout));
            }
            app.handle().plugin(
                tauri_plugin_log::Builder::new()
                    .clear_targets()
                    .targets(targets)
                    .max_file_size(LOG_FILE_MAX_BYTES)
                    .rotation_strategy(RotationStrategy::KeepSome(LOG_FILES_KEPT))
                    .level(log::LevelFilter::Info)
                    .build(),
            )?;
            log::info!("Prompt Translator {} started", app.package_info().version);

            // Translator-run registry (read by session discovery) and the run counter that
            // triggers cleanup; the dirs are tidied once at startup.
            // The Claude Code provider gets handles to the same two (clones share the state).
            let translator_pids = translation::providers::claude_code::TranslatorPids::new();
            let housekeeping = translation::providers::claude_code::Housekeeping::new();
            app.manage(translator_pids.clone());
            app.manage(housekeeping.clone());
            translation::providers::claude_code::housekeeping::run(true);

            let settings = settings::SettingsStore::load(system::paths::settings_file()?);
            let presets = presets::PresetStore::open(system::paths::presets_dir()?);
            // History must never stop the app: without the database it is not managed, the
            // service skips saving and the history commands return an error.
            let history = match history::HistoryDb::open(&system::paths::history_db()?) {
                Ok(db) => {
                    app.manage(db.clone());
                    Some(db)
                }
                Err(e) => {
                    log::warn!("History database unavailable: {e}");
                    None
                }
            };
            // The service shares the stores with their commands (clones are handles).
            app.manage(translation::service::TranslationService::new(
                settings.clone(),
                presets.clone(),
                translation::providers::registry(translator_pids, housekeeping),
                history,
            ));
            app.manage(settings);
            app.manage(presets);
            app.manage(setup::ollama_setup::SetupTasks::default());
            // Bound terminal targets; their watcher runs only while some exist.
            app.manage(target::sessions::Bindings::new());

            system::tray::init(app.handle())?;
            system::autostart::init(app.handle());
            // The popup exists (hidden) from the start so the hotkey works at once; the hotkey
            // is registered after the settings are managed.
            system::popup::init(app.handle())?;
            system::hotkey::init(app.handle());
            updater::spawn_startup_check(app.handle());
            // Started by the autostart entry: stay in the tray (the main window is created
            // hidden).
            if !system::autostart::launched_by_autostart() {
                system::tray::show_main_window(app.handle());
            }
            Ok(())
        })
        // One registration line per module.
        .invoke_handler(tauri::generate_handler![
            system::commands::ping,
            system::commands::hotkey_error,
            system::commands::pick_file,
            system::commands::open_url,
            system::commands::app_info,
            system::commands::open_folder,
            system::commands::unload_model,
            system::commands::uninstaller_available,
            system::commands::run_uninstaller,
            updater::check_update,
            updater::install_update,
            settings::commands::get_settings,
            settings::commands::save_settings,
            settings::commands::set_last_popup_mode,
            translation::commands::detect_mode,
            translation::commands::list_languages,
            translation::commands::translate,
            translation::commands::cancel_translation,
            translation::commands::provider_status,
            translation::commands::copy_result,
            presets::commands::list_presets,
            presets::commands::save_preset,
            presets::commands::delete_preset,
            presets::commands::reset_presets,
            history::commands::history_list,
            history::commands::history_get,
            history::commands::history_delete,
            history::commands::history_clear,
            history::commands::mistakes_summary,
            setup::commands::environment_check,
            setup::commands::ollama_install,
            setup::commands::ollama_start,
            setup::commands::ollama_pull,
            setup::commands::ollama_benchmark,
            setup::commands::cancel_setup_task,
            setup::commands::complete_setup,
            target::commands::list_session_targets,
            target::commands::bind_terminal_by_focus,
            target::commands::bind_target,
            target::commands::unbind_target,
            target::commands::deliver_to_target
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Every real exit (tray Quit, no close-to-tray, uninstall) frees the model's memory.
            // Close-to-tray only hides the window, so it never gets here.
            if let tauri::RunEvent::Exit = event {
                system::commands::unload_model_on_exit(app);
            }
        });
}
