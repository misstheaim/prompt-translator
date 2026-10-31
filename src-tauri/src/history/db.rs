//! Opens `history.db`, applies the migrations (plan §4.7, T27) and holds the connection as
//! managed state.

use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use rusqlite::Connection;
use rusqlite::functions::FunctionFlags;
use rusqlite_migration::{M, Migrations};

use crate::error::{AppError, AppResult};

/// Timestamp format of `translations.created_at`: ISO 8601, UTC, millisecond precision.
/// Used both when inserting and when computing the `since_days` cut-off, so they compare as text.
pub const TIMESTAMP_SQL: &str = "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')";

const SCHEMA_V1: &str = "
CREATE TABLE translations (
  id INTEGER PRIMARY KEY,
  created_at TEXT NOT NULL,
  mode TEXT NOT NULL, provider TEXT NOT NULL, model TEXT NOT NULL, preset_id TEXT NOT NULL,
  source_text TEXT NOT NULL, output_text TEXT NOT NULL,
  duration_ms INTEGER NOT NULL,
  token_warnings TEXT NOT NULL DEFAULT '[]',
  accepted_target TEXT,
  accepted_session_id TEXT,
  accepted_session_name TEXT
);
CREATE TABLE issues (
  id INTEGER PRIMARY KEY,
  translation_id INTEGER NOT NULL REFERENCES translations(id) ON DELETE CASCADE,
  category TEXT NOT NULL, original TEXT NOT NULL, corrected TEXT NOT NULL, explanation TEXT NOT NULL
);
CREATE INDEX idx_translations_created ON translations(created_at);
CREATE INDEX idx_issues_category ON issues(category);
CREATE INDEX idx_issues_translation ON issues(translation_id);
";

/// The language pair of each entry (T52); NULL in entries saved before it, which were all
/// Russian → English.
const SCHEMA_V2: &str = "
ALTER TABLE translations ADD COLUMN source_language TEXT;
ALTER TABLE translations ADD COLUMN target_language TEXT;
";

fn migrations() -> Migrations<'static> {
    Migrations::new(vec![M::up(SCHEMA_V1), M::up(SCHEMA_V2)])
}

/// Applies `PRAGMA`s, the Unicode-aware `ulower` SQL function and the pending migrations.
fn prepare(conn: &mut Connection) -> AppResult<()> {
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // `journal_mode` returns a row, so it can't go through `execute`.
    conn.pragma_update_and_check(None, "journal_mode", "WAL", |_| Ok(()))?;
    // SQLite's own `lower()` only folds ASCII; mistakes are often Cyrillic.
    conn.create_scalar_function(
        "ulower",
        1,
        FunctionFlags::SQLITE_UTF8 | FunctionFlags::SQLITE_DETERMINISTIC,
        |ctx| {
            let text: Option<String> = ctx.get(0)?;
            Ok(text.map(|t| t.to_lowercase()))
        },
    )?;
    migrations().to_latest(conn)?;
    Ok(())
}

/// Opens (creating if needed) the database file and brings it up to date.
pub fn open(path: &Path) -> AppResult<Connection> {
    let mut conn = Connection::open(path)?;
    prepare(&mut conn)?;
    Ok(conn)
}

/// The history database as managed state. Repo functions are sync; callers use `spawn_blocking`.
/// Cloning gives another handle to the same connection.
#[derive(Clone)]
pub struct HistoryDb {
    conn: Arc<Mutex<Connection>>,
}

impl HistoryDb {
    pub fn open(path: &Path) -> AppResult<Self> {
        Ok(Self {
            conn: Arc::new(Mutex::new(open(path)?)),
        })
    }

    pub fn conn(&self) -> AppResult<MutexGuard<'_, Connection>> {
        self.conn
            .lock()
            .map_err(|_| AppError::Db("history database lock poisoned".into()))
    }
}
