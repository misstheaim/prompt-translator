//! History repository (plan T27): sync functions over a `rusqlite::Connection`; callers run
//! them inside `spawn_blocking`.
//!
//! Text comparisons use the `ulower` SQL function (see `db.rs`) so they fold case for any
//! script, not just ASCII.

use rusqlite::types::Type;
use rusqlite::{Connection, Row, ToSql, params};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::db::TIMESTAMP_SQL;
use super::{
    AcceptedTarget, CategoryCount, HistoryEntry, HistoryEntrySummary, IssueFilter, MistakesSummary,
    RecurringMistake, VocabularyItem,
};
use crate::error::{AppError, AppResult};
use crate::translation::model::{Issue, TokenWarning, TranslationRequest, TranslationResult};

const PREVIEW_CHARS: usize = 120;
const MAX_RECURRING: i64 = 50;
const MAX_VOCABULARY: i64 = 100;

/// Enums are stored as their serde string (`"translate"`, `"claudeCode"`, `"word_choice"`).
fn enum_to_str<T: Serialize>(value: &T) -> AppResult<String> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(s) => Ok(s),
        other => Err(AppError::Db(format!("expected a string enum, got {other}"))),
    }
}

fn str_to_enum<T: DeserializeOwned>(s: &str) -> Result<T, serde_json::Error> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
}

fn col_enum<T: DeserializeOwned>(row: &Row<'_>, idx: usize) -> rusqlite::Result<T> {
    let s: String = row.get(idx)?;
    str_to_enum(&s)
        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(idx, Type::Text, Box::new(e)))
}

fn accepted_from_row(
    target: Option<String>,
    session_id: Option<String>,
    session_name: Option<String>,
) -> Option<AcceptedTarget> {
    match target.as_deref() {
        Some("clipboard") => Some(AcceptedTarget::Clipboard),
        Some("session") => Some(AcceptedTarget::Session {
            id: session_id.unwrap_or_default(),
            name: session_name.unwrap_or_default(),
        }),
        _ => None,
    }
}

/// Stores a translation and its issues in one transaction and returns the new id.
pub fn insert(
    conn: &Connection,
    result: &TranslationResult,
    request: &TranslationRequest,
) -> AppResult<i64> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "INSERT INTO translations (created_at, mode, provider, model, preset_id, source_text, \
             output_text, duration_ms, token_warnings) \
             VALUES ({TIMESTAMP_SQL}, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)"
        ),
        params![
            enum_to_str(&request.mode)?,
            enum_to_str(&result.provider)?,
            result.model,
            request.preset_id,
            request.source_text,
            result.output,
            i64::try_from(result.duration_ms).unwrap_or(i64::MAX),
            serde_json::to_string(&result.token_warnings)?,
        ],
    )?;
    let id = tx.last_insert_rowid();
    {
        let mut stmt = tx.prepare(
            "INSERT INTO issues (translation_id, category, original, corrected, explanation) \
             VALUES (?1, ?2, ?3, ?4, ?5)",
        )?;
        for issue in &result.issues {
            stmt.execute(params![
                id,
                enum_to_str(&issue.category)?,
                issue.original,
                issue.corrected,
                issue.explanation
            ])?;
        }
    }
    tx.commit()?;
    Ok(id)
}

/// Records where an entry went. A missing id is not an error (the entry may have been deleted).
pub fn mark_accepted(conn: &Connection, id: i64, target: &AcceptedTarget) -> AppResult<()> {
    let (kind, session_id, session_name) = match target {
        AcceptedTarget::Clipboard => ("clipboard", None, None),
        AcceptedTarget::Session { id, name } => ("session", Some(id.as_str()), Some(name.as_str())),
    };
    conn.execute(
        "UPDATE translations SET accepted_target = ?1, accepted_session_id = ?2, \
         accepted_session_name = ?3 WHERE id = ?4",
        params![kind, session_id, session_name, id],
    )?;
    Ok(())
}

