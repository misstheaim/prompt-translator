//! Ollama setup (plan T35): install through winget, start the app, download a model, benchmark it.
//!
//! The logic lives here and reports progress through a callback, so it doesn't depend on a Tauri
//! `AppHandle`; `setup::commands` wires it to the `setup://progress` event. Ollama is used for
//! text generation only: nothing here sends `tools` to `/api/chat` (D7).

use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use ts_rs::TS;

use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::translation::model::{Mode, ProviderKind, TranslationRequest};
use crate::translation::providers::ollama::same_model;
use crate::translation::service::TranslationService;

/// The event carrying [`SetupProgress`].
pub const PROGRESS_EVENT: &str = "setup://progress";
/// How long `ollama_start` waits for Ollama to answer.
pub const START_TIMEOUT: Duration = Duration::from_secs(30);
const START_POLL_INTERVAL: Duration = Duration::from_millis(500);
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
/// A model download that sends nothing for this long is treated as failed.
const PULL_STALL_TIMEOUT: Duration = Duration::from_secs(120);
/// Progress events of the download are sent at most this often (status changes always are).
const PROGRESS_INTERVAL: Duration = Duration::from_millis(150);
const ERROR_EXCERPT_CHARS: usize = 300;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const DETACHED_PROCESS: u32 = 0x0000_0008;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// winget exit codes that mean "Ollama is already there".
const WINGET_ALREADY_INSTALLED: [i32; 2] = [
    -1_978_335_135, // 0x8A150061 package already installed
    -1_978_335_189, // 0x8A15002B no applicable upgrade
];

/// The fixed prompts of the benchmark.
const BENCH_TRANSLATE_TEXT: &str = "Посмотри, пожалуйста, почему падает сборка после последнего \
                                    коммита в ветке develop и исправь ошибку в файле config.rs.";
const BENCH_CORRECT_TEXT: &str = "Please check why the build is faling after the last commit \
                                  in develop branch and fix a error in the file config.rs, \
                                  I think the problem it is in the paths handling.";
/// The preset used when the user has deleted `general`.
const BENCH_PRESET: &str = "general";

// ---- progress contract ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum SetupPhase {
    Install,
    Download,
}

/// Payload of `setup://progress`. The install phase fills `message`; the download phase fills
/// `status` and the byte counts (summed over all layers of the model).
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SetupProgress {
    pub phase: SetupPhase,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub completed_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional, type = "number")]
    pub total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub status: Option<String>,
}

/// What `ollama_benchmark` returns.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BenchmarkResult {
    #[ts(type = "number")]
    pub translate_ms: u64,
    #[ts(type = "number")]
    pub correct_ms: u64,
    /// Share of the loaded model that sits in GPU memory (`size_vram / size`), 0 to 100.
    pub gpu_offload_percent: u32,
}

// ---- the running task ----

/// The one install / download that may run at a time, so `cancel_setup_task` knows what to stop.
#[derive(Default)]
pub struct SetupTasks {
    current: Mutex<Option<CancellationToken>>,
}

/// Holds the task slot; dropping it frees the slot.
pub struct TaskGuard<'a> {
    tasks: &'a SetupTasks,
    pub token: CancellationToken,
}

impl SetupTasks {
    /// Claims the slot, or fails when another setup task is running.
    pub fn begin(&self) -> AppResult<TaskGuard<'_>> {
        let mut current = lock(&self.current);
        if current.is_some() {
            return Err(AppError::InvalidInput(
                "Another setup step is still running".into(),
            ));
        }
        let token = CancellationToken::new();
        *current = Some(token.clone());
        Ok(TaskGuard { tasks: self, token })
    }

    /// Cancels the running install or download (does nothing when none runs).
    pub fn cancel(&self) {
        if let Some(token) = lock(&self.current).as_ref() {
            token.cancel();
        }
    }
}

