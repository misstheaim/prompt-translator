//! The translator-run registry (plan D41): PIDs of the `claude -p` processes the app itself runs.
//! `claude agents --json` lists them as interactive sessions while they run, so session
//! discovery and injection (T38, T39) must exclude them.

use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// The session name every translator run gets (`--name prompt-translator`). Matched exactly,
/// never by prefix: a user's session may be called e.g. `prompt-translator-d2`.
pub const TRANSLATOR_NAME: &str = "prompt-translator";

/// Shared set of live translator PIDs. Clones are handles to the same set.
#[derive(Clone, Default)]
pub struct TranslatorPids(Arc<Mutex<HashSet<u32>>>);

impl TranslatorPids {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashSet<u32>> {
        // A poisoned set is still a valid set of numbers.
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// T17b calls this right after spawning a translator run.
    pub fn add(&self, pid: u32) {
        self.lock().insert(pid);
    }

    /// T17b calls this once the process is gone.
    pub fn remove(&self, pid: u32) {
        self.lock().remove(&pid);
    }

    pub fn contains(&self, pid: u32) -> bool {
        self.lock().contains(&pid)
    }

    /// A copy of the live translator PIDs (session targets skip windows above them, T38).
    pub fn pids(&self) -> Vec<u32> {
        self.lock().iter().copied().collect()
    }
}

/// Whether a session entry is one of the app's own translator runs: its name is exactly
/// `prompt-translator` **or** its PID is registered. Two independent checks on purpose.
pub fn is_translator_run(pids: &TranslatorPids, pid: u32, name: Option<&str>) -> bool {
    name == Some(TRANSLATOR_NAME) || pids.contains(pid)
}
