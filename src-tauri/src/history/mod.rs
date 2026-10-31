//! SQLite translation history (plan §4.7, T27) and the "my mistakes" summary (plan §4.6).

pub mod commands;
pub mod db;
pub mod repo;

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::translation::model::{Issue, IssueCategory, Mode, ProviderKind, TokenWarning};

pub use db::HistoryDb;

/// Where an accepted result went.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[ts(export)]
pub enum AcceptedTarget {
    Clipboard,
    Session { id: String, name: String },
}

/// A row of the history list: the first 120 characters of source and output.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryEntrySummary {
    #[ts(type = "number")]
    pub id: i64,
    pub created_at: String,
    pub mode: Mode,
    pub provider: ProviderKind,
    pub model: String,
    pub source_preview: String,
    pub output_preview: String,
    pub issue_count: u32,
    pub accepted: Option<AcceptedTarget>,
    /// ISO 639-1 code of the language translated from (`ru` for entries saved before T52).
    pub source_language: String,
    /// ISO 639-1 code of the result's language (`en` for entries saved before T52).
    pub target_language: String,
}

/// A full history entry with its issues.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HistoryEntry {
    #[ts(type = "number")]
    pub id: i64,
    pub created_at: String,
    pub mode: Mode,
    pub provider: ProviderKind,
    pub model: String,
    pub preset_id: String,
    pub source_text: String,
    pub output_text: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
    pub token_warnings: Vec<TokenWarning>,
    pub accepted: Option<AcceptedTarget>,
    pub issues: Vec<Issue>,
    /// ISO 639-1 code of the language translated from (`ru` for entries saved before T52).
    pub source_language: String,
    /// ISO 639-1 code of the result's language (`en` for entries saved before T52).
    pub target_language: String,
}

/// Filter of `history_list`: entries containing this mistake (case-insensitive).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IssueFilter {
    pub original: String,
    pub corrected: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct CategoryCount {
    pub category: IssueCategory,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RecurringMistake {
    pub original: String,
    pub corrected: String,
    pub category: IssueCategory,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct VocabularyItem {
    pub original: String,
    pub corrected: String,
    pub count: u32,
    /// ISO 8601 UTC time of the latest entry that contained it.
    pub last_seen: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MistakesSummary {
    pub by_category: Vec<CategoryCount>,
    pub recurring: Vec<RecurringMistake>,
    pub vocabulary: Vec<VocabularyItem>,
}
