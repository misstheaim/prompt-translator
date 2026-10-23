//! Technical token check (plan §4.5): finds tokens in the source (ticket IDs,
//! identifiers, file names, paths, URLs, code spans) that are missing verbatim
//! from the output. Plain words, misspellings and non-English words are never
//! tokens.

use std::sync::LazyLock;

use regex::Regex;

use super::model::{TokenKind, TokenWarning};

const FILE_EXTENSIONS: &str = "json|jsonc|rs|ts|tsx|js|jsx|mjs|cjs|py|md|txt|toml|yaml|yml|cs|csproj|sln|xml|html|css|scss|sql|sh|ps1|bat|cmd|log|lock|cfg|ini|env|exe|dll|png|jpg|jpeg|gif|svg|pdf|csv|go|java|kt|c|h|cpp|hpp|rb|php|zip|config|conf|vue|dockerfile";

static CODE_SPAN: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"`[^`\r\n]+`").unwrap());
static URL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?i)\bhttps?://[^\s<>"'`]+"#).unwrap());
static WIN_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(?-u:\b)[A-Za-z]:\\[^\s"'<>|*?`]+"#).unwrap());
static UNIX_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?:[~.]{0,2}/)?[A-Za-z0-9_.\-]+(?:/[A-Za-z0-9_.\-]+)+").unwrap());
static FILE_NAME: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"(?i)(?-u:\b)[A-Za-z0-9_][A-Za-z0-9_\-.]*\.(?:{FILE_EXTENSIONS})(?-u:\b)"
    ))
    .unwrap()
});
static TICKET_ID: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u:\b)[A-Z][A-Z0-9]+-\d+(?-u:\b)").unwrap());
static ISSUE_REF: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#\d+(?-u:\b)").unwrap());
static SNAKE_CASE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?-u:\b)[A-Za-z][A-Za-z0-9]*(?:_[A-Za-z0-9]+)+(?-u:\b)").unwrap()
});
static CAMEL_CASE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?-u:\b)[A-Za-z][a-z0-9]*[A-Z][A-Za-z0-9]*(?-u:\b)").unwrap());

/// Collects every match of `re` (after `accept` maps it to the token text, or
/// rejects it) and blanks it out of `text` so later patterns can't match inside it.
fn extract(
    text: &mut String,
    re: &Regex,
    kind: TokenKind,
    found: &mut Vec<(String, TokenKind)>,
    accept: impl Fn(&str) -> Option<String>,
) {
    let replaced = re.replace_all(text, |caps: &regex::Captures| {
        let m = &caps[0];
        match accept(m) {
            Some(token) => {
                // Keep any trimmed tail so it stays in the text.
                let tail = m.strip_prefix(token.as_str()).unwrap_or("").to_string();
                found.push((token, kind));
                format!(" {tail}")
            }
            None => m.to_string(),
        }
    });
    *text = replaced.into_owned();
}

fn trim_trailing_punct(m: &str) -> Option<String> {
    let t = m.trim_end_matches(['.', ',', ';', ':', '!', '?', ')', ']']);
    (!t.is_empty()).then(|| t.to_string())
}

fn accept_unix_path(m: &str) -> Option<String> {
    let t = m.trim_end_matches('.');
    let segments: Vec<&str> = t.split('/').filter(|s| !s.is_empty()).collect();
    if segments.len() < 2 && !t.starts_with(['/', '~', '.']) {
        return None;
    }
    if !t.chars().any(|c| c.is_ascii_alphabetic()) {
        return None; // dates like 12/05/2024
    }
    let anchored = t.starts_with(['/', '~', '.']);
    let last_has_ext = segments
        .last()
        .is_some_and(|s| s.contains('.') && !s.starts_with('.') && !s.ends_with('.'));
    (anchored || segments.len() >= 3 || last_has_ext).then(|| t.to_string())
}

fn is_plural_acronym(m: &str) -> bool {
    let base = m.strip_suffix('s').unwrap_or(m);
    !base.chars().any(|c| c.is_ascii_lowercase())
}

/// Returns the technical tokens found in `source` that don't appear verbatim in
/// `output`, deduplicated, in order of first appearance per pattern group.
pub fn check_tokens(source: &str, output: &str) -> Vec<TokenWarning> {
    let mut found: Vec<(String, TokenKind)> = Vec::new();
    let mut text = source.to_string();

    extract(
        &mut text,
        &CODE_SPAN,
        TokenKind::CodeSpan,
        &mut found,
        |m| Some(m.to_string()),
    );
    extract(
        &mut text,
        &URL,
        TokenKind::Url,
        &mut found,
        trim_trailing_punct,
    );
    extract(
        &mut text,
        &WIN_PATH,
        TokenKind::Path,
        &mut found,
        trim_trailing_punct,
    );
    extract(
        &mut text,
        &UNIX_PATH,
        TokenKind::Path,
        &mut found,
        accept_unix_path,
    );
    extract(
        &mut text,
        &FILE_NAME,
        TokenKind::FileName,
        &mut found,
        |m| Some(m.to_string()),
    );
    extract(
        &mut text,
        &TICKET_ID,
        TokenKind::TicketId,
        &mut found,
        |m| Some(m.to_string()),
    );
    extract(
        &mut text,
        &ISSUE_REF,
        TokenKind::TicketId,
        &mut found,
        |m| Some(m.to_string()),
    );
    extract(
        &mut text,
        &SNAKE_CASE,
        TokenKind::Identifier,
        &mut found,
        |m| Some(m.to_string()),
    );
    extract(
        &mut text,
        &CAMEL_CASE,
        TokenKind::Identifier,
        &mut found,
        |m| (!is_plural_acronym(m)).then(|| m.to_string()),
    );

    let mut seen = std::collections::HashSet::new();
    found
        .into_iter()
        .filter(|(token, _)| !output.contains(token.as_str()) && seen.insert(token.clone()))
        .map(|(token, kind)| TokenWarning { token, kind })
        .collect()
}