impl Drop for TaskGuard<'_> {
    fn drop(&mut self) {
        *lock(&self.tasks.current) = None;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn endpoint(settings: &Settings) -> &str {
    settings.ollama.endpoint.trim().trim_end_matches('/')
}

// ---- install ----

/// Runs `winget install` for Ollama and reports its output lines.
pub async fn install(
    cancel: CancellationToken,
    emit: Arc<dyn Fn(SetupProgress) + Send + Sync>,
) -> AppResult<()> {
    let mut cmd = Command::new("winget");
    cmd.args([
        "install",
        "-e",
        "--id",
        "Ollama.Ollama",
        "--accept-package-agreements",
        "--accept-source-agreements",
        "--silent",
    ])
    .stdin(Stdio::null())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            winget_missing()
        } else {
            AppError::Io(format!("Couldn't start winget: {e}"))
        }
    })?;

    let tail = Arc::new(Mutex::new(Vec::<String>::new()));
    let mut readers = Vec::new();
    if let Some(out) = child.stdout.take() {
        readers.push(tokio::spawn(pump(out, emit.clone(), tail.clone())));
    }
    if let Some(err) = child.stderr.take() {
        readers.push(tokio::spawn(pump(err, emit.clone(), tail.clone())));
    }

    let status = tokio::select! {
        biased;
        () = cancel.cancelled() => {
            let _ = child.kill().await;
            for reader in &readers {
                reader.abort();
            }
            return Err(AppError::Cancelled);
        }
        status = child.wait() => status?,
    };
    for reader in readers {
        let _ = reader.await;
    }

    match status.code() {
        Some(0) => Ok(()),
        Some(code) if WINGET_ALREADY_INSTALLED.contains(&code) => Ok(()),
        code => {
            let last = lock(&tail).last().cloned().unwrap_or_default();
            Err(AppError::Io(winget_failure_message(code, &last)))
        }
    }
}

fn winget_missing() -> AppError {
    AppError::InvalidInput(
        "winget isn't available on this PC. Download Ollama from https://ollama.com/download \
         and install it manually."
            .into(),
    )
}

fn winget_failure_message(code: Option<i32>, last_line: &str) -> String {
    let code = code.map_or("none".to_owned(), |c| format!("{c:#010x}"));
    if last_line.is_empty() {
        format!("The Ollama installation failed (winget exit code {code}).")
    } else {
        format!("The Ollama installation failed (winget exit code {code}): {last_line}")
    }
}

/// Reads a process stream, reports each output line, and keeps the last few in `tail`.
async fn pump<R: AsyncRead + Unpin>(
    mut reader: R,
    emit: Arc<dyn Fn(SetupProgress) + Send + Sync>,
    tail: Arc<Mutex<Vec<String>>>,
) {
    let mut pending = Vec::<u8>::new();
    let mut buf = [0u8; 4096];
    loop {
        let n = match reader.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        pending.extend_from_slice(&buf[..n]);
        // winget redraws its progress bar with `\r`, so both `\r` and `\n` end a line.
        while let Some(pos) = pending.iter().position(|&b| b == b'\n' || b == b'\r') {
            let line: Vec<u8> = pending.drain(..=pos).collect();
            report_line(&line, &emit, &tail);
        }
    }
    if !pending.is_empty() {
        report_line(&pending, &emit, &tail);
    }
}

fn report_line(
    raw: &[u8],
    emit: &Arc<dyn Fn(SetupProgress) + Send + Sync>,
    tail: &Arc<Mutex<Vec<String>>>,
) {
    let Some(line) = clean_output_line(&String::from_utf8_lossy(raw)) else {
        return;
    };
    {
        let mut tail = lock(tail);
        tail.push(line.clone());
        if tail.len() > 5 {
            tail.remove(0);
        }
    }
    emit(SetupProgress {
        phase: SetupPhase::Install,
        message: Some(line),
        completed_bytes: None,
        total_bytes: None,
        status: None,
    });
}

/// Trims a winget output line; `None` for blanks and the spinner characters (`-`, `\`, `|`, `/`).
fn clean_output_line(raw: &str) -> Option<String> {
    let line = raw.trim();
    if line.chars().count() <= 1 {
        return None;
    }
    Some(line.to_owned())
}

