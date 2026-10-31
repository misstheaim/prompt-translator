//! Tauri commands of the `history` module.
//!
//! `HistoryDb` is not managed when the database couldn't be opened at startup, so every command
//! looks it up with `try_state` and returns a `Db` error when it is missing.

use rusqlite::Connection;
use tauri::{AppHandle, Manager};

use super::{HistoryDb, HistoryEntry, HistoryEntrySummary, IssueFilter, MistakesSummary, repo};
use crate::error::{AppError, AppResult};
use crate::translation::model::{IssueCategory, Mode};

/// Rows per page when the UI asks for more than this (or for nothing sensible).
const MAX_LIMIT: u32 = 500;

/// The managed database, or a `Db` error when it couldn't be opened at startup.
fn db(app: &AppHandle) -> AppResult<HistoryDb> {
    app.try_state::<HistoryDb>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| AppError::Db("The history database is unavailable".into()))
}

/// Runs a repo function on the blocking pool with the connection locked.
async fn run<T, F>(app: &AppHandle, f: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> AppResult<T> + Send + 'static,
{
    let db = db(app)?;
    tauri::async_runtime::spawn_blocking(move || f(&*db.conn()?))
        .await
        .map_err(|e| AppError::Db(format!("History task failed: {e}")))?
}

/// Marks an entry as accepted with `target`. Used by `copy_result` (and later `accept_result`).
/// Never fails the caller: a missing database or a DB error is only logged.
pub async fn mark_accepted(app: &AppHandle, id: i64, target: super::AcceptedTarget) {
    let marked = run(app, move |conn| repo::mark_accepted(conn, id, &target)).await;
    if let Err(e) = marked {
        log::warn!("Couldn't mark history entry {id} as accepted: {e:?}");
    }
}

/// Newest first; `query` searches source and output, `issue` keeps entries with that mistake,
/// `mode`, `category` and `since_days` narrow further (all must match).
#[tauri::command]
#[allow(clippy::too_many_arguments)] // flat arguments are the command's contract with the UI
pub async fn history_list(
    app: AppHandle,
    query: Option<String>,
    issue: Option<IssueFilter>,
    mode: Option<Mode>,
    category: Option<IssueCategory>,
    since_days: Option<u32>,
    limit: u32,
    offset: u32,
) -> AppResult<Vec<HistoryEntrySummary>> {
    let limit = limit.clamp(1, MAX_LIMIT);
    run(&app, move |conn| {
        let filters = repo::ListFilters {
            query: query.as_deref(),
            issue: issue.as_ref(),
            mode,
            category,
            since_days,
        };
        repo::list(conn, &filters, limit, offset)
    })
    .await
}

#[tauri::command]
pub async fn history_get(app: AppHandle, id: i64) -> AppResult<HistoryEntry> {
    run(&app, move |conn| repo::get(conn, id))
        .await?
        .ok_or_else(|| AppError::InvalidInput("This history entry doesn't exist".into()))
}

#[tauri::command]
pub async fn history_delete(app: AppHandle, id: i64) -> AppResult<()> {
    run(&app, move |conn| repo::delete(conn, id)).await
}

/// Deletes the whole history. Turning history off never does this; only Clear does.
#[tauri::command]
pub async fn history_clear(app: AppHandle) -> AppResult<()> {
    run(&app, repo::clear).await
}

#[tauri::command]
pub async fn mistakes_summary(
    app: AppHandle,
    since_days: Option<u32>,
) -> AppResult<MistakesSummary> {
    run(&app, move |conn| repo::mistakes_summary(conn, since_days)).await
}
