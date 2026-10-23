//! The single source of every app path (plan D14, D27).
//!
//! Everything lives under `app_config_dir()` (settings, presets) or `app_local_data_dir()`
//! (history, logs, Claude working dirs, temp files). `app_data_dir()` is never used: on Windows
//! it is the Roaming folder. Each accessor creates its directory on first use.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

struct Roots {
    config: PathBuf,
    local: PathBuf,
}

static ROOTS: OnceLock<Roots> = OnceLock::new();

/// Resolves the two root directories from the app handle. Call once at startup.
pub fn init(app: &AppHandle) -> AppResult<()> {
    let config = app.path().app_config_dir()?;
    let local = app.path().app_local_data_dir()?;
    // A second call keeps the first value, which is fine: the roots never change at runtime.
    let _ = ROOTS.set(Roots { config, local });
    Ok(())
}

fn roots() -> AppResult<&'static Roots> {
    ROOTS
        .get()
        .ok_or_else(|| AppError::Io("app paths are not initialized".into()))
}

fn ensure(dir: PathBuf) -> AppResult<PathBuf> {
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn ensure_parent(file: &Path) -> AppResult<()> {
    if let Some(parent) = file.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// `app_config_dir()`: settings and presets.
pub fn config_dir() -> AppResult<PathBuf> {
    ensure(roots()?.config.clone())
}

/// `app_local_data_dir()`: everything that must not roam.
pub fn local_dir() -> AppResult<PathBuf> {
    ensure(roots()?.local.clone())
}

pub fn settings_file() -> AppResult<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

pub fn presets_dir() -> AppResult<PathBuf> {
    ensure(config_dir()?.join("presets"))
}

pub fn history_db() -> AppResult<PathBuf> {
    let file = local_dir()?.join("history.db");
    ensure_parent(&file)?;
    Ok(file)
}

pub fn logs_dir() -> AppResult<PathBuf> {
    ensure(local_dir()?.join("logs"))
}

/// Isolated `CLAUDE_CONFIG_DIR` for the translator's `claude -p` runs.
pub fn claude_config_dir() -> AppResult<PathBuf> {
    ensure(local_dir()?.join("claude-config"))
}

/// Fixed, empty working directory for `claude -p` runs.
pub fn claude_cwd() -> AppResult<PathBuf> {
    ensure(local_dir()?.join("claude-cwd"))
}

pub fn tmp_dir() -> AppResult<PathBuf> {
    ensure(local_dir()?.join("tmp"))
}