// ---- start ----

/// Starts Ollama when it isn't answering and waits until `/api/tags` responds (30 s).
pub async fn start(
    settings: &Settings,
    cancel: CancellationToken,
    timeout: Duration,
) -> AppResult<()> {
    let client = reqwest::Client::new();
    let url = format!("{}/api/tags", endpoint(settings));
    if probe(&client, &url).await {
        return Ok(());
    }
    launch_ollama()?;
    poll_until(
        || probe(&client, &url),
        timeout,
        START_POLL_INTERVAL,
        &cancel,
    )
    .await
}

async fn probe(client: &reqwest::Client, url: &str) -> bool {
    match client.get(url).timeout(PROBE_TIMEOUT).send().await {
        Ok(response) => response.status().is_success(),
        Err(_) => false,
    }
}

/// Calls `probe` every `interval` until it returns true. Errors: `Cancelled`, or `Io` when
/// `timeout` passes first.
async fn poll_until<F, Fut>(
    mut probe: F,
    timeout: Duration,
    interval: Duration,
    cancel: &CancellationToken,
) -> AppResult<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    loop {
        if probe().await {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(AppError::Io(format!(
                "Ollama didn't start within {} seconds",
                timeout.as_secs()
            )));
        }
        tokio::select! {
            biased;
            () = cancel.cancelled() => return Err(AppError::Cancelled),
            () = tokio::time::sleep(interval) => {}
        }
    }
}

fn local_ollama_dir() -> Option<PathBuf> {
    std::env::var_os("LOCALAPPDATA").map(|d| PathBuf::from(d).join("Programs").join("Ollama"))
}

/// Launches the tray app (`ollama app.exe`), or `ollama serve` when only the CLI exists. The
/// process is detached and the app never stops it (D10).
fn launch_ollama() -> AppResult<()> {
    let dir = local_ollama_dir();
    let app = dir.as_ref().map(|d| d.join("ollama app.exe"));
    let (program, args): (PathBuf, &[&str]) = if let Some(app) = app.filter(|p| p.is_file()) {
        (app, &[])
    } else if let Some(cli) = find_ollama_cli(dir) {
        (cli, &["serve"])
    } else {
        return Err(AppError::InvalidInput(
            "Ollama isn't installed. Install it first.".into(),
        ));
    };
    let mut cmd = std::process::Command::new(&program);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    }
    cmd.spawn()
        .map(drop)
        .map_err(|e| AppError::Io(format!("Couldn't start Ollama: {e}")))
}

fn find_ollama_cli(local_dir: Option<PathBuf>) -> Option<PathBuf> {
    let default = local_dir.map(|d| d.join("ollama.exe"));
    if let Some(path) = default.filter(|p| p.is_file()) {
        return Some(path);
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join("ollama.exe"))
        .find(|p| p.is_file())
}

// ---- pull ----

/// One parsed line of the `/api/pull` stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PullLine {
    /// A status line, with the layer's byte counts when it carries them.
    Status {
        status: String,
        digest: Option<String>,
        total: Option<u64>,
        completed: Option<u64>,
    },
    /// The model is fully downloaded.
    Success,
    /// Ollama reported an error (for example an unknown model).
    Error(String),
}

/// Parses one NDJSON line; `None` for blank or unreadable lines.
pub fn parse_pull_line(line: &str) -> Option<PullLine> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }
    let value: Value = serde_json::from_str(line).ok()?;
    if let Some(error) = value.get("error").and_then(Value::as_str) {
        return Some(PullLine::Error(error.to_owned()));
    }
    let status = value.get("status")?.as_str()?.to_owned();
    if status == "success" {
        return Some(PullLine::Success);
    }
    Some(PullLine::Status {
        digest: value
            .get("digest")
            .and_then(Value::as_str)
            .map(str::to_owned),
        total: value.get("total").and_then(Value::as_u64),
        completed: value.get("completed").and_then(Value::as_u64),
        status,
    })
}

