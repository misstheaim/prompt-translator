//! Settings model and SettingsStore (settings.json), plan §4.3 and T8.

pub mod commands;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::translation::model::{Mode, ProviderKind};

/// Event emitted (with the saved [`Settings`]) after every successful save.
pub const SETTINGS_CHANGED_EVENT: &str = "settings://changed";

const TIMEOUT_RANGE: std::ops::RangeInclusive<u32> = 5..=600;
const NUM_CTX_RANGE: std::ops::RangeInclusive<u32> = 2048..=32768;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct OllamaSettings {
    pub endpoint: String,
    pub model: String,
    pub keep_alive: String,
    pub num_ctx: u32,
    pub timeout_secs: u32,
}

impl Default for OllamaSettings {
    fn default() -> Self {
        Self {
            endpoint: "http://localhost:11434".into(),
            model: "qwen3.5:9b".into(),
            keep_alive: "30m".into(),
            num_ctx: 8192,
            timeout_secs: 60,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ClaudeSettings {
    /// `None` means "find claude.exe automatically".
    pub executable_path: Option<String>,
    pub model: String,
    pub timeout_secs: u32,
}

impl Default for ClaudeSettings {
    fn default() -> Self {
        Self {
            executable_path: None,
            model: "haiku".into(),
            timeout_secs: 90,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct LayoutFixSettings {
    pub enabled: bool,
    pub hotkey: String,
    pub switch_input_language: bool,
}

impl Default for LayoutFixSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            hotkey: "Ctrl+Alt+Space".into(),
            switch_input_language: true,
        }
    }
}

/// The contents of `settings.json`. Unknown or missing fields fall back to defaults.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub default_provider: ProviderKind,
    pub default_preset_id: String,
    pub ollama: OllamaSettings,
    pub claude: ClaudeSettings,
    pub history_enabled: bool,
    pub hotkey: String,
    pub autostart: bool,
    pub close_to_tray: bool,
    pub setup_completed: bool,
    pub auto_check_updates: bool,
    pub auto_copy_selection: bool,
    pub session_append_hint_shown: bool,
    pub last_popup_mode: Mode,
    pub dev_mock_provider: bool,
    pub layout_fix: LayoutFixSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_provider: ProviderKind::Ollama,
            default_preset_id: "technical-development".into(),
            ollama: OllamaSettings::default(),
            claude: ClaudeSettings::default(),
            history_enabled: true,
            hotkey: "Ctrl+Shift+Space".into(),
            autostart: false,
            close_to_tray: true,
            setup_completed: false,
            auto_check_updates: true,
            auto_copy_selection: true,
            session_append_hint_shown: false,
            last_popup_mode: Mode::Correct,
            dev_mock_provider: false,
            layout_fix: LayoutFixSettings::default(),
        }
    }
}

impl Settings {
    /// Checks the rules of T8; every violation is an `AppError::InvalidInput`.
    pub fn validate(&self) -> AppResult<()> {
        if !is_http_url(&self.ollama.endpoint) {
            return Err(invalid(
                "The Ollama endpoint must be a URL like http://localhost:11434",
            ));
        }
        if !TIMEOUT_RANGE.contains(&self.ollama.timeout_secs) {
            return Err(invalid("The Ollama timeout must be 5 to 600 seconds"));
        }
        if !TIMEOUT_RANGE.contains(&self.claude.timeout_secs) {
            return Err(invalid("The Claude timeout must be 5 to 600 seconds"));
        }
        if !NUM_CTX_RANGE.contains(&self.ollama.num_ctx) {
            return Err(invalid("The context size must be 2048 to 32768"));
        }
        if self.hotkey.trim().is_empty() {
            return Err(invalid("The hotkey must not be empty"));
        }
        if self.layout_fix.enabled && self.layout_fix.hotkey.trim().is_empty() {
            return Err(invalid("The layout fix hotkey must not be empty"));
        }
        Ok(())
    }
}

fn invalid(message: &str) -> AppError {
    AppError::InvalidInput(message.into())
}

/// An absolute `http://` or `https://` URL with a non-empty host.
fn is_http_url(value: &str) -> bool {
    let value = value.trim();
    let rest = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"));
    let Some(rest) = rest else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = authority.rsplit('@').next().unwrap_or("");
    !host.is_empty() && !host.starts_with(':') && !value.chars().any(char::is_whitespace)
}

/// Holds the current settings and persists them to one JSON file.
///
/// Built from a plain path, so it works without a Tauri `AppHandle`. The app registers it as
/// managed state; other modules read it with [`SettingsStore::get`]. Clones are handles to the
/// same settings (the translation service keeps one).
#[derive(Clone)]
pub struct SettingsStore {
    path: PathBuf,
    current: Arc<RwLock<Settings>>,
}

impl SettingsStore {
    /// Loads the file at `path`. Missing file: defaults. Corrupt file: renamed to
    /// `settings.json.bak` (next to it), logged, defaults used. Never fails.
    pub fn load(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let settings = read_or_default(&path);
        Self {
            path,
            current: Arc::new(RwLock::new(settings)),
        }
    }

    /// A copy of the current settings.
    pub fn get(&self) -> Settings {
        self.current
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Validates, writes atomically (temp file + rename), then replaces the in-memory value.
    /// Returns the saved settings.
    pub fn save(&self, settings: Settings) -> AppResult<Settings> {
        settings.validate()?;
        // Holding the write lock for the whole save keeps concurrent saves ordered.
        let mut current = self
            .current
            .write()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        write_atomic(&self.path, &settings)?;
        *current = settings.clone();
        Ok(settings)
    }
}

fn read_or_default(path: &Path) -> Settings {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
        Err(e) => {
            log::error!("Can't read {}: {e}; using default settings", path.display());
            return Settings::default();
        }
    };
    match serde_json::from_str::<Settings>(&text) {
        Ok(settings) => settings,
        Err(e) => {
            let backup = backup_path(path);
            match fs::rename(path, &backup) {
                Ok(()) => log::error!(
                    "{} is corrupt ({e}); moved to {} and using default settings",
                    path.display(),
                    backup.display()
                ),
                Err(rename_err) => log::error!(
                    "{} is corrupt ({e}) and could not be backed up ({rename_err}); \
                     using default settings",
                    path.display()
                ),
            }
            Settings::default()
        }
    }
}

/// `settings.json` -> `settings.json.bak`.
fn backup_path(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".bak");
    path.with_file_name(name)
}

fn write_atomic(path: &Path, settings: &Settings) -> AppResult<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(settings)?;
    let mut tmp_name = path.file_name().unwrap_or_default().to_os_string();
    tmp_name.push(".tmp");
    let tmp = path.with_file_name(tmp_name);
    fs::write(&tmp, json)?;
    if let Err(e) = fs::rename(&tmp, path) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}
