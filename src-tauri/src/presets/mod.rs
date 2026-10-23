//! Preset model and PresetStore (`presets/<id>.json`), plan §4.3 and T9.

pub mod commands;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::translation::model::Mode;

const MAX_RULES: usize = 30;
const MAX_RULE_CHARS: usize = 500;

/// The bundled presets (`src-tauri/resources/presets/`), embedded at compile time so the store
/// needs neither an `AppHandle` nor resource paths.
const BUILT_INS: [(&str, &str); 4] = [
    (
        "general",
        include_str!("../../resources/presets/general.json"),
    ),
    (
        "technical-development",
        include_str!("../../resources/presets/technical-development.json"),
    ),
    (
        "code-review",
        include_str!("../../resources/presets/code-review.json"),
    ),
    (
        "work-communication",
        include_str!("../../resources/presets/work-communication.json"),
    ),
];

/// Domain rules plus a default mode (plan D7).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Preset {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub default_mode: Mode,
    #[serde(default)]
    pub rules: Vec<String>,
    /// True for the four bundled presets (also after the user edits them).
    #[serde(default)]
    pub built_in: bool,
}

impl Preset {
    /// Checks the rules of T9; every violation is an `AppError::InvalidInput`.
    pub fn validate(&self) -> AppResult<()> {
        let slug_ok = !self.id.is_empty()
            && self
                .id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !slug_ok {
            return Err(invalid(
                "The preset id may only contain lowercase letters, digits and hyphens",
            ));
        }
        if self.name.trim().is_empty() {
            return Err(invalid("The preset name must not be empty"));
        }
        if self.rules.len() > MAX_RULES {
            return Err(invalid("A preset can have at most 30 rules"));
        }
        if self
            .rules
            .iter()
            .any(|r| r.chars().count() > MAX_RULE_CHARS)
        {
            return Err(invalid("A rule can have at most 500 characters"));
        }
        Ok(())
    }
}

fn invalid(message: &str) -> AppError {
    AppError::InvalidInput(message.into())
}

fn built_in_presets() -> Vec<Preset> {
    BUILT_INS
        .iter()
        .map(|(id, json)| {
            let mut preset: Preset = serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("bundled preset {id} is invalid: {e}"));
            preset.built_in = true;
            preset
        })
        .collect()
}

fn is_built_in_id(id: &str) -> bool {
    BUILT_INS.iter().any(|(built_in, _)| *built_in == id)
}

/// Reads and writes the preset files in one directory.
///
/// Built from a plain path, so it works without a Tauri `AppHandle`. Every call reads the
/// directory, so edits made outside the app are picked up.
pub struct PresetStore {
    dir: PathBuf,
    /// Serializes writes (save, delete, reset).
    write_lock: Mutex<()>,
}

impl PresetStore {
    /// Opens the store at `dir`. When the directory has no preset files yet (first run), the
    /// bundled presets are copied in. A failure to seed is logged, not fatal.
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        let store = Self {
            dir: dir.into(),
            write_lock: Mutex::new(()),
        };
        if let Err(e) = store.seed_if_empty() {
            log::error!("Can't seed presets in {}: {e}", store.dir.display());
        }
        store
    }

    fn seed_if_empty(&self) -> AppResult<()> {
        fs::create_dir_all(&self.dir)?;
        if !self.json_files().is_empty() {
            return Ok(());
        }
        for preset in built_in_presets() {
            self.write(&preset)?;
        }
        Ok(())
    }

    fn json_files(&self) -> Vec<PathBuf> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.is_file() && p.extension().is_some_and(|ext| ext == "json"))
            .collect()
    }

    fn file_for(&self, id: &str) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// All valid presets sorted by name. An unreadable or invalid file is skipped with a
    /// logged warning.
    pub fn list(&self) -> Vec<Preset> {
        let mut presets: Vec<Preset> = self
            .json_files()
            .iter()
            .filter_map(|path| read_preset(path))
            .collect();
        presets.sort_by_cached_key(|p| (p.name.to_lowercase(), p.id.clone()));
        presets
    }

    pub fn get(&self, id: &str) -> Option<Preset> {
        // The id becomes a file name, so refuse anything but a slug.
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
        read_preset(&self.file_for(id))
    }

    /// Validates and writes the preset (new or edited). Returns the full list.
    pub fn save(&self, mut preset: Preset) -> AppResult<Vec<Preset>> {
        preset.validate()?;
        preset.built_in = is_built_in_id(&preset.id);
        let _guard = self.lock();
        self.write(&preset)?;
        Ok(self.list())
    }

    /// Deletes a preset. Refuses to delete the last remaining one. Returns the full list.
    pub fn delete(&self, id: &str) -> AppResult<Vec<Preset>> {
        let _guard = self.lock();
        let presets = self.list();
        let Some(target) = presets.iter().find(|p| p.id == id) else {
            return Err(invalid("This preset does not exist"));
        };
        if presets.len() <= 1 {
            return Err(invalid("You can't delete the last preset"));
        }
        fs::remove_file(self.file_for(&target.id))?;
        Ok(self.list())
    }

    /// Restores the four bundled presets and keeps user-created ones. Returns the full list.
    pub fn reset(&self) -> AppResult<Vec<Preset>> {
        let _guard = self.lock();
        fs::create_dir_all(&self.dir)?;
        for preset in built_in_presets() {
            self.write(&preset)?;
        }
        Ok(self.list())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ()> {
        self.write_lock
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Atomic write: temp file + rename.
    fn write(&self, preset: &Preset) -> AppResult<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.file_for(&preset.id);
        let tmp = self.dir.join(format!("{}.json.tmp", preset.id));
        fs::write(&tmp, serde_json::to_string_pretty(preset)?)?;
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(e.into());
        }
        Ok(())
    }
}

fn read_preset(path: &Path) -> Option<Preset> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) => {
            log::warn!(
                "Skipping preset file {}: can't read it ({e})",
                path.display()
            );
            return None;
        }
    };
    let preset: Preset = match serde_json::from_str(&text) {
        Ok(preset) => preset,
        Err(e) => {
            log::warn!(
                "Skipping preset file {}: invalid JSON ({e})",
                path.display()
            );
            return None;
        }
    };
    if let Err(e) = preset.validate() {
        log::warn!("Skipping preset file {}: {e}", path.display());
        return None;
    }
    if path.file_stem().and_then(|s| s.to_str()) != Some(preset.id.as_str()) {
        log::warn!(
            "Skipping preset file {}: the id \"{}\" doesn't match the file name",
            path.display(),
            preset.id
        );
        return None;
    }
    Some(preset)
}