/// Sums the byte counts over all layers: Ollama reports each layer separately.
#[derive(Default)]
struct PullTracker {
    layers: HashMap<String, (u64, u64)>,
}

impl PullTracker {
    fn update(&mut self, digest: &str, total: Option<u64>, completed: Option<u64>) {
        let entry = self.layers.entry(digest.to_owned()).or_default();
        if let Some(total) = total {
            entry.0 = total;
        }
        if let Some(completed) = completed {
            entry.1 = completed;
        }
    }

    /// `(completed, total)` over all layers seen so far.
    fn sums(&self) -> (u64, u64) {
        self.layers
            .values()
            .fold((0, 0), |(c, t), &(total, completed)| {
                (c + completed.min(total), t + total)
            })
    }
}

/// Downloads `model`. Errors: `OllamaUnreachable`, `Cancelled`, `Io` (Ollama's message, a
/// stalled or broken stream).
pub async fn pull(
    settings: &Settings,
    model: &str,
    cancel: CancellationToken,
    emit: Arc<dyn Fn(SetupProgress) + Send + Sync>,
) -> AppResult<()> {
    let model = model.trim();
    if model.is_empty() || model.chars().any(char::is_whitespace) {
        return Err(AppError::InvalidInput("Enter a valid model name".into()));
    }
    let url = format!("{}/api/pull", endpoint(settings));
    let client = reqwest::Client::new();
    let work = pull_stream(&client, &url, model, &emit);
    tokio::select! {
        biased;
        () = cancel.cancelled() => Err(AppError::Cancelled),
        result = work => result,
    }
}

async fn pull_stream(
    client: &reqwest::Client,
    url: &str,
    model: &str,
    emit: &Arc<dyn Fn(SetupProgress) + Send + Sync>,
) -> AppResult<()> {
    let mut response = client
        .post(url)
        .json(&json!({ "model": model, "stream": true }))
        .send()
        .await
        .map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                AppError::OllamaUnreachable
            } else {
                AppError::Io(format!("The download request failed: {e}"))
            }
        })?;
    let http_status = response.status();
    if !http_status.is_success() {
        let body = response.text().await.unwrap_or_default();
        return Err(AppError::Io(format!(
            "Ollama refused the download (HTTP {}): {}",
            http_status.as_u16(),
            excerpt(&body)
        )));
    }

    let mut tracker = PullTracker::default();
    let mut pending = Vec::<u8>::new();
    let mut last_emit: Option<Instant> = None;
    let mut last_status = String::new();
    let mut succeeded = false;
    loop {
        let chunk = match tokio::time::timeout(PULL_STALL_TIMEOUT, response.chunk()).await {
            Err(_) => {
                return Err(AppError::Io(
                    "The download stalled: Ollama sent nothing for two minutes".into(),
                ));
            }
            Ok(Err(e)) => return Err(AppError::Io(format!("The download was interrupted: {e}"))),
            Ok(Ok(chunk)) => chunk,
        };
        let Some(chunk) = chunk else { break };
        pending.extend_from_slice(&chunk);
        while let Some(pos) = pending.iter().position(|&b| b == b'\n') {
            let line: Vec<u8> = pending.drain(..=pos).collect();
            let Some(parsed) = parse_pull_line(&String::from_utf8_lossy(&line)) else {
                continue;
            };
            match parsed {
                PullLine::Error(message) => {
                    return Err(AppError::Io(format!(
                        "Couldn't download {model}: {message}"
                    )));
                }
                PullLine::Success => succeeded = true,
                PullLine::Status {
                    status,
                    digest,
                    total,
                    completed,
                } => {
                    if let Some(digest) = digest {
                        tracker.update(&digest, total, completed);
                    }
                    let changed = status != last_status;
                    let due = last_emit.is_none_or(|t| t.elapsed() >= PROGRESS_INTERVAL);
                    if changed || due {
                        let (done, all) = tracker.sums();
                        emit(download_progress(status.clone(), done, all));
                        last_emit = Some(Instant::now());
                        last_status = status;
                    }
                }
            }
        }
    }
    if !succeeded {
        // A stream that ends without `success` was cut off.
        return Err(AppError::Io(
            "The download ended before Ollama reported success. Retry to resume it.".into(),
        ));
    }
    let (_, all) = tracker.sums();
    emit(download_progress("success".into(), all, all));
    Ok(())
}

