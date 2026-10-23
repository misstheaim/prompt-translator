//! Data contracts for translation (plan §4.3), exported to TypeScript via ts-rs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum Mode {
    Translate,
    Improve,
    Correct,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum ProviderKind {
    Ollama,
    ClaudeCode,
    /// Only in debug builds / the dev setting.
    Mock,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TranslationRequest {
    /// UUID from the UI, used for cancel.
    pub request_id: String,
    pub source_text: String,
    pub mode: Mode,
    pub provider: ProviderKind,
    pub preset_id: String,
}

/// Matches the LLM output schema, so `snake_case`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export, rename_all = "snake_case")]
pub enum IssueCategory {
    Spelling,
    Grammar,
    Article,
    Tense,
    Preposition,
    WordChoice,
    /// A non-English word or phrase the user didn't know, replaced by English.
    Vocabulary,
    Punctuation,
    Style,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Issue {
    pub original: String,
    pub corrected: String,
    pub category: IssueCategory,
    pub explanation: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename_all = "camelCase")]
pub enum TokenKind {
    TicketId,
    Identifier,
    FileName,
    Path,
    Url,
    CodeSpan,
    Number,
}

/// A token present in the source but missing verbatim from the output.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TokenWarning {
    pub token: String,
    pub kind: TokenKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct TranslationResult {
    /// `None` when history is off.
    #[ts(type = "number | null")]
    pub history_id: Option<i64>,
    pub output: String,
    /// Empty for Translate.
    pub issues: Vec<Issue>,
    pub token_warnings: Vec<TokenWarning>,
    pub provider: ProviderKind,
    pub model: String,
    #[ts(type = "number")]
    pub duration_ms: u64,
}
