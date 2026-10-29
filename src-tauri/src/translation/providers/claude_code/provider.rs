//! `claude_code` provider: runs one prompt in a separate, isolated, short-lived `claude -p`
//! process (plan T17b, D41) built exactly from `docs/spikes/claude-invocation.json`.
//!
//! Every run:
//! - writes the system prompt to a UTF-8 file under the app's `tmp` dir (`--system-prompt-file`)
//!   and passes the output schema inline (`--json-schema`, minified JSON);
//! - is spawned from the fixed, empty `claude-cwd` with `CREATE_NO_WINDOW`, the contract's env
//!   (auto-updater and nonessential traffic off, `TEMP`/`TMP` in the app's `tmp` dir) and without
//!   the [`ENV_REMOVE`] session markers;
//! - starts suspended, goes into a Job Object with kill-on-close, then resumes, so cancel or
//!   timeout kills the whole process tree (fallback: `taskkill /T /F /PID`);
//! - gets the user message on stdin, then stdin is closed;
//! - is registered in [`TranslatorPids`] while it runs, so session discovery skips it.
//!
//! The cleanup lives in [`Run`]'s `Drop`: the service drops the `complete` future on cancel and
//! on timeout, and dropping it must still kill the tree, unregister the PID and delete the
//! system-prompt file.

use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::{Child, Command};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

#[cfg(windows)]
use super::job::{self, Job};
use super::{ClaudeExe, ENV_REMOVE, ExeKind, Housekeeping, TRANSLATOR_NAME, TranslatorPids};
use crate::error::{AppError, AppResult};
use crate::settings::{ClaudeSettings, Settings};
use crate::system::paths;
use crate::translation::model::ProviderKind;
use crate::translation::prompt_builder::{BuiltPrompt, output_schema};
use crate::translation::service::{ProviderOutput, RawOutput, TranslationProvider};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;
#[cfg(windows)]
const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// Longest stderr excerpt carried by `ProcessCrashed`.
const STDERR_EXCERPT_CHARS: usize = 500;
/// Longest raw text carried by `MalformedOutput`.
const MALFORMED_EXCERPT_CHARS: usize = 2000;

/// The contract's envelope field names (`envelope` in `claude-invocation.json`).
const RESULT_FIELD: &str = "result";
const ERROR_FIELD: &str = "is_error";
const STRUCTURED_FIELD: &str = "structured_output";

/// Lower-case fragments of the `result` text that mean "not signed in" in an error envelope.
/// The contract's case is `Not logged in · Please run /login`.
const AUTH_MARKERS: &[&str] = &[
    "not logged in",
    "/login",
    "invalid api key",
    "authentication",
    "oauth token",
];

pub struct ClaudeCodeProvider {
    pids: TranslatorPids,
    housekeeping: Housekeeping,
}

impl ClaudeCodeProvider {
    /// `pids` and `housekeeping` are handles to the app's shared registry and run counter.
    pub fn new(pids: TranslatorPids, housekeeping: Housekeeping) -> Self {
        Self { pids, housekeeping }
    }
}

#[async_trait]
impl TranslationProvider for ClaudeCodeProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::ClaudeCode
    }

    fn timeout(&self, settings: &Settings) -> Duration {
        Duration::from_secs(u64::from(settings.claude.timeout_secs))
    }

    async fn complete(
        &self,
        prompt: &BuiltPrompt,
        settings: &Settings,
        cancel: CancellationToken,
    ) -> AppResult<ProviderOutput> {
        // Resolved per run: the auto-updater replaces claude.exe in place.
        let exe = super::find_claude(settings.claude.executable_path.as_deref())
            .ok_or(AppError::ClaudeNotFound)?;
        let model = model(&settings.claude);
        let (cwd, tmp) = (paths::claude_cwd()?, paths::tmp_dir()?);
        let invocation = Invocation {
            exe: &exe,
            cwd: &cwd,
            tmp: &tmp,
            model: &model,
        };
        let value = run(&invocation, prompt, &self.pids, &self.housekeeping, &cancel).await?;
        Ok(ProviderOutput {
            raw: RawOutput::Json(value),
            model,
        })
    }
}

