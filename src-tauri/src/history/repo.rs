//! History repository (plan T27): sync functions over a `rusqlite::Connection`; callers run
//! them inside `spawn_blocking`.
//!
//! Text comparisons use the `ulower` SQL function (see `db.rs`) so they fold case for any
//! script, not just ASCII.

use std::collections::{HashMap, HashSet};

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
use crate::translation::language::{self, LanguagePair};
use crate::translation::model::{
    Issue, IssueCategory, Mode, TokenWarning, TranslationRequest, TranslationResult,
};

const PREVIEW_CHARS: usize = 120;
const MAX_RECURRING: usize = 50;
const MAX_VOCABULARY: usize = 100;

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

/// The pair of an entry; NULL columns (entries saved before T52) were Russian → English.
fn languages_from_row(source: Option<String>, target: Option<String>) -> (String, String) {
    (
        source.unwrap_or_else(|| language::DEFAULT_NATIVE.to_owned()),
        target.unwrap_or_else(|| language::DEFAULT_TARGET.to_owned()),
    )
}

/// Stores a translation and its issues in one transaction and returns the new id. `pair` is
/// the language pair the request used.
pub fn insert(
    conn: &Connection,
    result: &TranslationResult,
    request: &TranslationRequest,
    pair: LanguagePair,
) -> AppResult<i64> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        &format!(
            "INSERT INTO translations (created_at, mode, provider, model, preset_id, source_text, \
             output_text, duration_ms, token_warnings, source_language, target_language) \
             VALUES ({TIMESTAMP_SQL}, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"
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
            pair.native.code,
            pair.target.code,
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

/// Filters of [`list`]; all the given ones must match.
#[derive(Default)]
pub struct ListFilters<'a> {
    /// Matches source or output text, case-insensitive.
    pub query: Option<&'a str>,
    /// Keeps entries that contain this mistake.
    pub issue: Option<&'a IssueFilter>,
    pub mode: Option<Mode>,
    /// Keeps entries with at least one issue of this category.
    pub category: Option<IssueCategory>,
    /// Keeps entries of the last N days.
    pub since_days: Option<u32>,
    /// Keeps entries whose result is in this language (ISO 639-1 code; entries saved before
    /// T52 count as the default target, see [`target_language_sql`]).
    pub target_language: Option<&'a str>,
}

/// The target language of the row `t` in SQL: NULL (entries saved before T52) is the default
/// target, as in [`languages_from_row`].
fn target_language_sql() -> String {
    // The default is a fixed ISO code, never user input, so it can be inlined.
    format!(
        "COALESCE(t.target_language, '{}')",
        language::DEFAULT_TARGET
    )
}

/// The distinct target languages of the saved entries (ISO 639-1 codes, sorted by code).
pub fn target_languages(conn: &Connection) -> AppResult<Vec<String>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT DISTINCT {} AS lang FROM translations t ORDER BY lang",
        target_language_sql()
    ))?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// Newest first.
pub fn list(
    conn: &Connection,
    filters: &ListFilters<'_>,
    limit: u32,
    offset: u32,
) -> AppResult<Vec<HistoryEntrySummary>> {
    let ListFilters {
        query,
        issue,
        mode,
        category,
        since_days,
        target_language,
    } = *filters;
    let mut sql = String::from(
        "SELECT t.id, t.created_at, t.mode, t.provider, t.model, t.source_text, t.output_text, \
         (SELECT COUNT(*) FROM issues i WHERE i.translation_id = t.id), \
         t.accepted_target, t.accepted_session_id, t.accepted_session_name, \
         t.source_language, t.target_language \
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
    if let Some(m) = mode {
        args.push(Box::new(enum_to_str(&m)?));
        sql.push_str(&format!(" AND t.mode = ?{}", args.len()));
    }
    if let Some(c) = category {
        args.push(Box::new(enum_to_str(&c)?));
        sql.push_str(&format!(
            " AND EXISTS (SELECT 1 FROM issues i WHERE i.translation_id = t.id \
             AND i.category = ?{})",
            args.len()
        ));
    }
    if let Some(d) = since_days {
        args.push(Box::new(format!("-{d} days")));
        sql.push_str(&format!(
            " AND t.created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?{})",
            args.len()
        ));
    }
    if let Some(lang) = target_language {
        args.push(Box::new(lang.to_owned()));
        sql.push_str(&format!(" AND {} = ?{}", target_language_sql(), args.len()));
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
            let (source_language, target_language) = languages_from_row(row.get(11)?, row.get(12)?);
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
                source_language,
                target_language,
            })
        },
    )?;
    Ok(rows.collect::<Result<_, _>>()?)
}

