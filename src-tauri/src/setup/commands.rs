//! Tauri commands of the `setup` module.

use tauri::State;

use super::environment::{self, EnvironmentReport};
use crate::error::AppResult;
use crate::settings::SettingsStore;

/// What this machine has (GPU, RAM, Ollama, winget, Claude Code) and the recommended model tier.
/// Never fails: a check that can't run reports "not available".
#[tauri::command]
pub async fn environment_check(settings: State<'_, SettingsStore>) -> AppResult<EnvironmentReport> {
    Ok(environment::check(&settings.get()).await)
}
