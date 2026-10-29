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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // Must be the first plugin: a second launch focuses the main window and exits.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            system::tray::show_main_window(app);
        }))
        // Registered from Rust only; the webview gets no autostart permission.
        .plugin(system::autostart::plugin())
        .on_window_event(system::tray::on_window_event)
        // Used from Rust only (`copy_result`); the webview gets no clipboard permission.
        .plugin(tauri_plugin_clipboard_manager::init())
        // Used from Rust only (`pick_file`); the webview gets no dialog permission.
        .plugin(tauri_plugin_dialog::init())
        // Used from Rust only (`open_url`, allow-listed URLs); the webview gets no opener permission.
        .plugin(tauri_plugin_opener::init())
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

            system::tray::init(app.handle())?;
            system::autostart::init(app.handle());
            // Started by the autostart entry: stay in the tray (the main window is created
            // hidden; no popup exists at startup either).
            if !system::autostart::launched_by_autostart() {
                system::tray::show_main_window(app.handle());
            }
            Ok(())
        })
        // One registration line per module.
        .invoke_handler(tauri::generate_handler![
            system::commands::ping,
            system::commands::pick_file,
            system::commands::open_url,
            settings::commands::get_settings,
            settings::commands::save_settings,
            settings::commands::set_last_popup_mode,
            translation::commands::detect_mode,
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
            history::commands::mistakes_summary
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
