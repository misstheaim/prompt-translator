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
        // Used from Rust only (`copy_result`); the webview gets no clipboard permission.
        .plugin(tauri_plugin_clipboard_manager::init())
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

            let settings = settings::SettingsStore::load(system::paths::settings_file()?);
            let presets = presets::PresetStore::open(system::paths::presets_dir()?);
            // The service shares the stores with their commands (clones are handles).
            app.manage(translation::service::TranslationService::new(
                settings.clone(),
                presets.clone(),
                translation::providers::registry(),
            ));
            app.manage(settings);
            app.manage(presets);
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
            translation::commands::translate,
            translation::commands::cancel_translation,
            translation::commands::provider_status,
            translation::commands::copy_result,
            presets::commands::list_presets,
            presets::commands::save_preset,
            presets::commands::delete_preset,
            presets::commands::reset_presets
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
