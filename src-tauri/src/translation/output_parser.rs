//! Parses a model's answer into `output` + `issues` (plan §4.3).
//!
//! Rules:
//! - The answer is trimmed and parsed strictly as JSON. If that fails, a ```` ``` ```` fence
//!   around it (with or without a language tag) is stripped and it is parsed again.
//! - When the answer has prose before or after the JSON object (fenced or not), the first JSON
//!   object in it that has an `output` field is used.
//! - An unknown or missing `category` becomes [`IssueCategory::Other`]; an `issues` entry that
//!   isn't an object with string `original` and `corrected` fields is skipped. A missing or
//!   `null` `issues` field means no issues.
//! - An `issues` entry whose trimmed `original` equals its `corrected` is dropped (a no-op).
//! - A blank answer, an empty fence, or a blank or `null` `output` is [`AppError::EmptyOutput`].
//! - Anything else that can't be read is [`AppError::MalformedOutput`], with the raw answer as
//!   the error's detail.
//! - Translate mode drops any issues the model returns.

use serde_json::{Map, Value};

use super::model::{Issue, IssueCategory, Mode};
use crate::error::{AppError, AppResult};

/// At most this many characters of the raw answer are kept in a `MalformedOutput` error.
const MAX_DETAIL_CHARS: usize = 10_000;

/// At most this many `{` positions are tried when looking for JSON inside prose.
const MAX_OBJECT_STARTS: usize = 50;

/// The parsed answer of a model.
#[derive(Debug, Clone)]
pub struct ParsedOutput {
    pub output: String,
    /// Always empty in Translate mode.
    pub issues: Vec<Issue>,
}

/// Parses a raw text answer (Ollama's `response`, or any provider's plain text).
pub fn parse(raw: &str, mode: Mode) -> AppResult<ParsedOutput> {
    let text = raw.trim();
    // A valid answer may contain fences inside its strings (code blocks in the prompt), so
    // strict parsing of the whole answer comes first.
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return from_value(value, mode, raw);
    }
    let text = strip_fences(text);
    if text.is_empty() {
        return Err(AppError::EmptyOutput);
    }
    let value = find_json(text).ok_or_else(|| malformed(raw))?;
    from_value(value, mode, raw)
}

/// Parses an already-parsed JSON answer (Claude's structured output). A JSON string is parsed
/// as a raw text answer.
pub fn parse_value(value: Value, mode: Mode) -> AppResult<ParsedOutput> {
    match value {
        Value::String(raw) => parse(&raw, mode),
        Value::Null => Err(AppError::EmptyOutput),
        value => {
            let raw = value.to_string();
            from_value(value, mode, &raw)
        }
    }
}

/// Removes a Markdown code fence around the whole text: the opening ```` ``` ```` with its
/// language tag (such as `json`) and the closing ```` ``` ````. Text that doesn't start with a
/// fence is returned as it is; a fenced block after prose is found by [`find_json`].
fn strip_fences(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    let rest = rest.trim_start_matches(|c: char| c.is_ascii_alphanumeric());
    rest.strip_suffix("```").unwrap_or(rest).trim()
}

/// The whole text as JSON, or else the first JSON object inside it that has an `output` field.
fn find_json(text: &str) -> Option<Value> {
    if let Ok(value) = serde_json::from_str::<Value>(text) {
        return Some(value);
    }
    text.match_indices('{')
        .take(MAX_OBJECT_STARTS)
        .find_map(|(start, _)| {
            let mut stream =
                serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>();
            match stream.next() {
                Some(Ok(value @ Value::Object(_))) if value.get("output").is_some() => Some(value),
                _ => None,
            }
        })
}

fn from_value(value: Value, mode: Mode, raw: &str) -> AppResult<ParsedOutput> {
    let Value::Object(object) = value else {
        return Err(malformed(raw));
    };
    let output = match object.get("output") {
        Some(Value::Null) => return Err(AppError::EmptyOutput),
        Some(Value::String(output)) => output.trim().to_string(),
        None | Some(_) => return Err(malformed(raw)),
    };
    if output.is_empty() {
        return Err(AppError::EmptyOutput);
    }

    let issues = match (mode, object.get("issues")) {
        (Mode::Translate, _) | (_, None | Some(Value::Null)) => Vec::new(),
        (_, Some(Value::Array(items))) => items
            .iter()
            .filter_map(parse_issue)
            .filter(|issue| {
                // A no-op entry ("remote?" -> "remote?") teaches nothing; small models add them.
                let changed = issue.original != issue.corrected;
                if !changed {
                    log::warn!("Dropping an issue whose `original` equals `corrected`");
                }
                changed
            })
            .collect(),
        (_, Some(_)) => return Err(malformed(raw)),
    };

    Ok(ParsedOutput { output, issues })
}

fn parse_issue(item: &Value) -> Option<Issue> {
    let Value::Object(fields) = item else {
        log::warn!("Skipping an issue that isn't a JSON object");
        return None;
    };
    let (Some(original), Some(corrected)) =
        (string(fields, "original"), string(fields, "corrected"))
    else {
        log::warn!("Skipping an issue without `original` or `corrected`");
        return None;
    };
    Some(Issue {
        original,
        corrected,
        category: fields
            .get("category")
            .and_then(Value::as_str)
            .map_or(IssueCategory::Other, category),
        explanation: string(fields, "explanation").unwrap_or_default(),
    })
}

fn string(fields: &Map<String, Value>, key: &str) -> Option<String> {
    fields
        .get(key)
        .and_then(Value::as_str)
        .map(|s| s.trim().to_string())
}

/// Maps a category name to [`IssueCategory`], ignoring case, spaces, hyphens and underscores
/// (`word_choice`, `Word choice`, `word-choice`). Unknown names become `Other`.
fn category(name: &str) -> IssueCategory {
    let key: String = name
        .chars()
        .filter(|c| !matches!(c, '_' | '-' | ' '))
        .flat_map(char::to_lowercase)
        .collect();
    match key.as_str() {
        "spelling" => IssueCategory::Spelling,
        "grammar" => IssueCategory::Grammar,
        "article" | "articles" => IssueCategory::Article,
        "tense" | "tenses" => IssueCategory::Tense,
        "preposition" | "prepositions" => IssueCategory::Preposition,
        "wordchoice" => IssueCategory::WordChoice,
        "vocabulary" => IssueCategory::Vocabulary,
        "punctuation" => IssueCategory::Punctuation,
        "style" => IssueCategory::Style,
        _ => IssueCategory::Other,
    }
}

fn malformed(raw: &str) -> AppError {
    let detail = match raw.char_indices().nth(MAX_DETAIL_CHARS) {
        Some((cut, _)) => format!("{}…", &raw[..cut]),
        None => raw.to_string(),
    };
    AppError::MalformedOutput(detail)
}