/// `settings.claude.model`, or the default when it is blank.
fn model(settings: &ClaudeSettings) -> String {
    let model = settings.model.trim();
    if model.is_empty() {
        ClaudeSettings::default().model
    } else {
        model.to_owned()
    }
}

/// Everything one run needs besides the prompt: explicit inputs, so a run doesn't depend on the
/// app's managed state or its path roots.
pub struct Invocation<'a> {
    pub exe: &'a ClaudeExe,
    /// The fixed, empty working dir (`paths::claude_cwd()`).
    pub cwd: &'a Path,
    /// The app's temp dir (`paths::tmp_dir()`): system-prompt files and Claude's `TEMP`/`TMP`.
    pub tmp: &'a Path,
    pub model: &'a str,
}

/// The contract's `args`, in its order, with the placeholders filled in.
pub fn args(model: &str, system_prompt_file: &Path, json_schema: &str) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "-p",
        "--safe-mode",
        "--setting-sources",
        "",
        "--no-session-persistence",
        "--tools",
        "",
        "--strict-mcp-config",
        "--disable-slash-commands",
        "--name",
        TRANSLATOR_NAME,
        "--model",
        model,
        "--output-format",
        "json",
        "--system-prompt-file",
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push(system_prompt_file.as_os_str().to_owned());
    args.push("--json-schema".into());
    args.push(json_schema.into());
    args
}

/// The contract's added `env`.
pub fn env(tmp: &Path) -> Vec<(&'static str, OsString)> {
    vec![
        ("DISABLE_AUTOUPDATER", "1".into()),
        ("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1".into()),
        ("TEMP", tmp.as_os_str().to_owned()),
        ("TMP", tmp.as_os_str().to_owned()),
    ]
}

/// Runs the prompt in one `claude -p` process and returns its `structured_output`.
///
/// Errors: `InvalidInput` (empty user message), `ClaudeNotFound`, `ClaudeAuth`,
/// `ProcessCrashed` (non-zero exit, with a stderr excerpt), `MalformedOutput` (no or unreadable
/// structured output), `Cancelled`, `Io`.
pub async fn run(
    invocation: &Invocation<'_>,
    prompt: &BuiltPrompt,
    pids: &TranslatorPids,
    housekeeping: &Housekeeping,
    cancel: &CancellationToken,
) -> AppResult<Value> {
    // Empty stdin makes `claude -p` exit 1 with a usage error.
    if prompt.user.trim().is_empty() {
        return Err(AppError::InvalidInput("Enter some text first".into()));
    }
    let schema = serde_json::to_string(&output_schema(prompt.mode))
        .map_err(|e| AppError::Io(format!("Can't serialize the output schema: {e}")))?;

    let prompt_file = invocation
        .tmp
        .join(format!("system-prompt-{}.txt", Uuid::new_v4()));
    fs::write(&prompt_file, prompt.system.as_bytes())?;
    let mut run = Run::new(prompt_file, pids.clone(), housekeeping.clone());

    let mut cmd = Command::new(&invocation.exe.path);
    cmd.args(args(invocation.model, &run.prompt_file, &schema))
        .current_dir(invocation.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for var in ENV_REMOVE {
        cmd.env_remove(var);
    }
    for (key, value) in env(invocation.tmp) {
        cmd.env(key, value);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW | CREATE_SUSPENDED);

    let child = cmd.spawn().map_err(|e| spawn_error(&e, invocation.exe))?;
    run.started(child)?;

    let child = run.child.as_mut().expect("the child was just started");
    let mut stdin = child.stdin.take();
    let mut stdout = child.stdout.take();
    let mut stderr = child.stderr.take();
    let user = prompt.user.as_bytes();
    let io = async {
        let write = async {
            if let Some(mut pipe) = stdin.take() {
                // A failure here means claude exited early; its exit code and stderr say why.
                if let Err(e) = pipe.write_all(user).await {
                    log::info!("Writing the prompt to claude failed: {e}");
                }
                // Dropping the pipe closes stdin.
            }
        };
        let (_, out, err, status) = tokio::join!(
            write,
            read_all(stdout.as_mut()),
            read_all(stderr.as_mut()),
            child.wait()
        );
        (out, err, status)
    };
    let (out, err, status) = tokio::select! {
        biased;
        () = cancel.cancelled() => return Err(AppError::Cancelled),
        result = io => result,
    };
    let status: ExitStatus = status?;
    run.exited = true;
    interpret(status.code(), &out, &err)
}

async fn read_all<R: tokio::io::AsyncRead + Unpin>(pipe: Option<&mut R>) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Some(pipe) = pipe
        && let Err(e) = pipe.read_to_end(&mut buf).await
    {
        log::info!("Reading claude's output failed: {e}");
    }
    buf
}