/// One entry with its issues, or `None` when the id doesn't exist.
pub fn get(conn: &Connection, id: i64) -> AppResult<Option<HistoryEntry>> {
    let entry = conn.query_row(
        "SELECT id, created_at, mode, provider, model, preset_id, source_text, output_text, \
         duration_ms, token_warnings, accepted_target, accepted_session_id, accepted_session_name, \
         source_language, target_language \
         FROM translations WHERE id = ?1",
        [id],
        |row| {
            let warnings: String = row.get(9)?;
            let token_warnings: Vec<TokenWarning> =
                serde_json::from_str(&warnings).map_err(|e| {
                    rusqlite::Error::FromSqlConversionFailure(9, Type::Text, Box::new(e))
                })?;
            let duration: i64 = row.get(8)?;
            let (source_language, target_language) = languages_from_row(row.get(13)?, row.get(14)?);
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
                source_language,
                target_language,
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

/// Source text as compared for "the same text": trimmed, whitespace runs collapsed, lower-cased.
fn normalize_source(text: &str) -> String {
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// Running total of one (original, corrected) pair; `sources` holds the distinct source texts.
struct PairAgg {
    original: String,
    corrected: String,
    category: IssueCategory,
    last_seen: String,
    sources: HashSet<String>,
}

fn add_pair(
    map: &mut HashMap<(String, String), PairAgg>,
    category: IssueCategory,
    original: &str,
    corrected: &str,
    source: &str,
    created_at: &str,
) {
    // Rows arrive newest first, so the first one seen supplies the displayed spelling.
    let agg = map
        .entry((original.to_lowercase(), corrected.to_lowercase()))
        .or_insert_with(|| PairAgg {
            original: original.to_owned(),
            corrected: corrected.to_owned(),
            category,
            last_seen: created_at.to_owned(),
            sources: HashSet::new(),
        });
    agg.sources.insert(source.to_owned());
}

/// The "my mistakes" aggregates, over entries of the last `since_days` days (all when `None`)
/// whose result is in `target_language` (every language when `None`).
/// Every count is per distinct source text, so re-running one text never inflates it.
pub fn mistakes_summary(
    conn: &Connection,
    since_days: Option<u32>,
    target_language: Option<&str>,
) -> AppResult<MistakesSummary> {
    // `-N days` modifier for strftime; NULL cut-off means "no limit".
    let since: Option<String> = since_days.map(|d| format!("-{d} days"));

    let mut stmt = conn.prepare(&format!(
        "SELECT i.category, i.original, i.corrected, t.source_text, t.created_at \
         FROM issues i JOIN translations t ON t.id = i.translation_id \
         WHERE (?1 IS NULL OR t.created_at >= strftime('%Y-%m-%dT%H:%M:%fZ', 'now', ?1)) \
         AND (?2 IS NULL OR {} = ?2) \
         ORDER BY t.created_at DESC, t.id DESC, i.id",
        target_language_sql()
    ))?;
    let rows = stmt.query_map(params![since, target_language], |row| {
        let category: IssueCategory = col_enum(row, 0)?;
        let original: String = row.get(1)?;
        let corrected: String = row.get(2)?;
        let source: String = row.get(3)?;
        let created_at: String = row.get(4)?;
        Ok((category, original, corrected, source, created_at))
    })?;

    let mut category_seen: HashSet<(String, String, String, String)> = HashSet::new();
    let mut category_counts: HashMap<String, (IssueCategory, u32)> = HashMap::new();
    let mut pairs: HashMap<(String, String), PairAgg> = HashMap::new();
    let mut words: HashMap<(String, String), PairAgg> = HashMap::new();

    for row in rows {
        let (category, original, corrected, source, created_at) = row?;
        let source = normalize_source(&source);
        let (lo, lc) = (original.to_lowercase(), corrected.to_lowercase());
        let key = enum_to_str(&category)?;
        if category_seen.insert((source.clone(), key.clone(), lo, lc)) {
            category_counts.entry(key).or_insert((category, 0)).1 += 1;
        }
        add_pair(
            &mut pairs,
            category,
            &original,
            &corrected,
            &source,
            &created_at,
        );
        if category == IssueCategory::Vocabulary {
            add_pair(
                &mut words,
                category,
                &original,
                &corrected,
                &source,
                &created_at,
            );
        }
    }

    let mut by_category: Vec<(String, CategoryCount)> = category_counts
        .into_iter()
        .map(|(key, (category, count))| (key, CategoryCount { category, count }))
        .collect();
    by_category.sort_by(|a, b| b.1.count.cmp(&a.1.count).then_with(|| a.0.cmp(&b.0)));
    let by_category = by_category.into_iter().map(|(_, c)| c).collect();

    let mut recurring: Vec<PairAgg> = pairs
        .into_values()
        .filter(|p| p.sources.len() >= 2)
        .collect();
    recurring.sort_by(|a, b| {
        b.sources
            .len()
            .cmp(&a.sources.len())
            .then_with(|| b.last_seen.cmp(&a.last_seen))
    });
    let recurring = recurring
        .into_iter()
        .take(MAX_RECURRING)
        .map(|p| RecurringMistake {
            count: u32::try_from(p.sources.len()).unwrap_or(u32::MAX),
            original: p.original,
            corrected: p.corrected,
            category: p.category,
        })
        .collect();

    let mut vocabulary: Vec<PairAgg> = words.into_values().collect();
    vocabulary.sort_by(|a, b| {
        b.sources
            .len()
            .cmp(&a.sources.len())
            .then_with(|| b.last_seen.cmp(&a.last_seen))
    });
    let vocabulary = vocabulary
        .into_iter()
        .take(MAX_VOCABULARY)
        .map(|p| VocabularyItem {
            count: u32::try_from(p.sources.len()).unwrap_or(u32::MAX),
            original: p.original,
            corrected: p.corrected,
            last_seen: p.last_seen,
        })
        .collect();

    Ok(MistakesSummary {
        by_category,
        recurring,
        vocabulary,
    })
}
