//! Cleanup of the translator's own dirs (plan T17a; recipe in `docs/spikes/claude-isolation.md`).
//!
//! With the normal login (D41) nothing accumulates in `~/.claude`, which the app never touches.
//! Only two dirs under the app's local data dir are cleaned: `claude-cwd` must stay empty, and
//! `tmp` (system-prompt files, Claude's redirected temp files) loses its old files.

use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, SystemTime};

use crate::system::paths;

/// Housekeeping also runs after this many translator runs.
pub const RUNS_PER_CLEANUP: u32 = 50;
/// Files in `tmp` newer than this survive a cleanup while runs may be going on.
const TMP_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Run counter in managed state; T17b calls [`Housekeeping::run_finished`] after each run.
#[derive(Clone, Default)]
pub struct Housekeeping(Arc<AtomicU32>);

impl Housekeeping {
    pub fn new() -> Self {
        Self::default()
    }

    /// Counts one finished run and cleans up on every 50th.
    pub fn run_finished(&self) {
        let n = self.0.fetch_add(1, Ordering::Relaxed) + 1;
        if n.is_multiple_of(RUNS_PER_CLEANUP) {
            run(false);
        }
    }
}

/// Creates the fixed dirs and cleans them. At startup no translation is running, so `tmp` is
/// wiped completely; later only files older than a day go. Failures are logged, never raised.
pub fn run(startup: bool) {
    let (cwd, tmp) = match (paths::claude_cwd(), paths::tmp_dir()) {
        (Ok(cwd), Ok(tmp)) => (cwd, tmp),
        (Err(e), _) | (_, Err(e)) => {
            log::warn!("Claude housekeeping skipped: {e}");
            return;
        }
    };
    // A run may be using the cwd right now, but it never writes there; a leftover is junk.
    let max_age = if startup { None } else { Some(TMP_MAX_AGE) };
    for (dir, age) in [(&cwd, None), (&tmp, max_age)] {
        if let Err(e) = clean_dir(dir, age) {
            log::warn!("Cleaning {} failed: {e}", dir.display());
        }
    }
}

/// Deletes the entries directly inside `dir` (never `dir` itself, never anything outside it).
/// With `older_than`, entries modified more recently are kept. Symlinks are removed, not followed.
pub fn clean_dir(dir: &Path, older_than: Option<Duration>) -> io::Result<()> {
    let now = SystemTime::now();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let meta = fs::symlink_metadata(entry.path())?;
        if let Some(age) = older_than {
            let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            if now.duration_since(modified).unwrap_or_default() < age {
                continue;
            }
        }
        let path = entry.path();
        let res = if meta.is_dir() && !meta.file_type().is_symlink() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        if let Err(e) = res {
            // Something is in use (a running process): leave it for the next pass.
            log::warn!("Can't remove {}: {e}", path.display());
        }
    }
    Ok(())
}