fn download_progress(status: String, completed: u64, total: u64) -> SetupProgress {
    SetupProgress {
        phase: SetupPhase::Download,
        message: None,
        completed_bytes: Some(completed),
        total_bytes: Some(total),
        status: Some(status),
    }
}

fn excerpt(text: &str) -> String {
    text.trim().chars().take(ERROR_EXCERPT_CHARS).collect()
}

// ---- benchmark ----

/// Runs a Translate and a Correct prompt through the service with `model` and measures the GPU
/// offload. A first (untimed) Translate run loads the model.
pub async fn benchmark(
    service: &TranslationService,
    settings: &Settings,
    preset_id: &str,
    model: &str,
) -> AppResult<BenchmarkResult> {
    let mut bench_settings = settings.clone();
    bench_settings.ollama.model = model.trim().to_owned();
    // The benchmark prompts don't belong in the user's history.
    bench_settings.history_enabled = false;

    let run = |mode: Mode, text: &str| {
        let req = TranslationRequest {
            request_id: uuid::Uuid::new_v4().to_string(),
            source_text: text.to_owned(),
            mode,
            provider: ProviderKind::Ollama,
            preset_id: preset_id.to_owned(),
        };
        service.translate_with(req, bench_settings.clone())
    };

    // Warm-up: loads the model into memory, not timed.
    run(Mode::Translate, BENCH_TRANSLATE_TEXT).await?;
    let translate = run(Mode::Translate, BENCH_TRANSLATE_TEXT).await?;
    let correct = run(Mode::Correct, BENCH_CORRECT_TEXT).await?;

    let gpu_offload_percent = gpu_offload(settings, model).await;
    Ok(BenchmarkResult {
        translate_ms: translate.duration_ms,
        correct_ms: correct.duration_ms,
        gpu_offload_percent,
    })
}

/// The preset the benchmark uses: `general`, or the first one when it was deleted.
pub fn bench_preset(ids: &[String]) -> Option<String> {
    if ids.iter().any(|id| id == BENCH_PRESET) {
        Some(BENCH_PRESET.to_owned())
    } else {
        ids.first().cloned()
    }
}

/// `GET /api/ps`: how much of the loaded model is in VRAM. 0 when the model isn't listed or
/// the call fails (the wizard then treats the machine as CPU-only).
async fn gpu_offload(settings: &Settings, model: &str) -> u32 {
    let url = format!("{}/api/ps", endpoint(settings));
    let body: Option<Value> = async {
        reqwest::Client::new()
            .get(url)
            .timeout(PROBE_TIMEOUT)
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?
            .json()
            .await
            .ok()
    }
    .await;
    match body {
        Some(body) => gpu_offload_from_ps(&body, model),
        None => {
            log::warn!("/api/ps unavailable; reporting 0% GPU offload");
            0
        }
    }
}

/// `size_vram / size × 100` for `model` in an `/api/ps` body.
pub fn gpu_offload_from_ps(ps: &Value, model: &str) -> u32 {
    let entry = ps
        .get("models")
        .and_then(Value::as_array)
        .and_then(|models| {
            models.iter().find(|m| {
                ["name", "model"]
                    .iter()
                    .filter_map(|key| m.get(*key).and_then(Value::as_str))
                    .any(|name| same_model(name, model))
            })
        });
    let Some(entry) = entry else { return 0 };
    let size = entry.get("size").and_then(Value::as_u64).unwrap_or(0);
    let vram = entry.get("size_vram").and_then(Value::as_u64).unwrap_or(0);
    if size == 0 {
        return 0;
    }
    let percent = (vram as f64 / size as f64 * 100.0).round();
    percent.clamp(0.0, 100.0) as u32
}
