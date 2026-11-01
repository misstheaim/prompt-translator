//! Preset model and PresetStore (`presets/<id>.json`), plan §4.3 and T9.

pub mod commands;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::translation::model::Mode;

const MAX_RULES: usize = 30;
const MAX_RULE_CHARS: usize = 500;

/// The bundled presets (`src-tauri/resources/presets/`), embedded at compile time so the store
/// needs neither an `AppHandle` nor resource paths.
const BUILT_INS: [(&str, &str); 9] = [
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
    (
        "grumpy-senior-dev",
        include_str!("../../resources/presets/grumpy-senior-dev.json"),
    ),
    (
        "passive-aggressive",
        include_str!("../../resources/presets/passive-aggressive.json"),
    ),
    (
        "pirate-captain",
        include_str!("../../resources/presets/pirate-captain.json"),
    ),
    (
        "shakespearean",
        include_str!("../../resources/presets/shakespearean.json"),
    ),
    (
        "startup-founder",
        include_str!("../../resources/presets/startup-founder.json"),
    ),
];

/// The built-ins shipped before the seed record existed (T64): an install that has preset files
/// but no record has seeded exactly these.
const ORIGINAL_BUILT_INS: [&str; 4] = [
    "general",
    "technical-development",
    "code-review",
    "work-communication",
];

/// The file in the presets folder that lists the built-in ids already seeded (a JSON array of
/// strings). Without a `.json` extension, so [`PresetStore::list`] never reads it as a preset.
const SEED_RECORD: &str = ".seeded";

fn default_true() -> bool {
    true
}

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
    /// True for the bundled presets (also after the user edits them).
    #[serde(default)]
    pub built_in: bool,
    /// Whether results made with this preset are saved to history (T64). Missing in files
    /// written before T64: saved.
    #[serde(default = "default_true")]
    pub save_to_history: bool,
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
/// directory, so edits made outside the app are picked up. Clones are handles to the same store
/// (the translation service keeps one).
#[derive(Clone)]
pub struct PresetStore {
    dir: PathBuf,
    /// Serializes writes (save, delete, reset), shared by all clones.
    write_lock: Arc<Mutex<()>>,
}

impl PresetStore {
    /// Opens the store at `dir` and copies in every bundled preset that was never seeded there
    /// (all of them on the first run; the ones added by an update later). A built-in the user
    /// deleted is not brought back. A failure to seed is logged, not fatal.
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        let store = Self {
            dir: dir.into(),
            write_lock: Arc::new(Mutex::new(())),
        };
        if let Err(e) = store.seed_missing() {
            log::error!("Can't seed presets in {}: {e}", store.dir.display());
        }
        store
    }

    /// Seeds the built-ins missing from the seed record, then updates the record. Without a
    /// record, an empty folder is a first run (nothing seeded yet) and a folder with preset
    /// files is an install from before the record (the four original built-ins seeded).
    fn seed_missing(&self) -> AppResult<()> {
        fs::create_dir_all(&self.dir)?;
        let record = self.read_seed_record();
        let mut seeded: Vec<String> = match &record {
            Some(ids) => ids.clone(),
            None if self.json_files().is_empty() => Vec::new(),
            None => ORIGINAL_BUILT_INS
                .iter()
                .map(|id| (*id).to_string())
                .collect(),
        };
        let mut changed = record.is_none();
        for preset in built_in_presets() {
            if seeded.contains(&preset.id) {
                continue;
            }
            // A preset file with this id already there (made by the user) is kept as it is.
            if !self.file_for(&preset.id).exists() {
                self.write(&preset)?;
            }
            seeded.push(preset.id);
            changed = true;
        }
        if changed {
            self.write_seed_record(&seeded)?;
        }
        Ok(())
    }

    /// The ids in the seed record; `None` when there is no record or it can't be read.
    fn read_seed_record(&self) -> Option<Vec<String>> {
        let path = self.dir.join(SEED_RECORD);
        let text = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(e) => {
                log::warn!("Can't read {}: {e}", path.display());
                return None;
            }
        };
        match serde_json::from_str(&text) {
            Ok(ids) => Some(ids),
            Err(e) => {
                log::warn!("Ignoring {}: invalid JSON ({e})", path.display());
                None
            }
        }
    }

    /// Atomic write of the seed record: temp file + rename.
    fn write_seed_record(&self, ids: &[String]) -> AppResult<()> {
        let path = self.dir.join(SEED_RECORD);
        let tmp = self.dir.join(format!("{SEED_RECORD}.tmp"));
        fs::write(&tmp, serde_json::to_string_pretty(ids)?)?;
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(e.into());
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

    /// Restores all bundled presets and keeps user-created ones. Returns the full list.
    pub fn reset(&self) -> AppResult<Vec<Preset>> {
        let _guard = self.lock();
        fs::create_dir_all(&self.dir)?;
        let mut seeded = Vec::new();
        for preset in built_in_presets() {
            self.write(&preset)?;
            seeded.push(preset.id);
        }
        // Every built-in is there now; a later start must not see any of them as never seeded.
        if let Err(e) = self.write_seed_record(&seeded) {
            log::warn!("Can't update the preset seed record: {e}");
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