fn spawn_error(e: &io::Error, exe: &ClaudeExe) -> AppError {
    match e.kind() {
        io::ErrorKind::NotFound => AppError::ClaudeNotFound,
        // Rust refuses batch-file arguments it can't escape for cmd.exe.
        io::ErrorKind::InvalidInput if exe.kind == ExeKind::CmdShim => {
            AppError::ProcessCrashed(format!(
                "claude.cmd can't take the translator's arguments ({e}). Install the native \
                 claude.exe or set its path in Settings."
            ))
        }
        _ => AppError::ProcessCrashed(format!("Couldn't start claude: {e}")),
    }
}

/// One live run: kills the tree, unregisters the PID, deletes the system-prompt file and counts
/// the run for housekeeping when dropped, however the run ended.
struct Run {
    prompt_file: PathBuf,
    pids: TranslatorPids,
    housekeeping: Housekeeping,
    pid: Option<u32>,
    #[cfg(windows)]
    job: Option<Job>,
    /// Dropped after `Drop::drop` ran; `kill_on_drop` kills the main process if still alive.
    child: Option<Child>,
    /// The main process exited by itself.
    exited: bool,
}

impl Run {
    fn new(prompt_file: PathBuf, pids: TranslatorPids, housekeeping: Housekeeping) -> Self {
        Self {
            prompt_file,
            pids,
            housekeeping,
            pid: None,
            #[cfg(windows)]
            job: None,
            child: None,
            exited: false,
        }
    }

    /// Registers a freshly spawned (suspended) child, puts it into a job and resumes it.
    fn started(&mut self, child: Child) -> AppResult<()> {
        let pid = child.id();
        if let Some(pid) = pid {
            self.pids.add(pid);
        }
        self.pid = pid;
        #[cfg(windows)]
        {
            self.job = child.raw_handle().and_then(|handle| {
                match Job::new().and_then(|job| job.assign(handle).map(|()| job)) {
                    Ok(job) => Some(job),
                    Err(e) => {
                        log::warn!("No job object for the translator run, using taskkill: {e}");
                        None
                    }
                }
            });
            self.child = Some(child);
            let resumed = match pid {
                Some(pid) => job::resume_process(pid),
                None => Err(io::Error::other("the process has no id")),
            };
            resumed.map_err(|e| AppError::ProcessCrashed(format!("Couldn't start claude: {e}")))
        }
        #[cfg(not(windows))]
        {
            self.child = Some(child);
            Ok(())
        }
    }

    /// Kills the main process and everything it started.
    fn kill_tree(&mut self) {
        #[cfg(windows)]
        {
            if let Some(job) = &self.job {
                job.terminate();
                return;
            }
            // Before the main process dies: taskkill finds the tree through it.
            if let Some(pid) = self.pid {
                taskkill(pid);
            }
        }
        if let Some(child) = self.child.as_mut()
            && let Err(e) = child.start_kill()
        {
            log::info!("Killing claude failed: {e}");
        }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        if let Some(pid) = self.pid {
            if self.exited {
                // The main process is gone; anything it left behind goes with the job.
                #[cfg(windows)]
                if let Some(job) = &self.job {
                    job.terminate();
                }
            } else {
                self.kill_tree();
            }
            self.pids.remove(pid);
        }
        if let Err(e) = fs::remove_file(&self.prompt_file)
            && e.kind() != io::ErrorKind::NotFound
        {
            log::warn!("Can't delete the system-prompt file: {e}");
        }
        if self.child.is_some() {
            self.housekeeping.run_finished();
        }
    }
}

