//! Spike T58 (#148): deliver accepted text into an open Claude Code session without writing into
//! another process's console.
//!
//! - `list-terminals`: top-level terminal windows (HWND, pid, title, state) and Windows
//!   Terminal tabs (UIA `TabItem` name, RuntimeId, selected). Read-only.
//! - `paste`: bring a bound terminal window to the front, select the bound tab, paste the text
//!   once with Ctrl+V through the clipboard and restore the clipboard. **Never sends Enter.**
//! - `vscode`: open `vscode://anthropic.claude-code/open?session=<id>&prompt=<text>`.
//! - `sessions`: recent session ids from `~/.claude/projects` with their entrypoint. Read-only.
//!
//! Throwaway code, Windows only: not part of the app. Report and owner runbook:
//! docs/spikes/session-paste.md.

mod clipboard;
mod own_window;
mod paste;
mod sessions;
mod terminals;
mod text;
mod uia;
mod vscode;
mod win;

use std::collections::HashMap;
use std::process::ExitCode;

/// Every outcome has its own exit code, so a failure is detectable from a script.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Exit {
    /// `paste`: Ctrl+V sent and the clipboard restored (or left alone because it changed).
    /// `vscode`: the link was handed to the shell. `list-terminals`, `sessions`: listed.
    Ok = 0,
    /// Unexpected COM / UIA failure.
    Internal = 1,
    /// Bad arguments.
    Usage = 2,
    /// The text file is missing, not UTF-8 or empty.
    TextFile = 3,
    /// `--own-window` was closed instead of pressing Enter.
    Cancelled = 4,
    /// `IsWindow` is false: the bound terminal window is gone (closed).
    WindowGone = 10,
    /// The HWND now belongs to another process than `--pid`.
    PidMismatch = 11,
    /// The HWND is not a terminal window (class and exe checks, POPUP-007).
    NotTerminal = 12,
    /// `--tab` RuntimeId not found under the window (tab closed, or the id changed).
    TabNotFound = 13,
    /// `SelectionItemPattern.Select()` failed or the tab didn't become selected.
    TabSelectFailed = 14,
    /// The window stayed minimised after `ShowWindow(SW_RESTORE)`.
    StillMinimised = 15,
    /// The window did not become the foreground window (foreground lock).
    ForegroundRefused = 20,
    /// A modifier or Enter is still held (the user is typing): nothing pasted.
    KeysHeld = 21,
    /// The window or tab lost the foreground / selection just before the paste: nothing pasted.
    TargetLostFocus = 22,
    /// The clipboard couldn't be opened or written: nothing pasted.
    ClipboardBusy = 30,
    /// `SendInput` was blocked (e.g. an elevated terminal, UIPI): nothing pasted.
    InputBlocked = 31,
    /// Pasted, but the saved clipboard text couldn't be put back.
    RestoreFailed = 32,
    /// `ShellExecuteW` refused the `vscode://` link.
    LinkFailed = 40,
}

impl Exit {
    pub fn code(self) -> u8 {
        self as u8
    }
}

/// Parsed `--name value` options and `--switch` flags.
pub struct Flags {
    values: HashMap<String, String>,
    switches: Vec<String>,
}

impl Flags {
    pub fn parse(args: &[String], with_value: &[&str], switches: &[&str]) -> Result<Self, String> {
        let mut values = HashMap::new();
        let mut on = Vec::new();
        let mut iter = args.iter();
        while let Some(arg) = iter.next() {
            let Some(name) = arg.strip_prefix("--") else {
                return Err(format!("unexpected argument `{arg}`"));
            };
            if with_value.contains(&name) {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("`--{name}` needs a value"))?;
                values.insert(name.to_string(), value.clone());
            } else if switches.contains(&name) {
                on.push(name.to_string());
            } else {
                return Err(format!("unknown option `--{name}`"));
            }
        }
        Ok(Self {
            values,
            switches: on,
        })
    }

    pub fn value(&self, name: &str) -> Option<&str> {
        self.values.get(name).map(String::as_str)
    }

    pub fn has(&self, name: &str) -> bool {
        self.switches.iter().any(|s| s == name)
    }

    pub fn number<T: std::str::FromStr>(&self, name: &str, default: T) -> Result<T, String> {
        match self.value(name) {
            None => Ok(default),
            Some(v) => v
                .parse()
                .map_err(|_| format!("`--{name}` must be a number, got `{v}`")),
        }
    }
}

const USAGE: &str = "\
paste-spike: spike T58 (#148), see docs/spikes/session-paste.md

  paste-spike list-terminals [--all] [--title-chars <n>]
  paste-spike paste --hwnd <h> --text <file> [--pid <p>] [--tab <runtime-id>]
                    [--own-window | --delay <ms>] [--focus plain|alt|attach]
                    [--restore-delay <ms>] [--dry-run]
  paste-spike vscode --text <file> [--session <id>] [--print-only]
  paste-spike sessions [--limit <n>] [--entrypoint <name>]

paste never sends Enter: the text waits in Claude's input box.";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let rest = args.get(1..).unwrap_or(&[]);
    let result = match args.first().map(String::as_str) {
        Some("list-terminals") => terminals::run(rest),
        Some("paste") => paste::run(rest),
        Some("vscode") => vscode::run(rest),
        Some("sessions") => sessions::run(rest),
        _ => Err(Exit::Usage),
    };
    let exit = match result {
        Ok(()) => Exit::Ok,
        Err(exit) => {
            if exit == Exit::Usage {
                eprintln!("{USAGE}");
            }
            exit
        }
    };
    if exit != Exit::Ok {
        eprintln!("exit {} ({exit:?})", exit.code());
    }
    ExitCode::from(exit.code())
}

/// Prints a usage error and returns [`Exit::Usage`].
pub fn usage(message: impl std::fmt::Display) -> Exit {
    eprintln!("error: {message}");
    Exit::Usage
}
