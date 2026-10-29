//! Tauri commands of the `setup` module.

use std::sync::Arc;

use tauri::{AppHandle, Emitter, State};

use super::environment::{self, EnvironmentReport};
use super::ollama_setup::{self, BenchmarkResult, PROGRESS_EVENT, SetupProgress, SetupTasks};
use crate::error::{AppError, AppResult};
use crate::presets::PresetStore;
use crate::settings::{SETTINGS_CHANGED_EVENT, SettingsStore};
use crate::translation::service::TranslationService;

/// What this machine has (GPU, RAM, Ollama, winget, Claude Code) and the recommended model tier.
/// Never fails: a check that can't run reports "not available".
#[tauri::command]
pub async fn environment_check(settings: State<'_, SettingsStore>) -> AppResult<EnvironmentReport> {
    Ok(environment::check(&settings.get()).await)
}

fn progress_emitter(app: &AppHandle) -> Arc<dyn Fn(SetupProgress) + Send + Sync> {
    let app = app.clone();
    Arc::new(move |progress| {
        let _ = app.emit(PROGRESS_EVENT, progress);
    })
}

/// Installs Ollama with winget; output lines arrive as `setup://progress` (phase `install`).
/// Without winget it fails with `InvalidInput`; the UI then opens the download page.
#[tauri::command]
pub async fn ollama_install(app: AppHandle, tasks: State<'_, SetupTasks>) -> AppResult<()> {
    let guard = tasks.begin()?;
    ollama_setup::install(guard.token.clone(), progress_emitter(&app)).await
}

/// Starts Ollama if it isn't running and waits (30 s) until it answers.
#[tauri::command]
pub async fn ollama_start(
    settings: State<'_, SettingsStore>,
    tasks: State<'_, SetupTasks>,
) -> AppResult<()> {
    let guard = tasks.begin()?;
    ollama_setup::start(
        &settings.get(),
        guard.token.clone(),
        ollama_setup::START_TIMEOUT,
    )
    .await
}

/// Downloads a model (progress as `setup://progress`, phase `download`) and makes it the
/// selected Ollama model.
#[tauri::command]
pub async fn ollama_pull(
    app: AppHandle,
    settings: State<'_, SettingsStore>,
    tasks: State<'_, SetupTasks>,
    model: String,
) -> AppResult<()> {
    let guard = tasks.begin()?;
    ollama_setup::pull(
        &settings.get(),
        &model,
        guard.token.clone(),
        progress_emitter(&app),
    )
    .await?;
    let mut current = settings.get();
    current.ollama.model = model.trim().to_owned();
    let saved = settings.save(current)?;
    app.emit(SETTINGS_CHANGED_EVENT, &saved)?;
    Ok(())
}

/// Times a Translate and a Correct run with `model` and reads the GPU offload from Ollama.
#[tauri::command]
pub async fn ollama_benchmark(
    settings: State<'_, SettingsStore>,
    presets: State<'_, PresetStore>,
    service: State<'_, TranslationService>,
    model: String,
) -> AppResult<BenchmarkResult> {
    let ids: Vec<String> = presets.list().into_iter().map(|p| p.id).collect();
    let preset_id = ollama_setup::bench_preset(&ids)
        .ok_or_else(|| AppError::InvalidInput("There is no preset to run the test with".into()))?;
    ollama_setup::benchmark(&service, &settings.get(), &preset_id, &model).await
}

/// Cancels the running install wait or model download (does nothing when none runs).
#[tauri::command]
pub fn cancel_setup_task(tasks: State<'_, SetupTasks>) {
    tasks.cancel();
}

/// Marks the first-run setup as done.
#[tauri::command]
pub fn complete_setup(app: AppHandle, settings: State<'_, SettingsStore>) -> AppResult<()> {
    let mut current = settings.get();
    current.setup_completed = true;
    let saved = settings.save(current)?;
    app.emit(SETTINGS_CHANGED_EVENT, &saved)?;
    Ok(())
}