/// Fallback when the run has no job: `taskkill /T /F /PID`, waited for (it is short).
#[cfg(windows)]
fn taskkill(pid: u32) {
    use std::os::windows::process::CommandExt;
    let status = std::process::Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status();
    match status {
        Ok(status) if status.success() => {}
        Ok(status) => log::info!("taskkill for the translator run exited with {status}"),
        Err(e) => log::warn!("taskkill for the translator run failed: {e}"),
    }
}

/// Maps a finished run to its result (contract `outcomes`).
fn interpret(exit_code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> AppResult<Value> {
    let stdout = String::from_utf8_lossy(stdout);
    let stdout = stdout.trim();
    let envelope = serde_json::from_str::<Value>(stdout)
        .ok()
        .filter(Value::is_object);
    let result_text = envelope
        .as_ref()
        .and_then(|e| e.get(RESULT_FIELD))
        .and_then(Value::as_str)
        .unwrap_or("");

    let is_error = envelope
        .as_ref()
        .and_then(|e| e.get(ERROR_FIELD))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    // Only an error can be "not signed in": a free-text answer may contain the same words.
    if (is_error || exit_code != Some(0)) && is_auth_error(result_text) {
        return Err(AppError::ClaudeAuth);
    }
    if is_error {
        let subtype = envelope
            .as_ref()
            .and_then(|e| e.get("subtype"))
            .and_then(Value::as_str)
            .unwrap_or("");
        if subtype.contains(STRUCTURED_FIELD) {
            return Err(AppError::MalformedOutput(excerpt(
                result_text,
                MALFORMED_EXCERPT_CHARS,
            )));
        }
        let message = if result_text.is_empty() {
            subtype
        } else {
            result_text
        };
        return Err(AppError::ProcessCrashed(excerpt(
            message,
            STDERR_EXCERPT_CHARS,
        )));
    }
    match exit_code {
        Some(0) => {}
        code => return Err(crashed(code, stderr, stdout)),
    }

    let Some(envelope) = envelope.as_ref() else {
        return Err(AppError::MalformedOutput(excerpt(
            stdout,
            MALFORMED_EXCERPT_CHARS,
        )));
    };
    match envelope.get(STRUCTURED_FIELD) {
        Some(value) if !value.is_null() => Ok(value.clone()),
        // The model answered in free text: no fallback to `result`.
        _ => Err(AppError::MalformedOutput(excerpt(
            result_text,
            MALFORMED_EXCERPT_CHARS,
        ))),
    }
}

fn is_auth_error(result: &str) -> bool {
    let lowered = result.to_lowercase();
    AUTH_MARKERS.iter().any(|marker| lowered.contains(marker))
}

/// `ProcessCrashed` for a non-zero exit: the exit code and an excerpt of stderr (or of stdout
/// when stderr is empty).
fn crashed(code: Option<i32>, stderr: &[u8], stdout: &str) -> AppError {
    let stderr = String::from_utf8_lossy(stderr);
    let text = match stderr.trim() {
        "" => stdout,
        text => text,
    };
    let status = match code {
        Some(code) => format!("claude exited with code {code}"),
        None => "claude was terminated".to_owned(),
    };
    if text.is_empty() {
        AppError::ProcessCrashed(status)
    } else {
        AppError::ProcessCrashed(format!("{status}: {}", excerpt(text, STDERR_EXCERPT_CHARS)))
    }
}

fn excerpt(text: &str, max_chars: usize) -> String {
    text.trim().chars().take(max_chars).collect()
}
