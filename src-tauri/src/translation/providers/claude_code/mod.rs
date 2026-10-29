//! `claude_code` provider (plan T17a, T17b, D41): finding Claude Code, its status, the registry
//! of the app's own translator runs, the cleanup of the translator's dirs, and the translation
//! run itself ([`ClaudeCodeProvider`]).

pub mod detect;
pub mod housekeeping;
#[cfg(windows)]
mod job;
pub mod provider;
pub mod registry;

pub use detect::{ClaudeExe, ExeKind, find_claude, status};
pub use housekeeping::Housekeeping;
pub use provider::ClaudeCodeProvider;
pub use registry::{TRANSLATOR_NAME, TranslatorPids, is_translator_run};

/// Environment variables removed from every `claude` process the app spawns, so a Claude Code
/// session the app was started from can't leak into it (`docs/spikes/claude-invocation.json`).
/// An explicit list, not a `CLAUDE_CODE_*` prefix sweep.
pub const ENV_REMOVE: &[&str] = &[
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN",
    "CLAUDE_CODE_EXECPATH",
    "CLAUDE_PID",
    "CLAUDE_EFFORT",
];
