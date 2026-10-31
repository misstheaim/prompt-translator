//! Where accepted text is delivered (clipboard or Claude session), and the Claude sessions a
//! Translate tab can be bound to (T38, plan D47).
//!
//! In v0.1.0 a session target is a terminal window (Windows Terminal, with one of its tabs, or a
//! classic console): one Claude session per window. VS Code extension sessions aren't targets;
//! they use the clipboard (owner decision on #148).

pub mod clipboard;
pub mod commands;
pub mod deliver;
pub mod sessions;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The terminal app that hosts a target window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum TerminalHost {
    /// Windows Terminal (`CASCADIA_HOSTING_WINDOW_CLASS`); all its windows usually share one
    /// process, so the window handle tells them apart.
    WindowsTerminal,
    /// The classic console window (`ConsoleWindowClass`).
    Conhost,
}

/// A terminal window (and, for Windows Terminal, one of its tabs) running a Claude session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TerminalTarget {
    /// The top-level window handle.
    #[ts(type = "number")]
    pub hwnd: u64,
    /// The window's process (Windows Terminal itself, or a console's client such as
    /// `powershell.exe`). A handle reused by another process doesn't match any more.
    pub pid: u32,
    /// The Windows Terminal tab's UI Automation RuntimeId (`42.165152658.4.9`); absent for a
    /// console window, or when the tabs couldn't be read.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tab_runtime_id: Option<String>,
    /// The tab's name or the window's title when it was read; a label only, it changes while
    /// Claude works.
    pub title: String,
    pub host: TerminalHost,
}

/// Something a Translate tab can be bound to (`kind` tells which; only terminals for now).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum SessionTarget {
    Terminal(TerminalTarget),
}

/// What `deliver_to_target` did with the text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum DeliveryOutcome {
    /// The text waits in the session's input (not sent: the user presses Enter in Claude).
    Delivered,
    /// The session couldn't take it; the text was copied to the clipboard instead. `reason`
    /// is a message for the user.
    DeliveredToClipboard { reason: String },
}

/// Payload of `target://closed`: a bound terminal window (or its bound tab) is gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TargetClosed {
    #[ts(type = "number")]
    pub hwnd: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub tab_runtime_id: Option<String>,
}
