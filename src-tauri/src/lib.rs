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

            app.manage(settings::SettingsStore::load(
                system::paths::settings_file()?
            ));
            app.manage(presets::PresetStore::open(system::paths::presets_dir()?));
            // History must never stop the app: without the database, saving is skipped.
            match history::HistoryDb::open(&system::paths::history_db()?) {
                Ok(db) => {
                    app.manage(db);
                }
                Err(e) => log::warn!("History database unavailable: {e}"),
            }
            Ok(())
        })
        // One registration line per module.
        .invoke_handler(tauri::generate_handler![
            system::commands::ping,
            settings::commands::get_settings,
            settings::commands::save_settings,
            settings::commands::set_last_popup_mode,
            translation::commands::detect_mode,
            presets::commands::list_presets,
            presets::commands::save_preset,
            presets::commands::delete_preset,
            presets::commands::reset_presets
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
