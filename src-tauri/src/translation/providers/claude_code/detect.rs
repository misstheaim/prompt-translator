//! Finding `claude` and reading its status (plan T17a, D41).

use std::env;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;
use tokio::time::timeout;

use super::ENV_REMOVE;
use crate::settings::Settings;
use crate::translation::providers::ClaudeStatus;

const CHECK_TIMEOUT: Duration = Duration::from_secs(5);
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExeKind {
    /// A real `claude.exe`.
    Native,
    /// A `claude.cmd` shim: limited, arguments can't hold newlines or quotes.
    CmdShim,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaudeExe {
    pub path: PathBuf,
    pub kind: ExeKind,
}

fn kind_of(path: &Path) -> ExeKind {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
    if ext.eq_ignore_ascii_case("cmd") || ext.eq_ignore_ascii_case("bat") {
        ExeKind::CmdShim
    } else {
        ExeKind::Native
    }
}

/// Detection with explicit inputs: the settings path, the user profile dir and the `PATH` value.
/// Order: settings path, `<profile>\.local\bin\claude.exe`, `claude.exe` on `PATH`, only then
/// `claude.cmd` on `PATH`.
pub fn find_claude_in(
    configured: Option<&str>,
    user_profile: Option<&Path>,
    path_var: Option<&std::ffi::OsStr>,
) -> Option<ClaudeExe> {
    if let Some(p) = configured.map(str::trim).filter(|p| !p.is_empty()) {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(ClaudeExe {
                kind: kind_of(&p),
                path: p,
            });
        }
        log::warn!("The configured Claude executable doesn't exist; searching instead");
    }
    if let Some(profile) = user_profile {
        let p = profile.join(".local").join("bin").join("claude.exe");
        if p.is_file() {
            return Some(ClaudeExe {
                path: p,
                kind: ExeKind::Native,
            });
        }
    }
    let dirs: Vec<PathBuf> = path_var
        .map(|v| env::split_paths(v).collect())
        .unwrap_or_default();
    for (file, kind) in [
        ("claude.exe", ExeKind::Native),
        ("claude.cmd", ExeKind::CmdShim),
    ] {
        if let Some(p) = dirs.iter().map(|d| d.join(file)).find(|p| p.is_file()) {
            if kind == ExeKind::CmdShim {
                log::warn!("Using claude.cmd: limited, no newlines or quotes in arguments");
            }
            return Some(ClaudeExe { path: p, kind });
        }
    }
    None
}

/// Finds Claude Code for the given settings value (`settings.claude.executablePath`).
pub fn find_claude(configured: Option<&str>) -> Option<ClaudeExe> {
    let profile = env::var_os("USERPROFILE").map(PathBuf::from);
    find_claude_in(
        configured,
        profile.as_deref(),
        env::var_os("PATH").as_deref(),
    )
}

fn command(exe: &ClaudeExe, args: &[&str]) -> Command {
    let mut cmd = Command::new(&exe.path);
    cmd.args(args).stdin(Stdio::null()).kill_on_drop(true);
    for var in ENV_REMOVE {
        cmd.env_remove(var);
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

/// `claude --version`, trimmed; `None` on failure or after 5 s.
async fn version(exe: &ClaudeExe) -> Option<String> {
    let mut cmd = command(exe, &["--version"]);
    cmd.stdout(Stdio::piped()).stderr(Stdio::null());
    let out = timeout(CHECK_TIMEOUT, cmd.output()).await.ok()?.ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!text.is_empty()).then_some(text)
}

/// `claude auth status`: exit 0 signed in, exit 1 not, anything else or a timeout is unknown.
/// Its output holds the account's e-mail, so it goes to null and is never read or logged.
async fn signed_in(exe: &ClaudeExe) -> Option<bool> {
    let mut cmd = command(exe, &["auth", "status"]);
    cmd.stdout(Stdio::null()).stderr(Stdio::null());
    let status = timeout(CHECK_TIMEOUT, cmd.status()).await.ok()?.ok()?;
    match status.code() {
        Some(0) => Some(true),
        Some(1) => Some(false),
        _ => None,
    }
}

/// The value of `provider_status.claude` for the current settings.
pub async fn status(settings: &Settings) -> ClaudeStatus {
    let Some(exe) = find_claude(settings.claude.executable_path.as_deref()) else {
        return ClaudeStatus {
            found: false,
            path: None,
            version: None,
            signed_in: None,
        };
    };
    let (version, signed_in) = tokio::join!(version(&exe), signed_in(&exe));
    ClaudeStatus {
        found: true,
        path: Some(exe.path.to_string_lossy().into_owned()),
        version,
        signed_in,
    }
}