/// Escapes `%`, `_` and `\` for a `LIKE ... ESCAPE '\'` pattern.
fn like_pattern(query: &str) -> String {
    let mut out = String::with_capacity(query.len() + 2);
    out.push('%');
    for c in query.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

fn preview(text: &str) -> String {
    text.chars().take(PREVIEW_CHARS).collect()
}

/// Newest first. `query` matches source or output text; `issue` keeps entries that contain that
/// mistake. Both are case-insensitive.
pub fn list(
    conn: &Connection,
    query: Option<&str>,
    issue: Option<&IssueFilter>,
    limit: u32,
    offset: u32,
) -> AppResult<Vec<HistoryEntrySummary>> {
    let mut sql = String::from(
        "SELECT t.id, t.created_at, t.mode, t.provider, t.model, t.source_text, t.output_text, \
         (SELECT COUNT(*) FROM issues i WHERE i.translation_id = t.id), \
         t.accepted_target, t.accepted_session_id, t.accepted_session_name \
         FROM translations t WHERE 1 = 1",
    );
    let mut args: Vec<Box<dyn ToSql>> = Vec::new();

    if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
        args.push(Box::new(like_pattern(&q.to_lowercase())));
        let n = args.len();
        sql.push_str(&format!(
            " AND (ulower(t.source_text) LIKE ?{n} ESCAPE '\\' \
             OR ulower(t.output_text) LIKE ?{n} ESCAPE '\\')"
        ));
    }
    if let Some(f) = issue {
        args.push(Box::new(f.original.to_lowercase()));
        args.push(Box::new(f.corrected.to_lowercase()));
        let n = args.len();
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM issues i WHERE i.translation_id = t.id \
             AND ulower(i.original) = ?{} AND ulower(i.corrected) = ?{n})",
            n - 1
        ));
    }
    args.push(Box::new(i64::from(limit)));
    args.push(Box::new(i64::from(offset)));
    let n = args.len();
    sql.push_str(&format!(
        " ORDER BY t.created_at DESC, t.id DESC LIMIT ?{} OFFSET ?{n}",
        n - 1
    ));

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        rusqlite::params_from_iter(args.iter().map(|a| a.as_ref())),
        |row| {
            let source: String = row.get(5)?;
            let output: String = row.get(6)?;
            Ok(HistoryEntrySummary {
                id: row.get(0)?,
                created_at: row.get(1)?,
                mode: col_enum(row, 2)?,
                provider: col_enum(row, 3)?,
                model: row.get(4)?,
                source_preview: preview(&source),
                output_preview: preview(&output),
                issue_count: row.get(7)?,
                accepted: accepted_from_row(row.get(8)?, row.get(9)?, row.get(10)?),
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// One entry with its issues, or `None` when the id doesn't exist.
pub fn get(conn: &Connection, id: i64) -> AppResult<Option<HistoryEntry>> {
    let entry = conn.query_row(
        "SELECT id, created_at, mode, provider, model, preset_id, source_text, output_text, \
         duration_ms, token_warnings, accepted_target, accepted_session_id, accepted_session_name \
         FROM translations WHERE id = ?1",
        [id],
        |row| {
            let warnings: String = row.get(9)?;
            let token_warnings: Vec<TokenWarning> =
                serde_json::from_str(&warnings).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(9, Type::Text, Box::new(e))
                })?;
            let duration: i64 = row.get(8)?;
            Ok(HistoryEntry {
                id: row.get(0)?,
                created_at: row.get(1)?,
                mode: col_enum(row, 2)?,
                provider: col_enum(row, 3)?,
                model: row.get(4)?,
                preset_id: row.get(5)?,
                source_text: row.get(6)?,
                output_text: row.get(7)?,
                duration_ms: u64::try_from(duration).unwrap_or(0),
                token_warnings,
                accepted: accepted_from_row(row.get(10)?, row.get(11)?, row.get(12)?),
                issues: Vec::new(),
            })
        },
    );
    let mut entry = match entry {
        Ok(entry) => entry,
        Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
        Err(e) => return Err(e.into()),
    };
    let mut stmt = conn.prepare(
        "SELECT category, original, corrected, explanation FROM issues \
         WHERE translation_id = ?1 ORDER BY id",
    )?;
    let issues = stmt.query_map([id], |row| {
        Ok(Issue {
            category: col_enum(row, 0)?,
            original: row.get(1)?,
            corrected: row.get(2)?,
            explanation: row.get(3)?,
        })
    })?;
    entry.issues = issues.collect::<Result<_, _>>()?;
    Ok(Some(entry))
}

/// Deletes one entry; its issues go with it (`ON DELETE CASCADE`).
pub fn delete(conn: &Connection, id: i64) -> AppResult<()> {
    conn.execute("DELETE FROM translations WHERE id = ?1", [id])?;
    Ok(())
}

/// Deletes everything.
pub fn clear(conn: &Connection) -> AppResult<()> {
    conn.execute("DELETE FROM translations", [])?;
    Ok(())
}

/// The "my mistakes" aggregates, over entries of the last `since_days` days (all when `None`).
pub fn mistakes_summary(conn: &Connection, since_days: Option<u32>) -> AppResult<MistakesSummary> {
    // `-N days` modifier for strftime; NULL cut-off means "no limit".
    let since: Option<String> = since_days.map(|d| format!("-{d} days"));
    let filter = "(?1 IS NULL OR t.created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1))";

    let mut stmt = conn.prepare(&format!(
        "SELECT i.category, COUNT(*) FROM issues i JOIN translations t ON t.id = i.translation_id \
         WHERE {filter} GROUP BY i.category ORDER BY COUNT(*) DESC, i.category"
    ))?;
    let by_category = stmt
        .query_map([&since], |row| {
            Ok(CategoryCount {
                category: col_enum(row, 0)?,
                count: row.get(1)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    // Bare columns next to MAX() take their values from the row holding the maximum, so the
    // displayed spelling and category are those of the latest occurrence.
    let mut stmt = conn.prepare(&format!(
        "SELECT i.original, i.corrected, i.category, COUNT(*) AS n, MAX(t.created_at) \
         FROM issues i JOIN translations t ON t.id = i.translation_id \
         WHERE {filter} GROUP BY ulower(i.original), ulower(i.corrected) HAVING COUNT(*) >= 2 \
         ORDER BY n DESC, MAX(t.created_at) DESC LIMIT {MAX_RECURRING}"
    ))?;
    let recurring = stmt
        .query_map([&since], |row| {
            Ok(RecurringMistake {
                original: row.get(0)?,
                corrected: row.get(1)?,
                category: col_enum(row, 2)?,
                count: row.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    let mut stmt = conn.prepare(&format!(
        "SELECT i.original, i.corrected, COUNT(*) AS n, MAX(t.created_at) AS last_seen \
         FROM issues i JOIN translations t ON t.id = i.translation_id \
         WHERE i.category = 'vocabulary' AND {filter} \
         GROUP BY ulower(i.original), ulower(i.corrected) \
         ORDER BY n DESC, last_seen DESC LIMIT {MAX_VOCABULARY}"
    ))?;
    let vocabulary = stmt
        .query_map([&since], |row| {
            Ok(VocabularyItem {
                original: row.get(0)?,
                corrected: row.get(1)?,
                count: row.get(2)?,
                last_seen: row.get(3)?,
            })
        })?
        .collect::<Result<_, _>>()?;

    Ok(MistakesSummary {
        by_category,
        recurring,
        vocabulary,
    })
}
