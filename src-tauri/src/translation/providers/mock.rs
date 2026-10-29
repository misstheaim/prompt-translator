//! The mock provider: canned answers per mode after 800 ms, for development and for checking
//! the service by hand. The service only uses it in debug builds or with `devMockProvider` on.
//!
//! The canned output never contains the source's technical tokens, so a ticket ID, path or
//! identifier in the source shows up as a token warning. Markers in the source text (any case)
//! pick other outcomes:
//!
//! | Marker | Outcome |
//! |---|---|
//! | `[mock:malformed]` | an answer that isn't JSON → `MalformedOutput` |
//! | `[mock:empty]` | an empty `output` → `EmptyOutput` |
//! | `[mock:slow]` | answers after 10 s instead of 800 ms (time to press Cancel) |
//! | `[mock:timeout]` | never answers within the mock's 5 s timeout → `Timeout` |

use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::error::{AppError, AppResult};
use crate::settings::Settings;
use crate::translation::model::{Mode, ProviderKind};
use crate::translation::prompt_builder::BuiltPrompt;
use crate::translation::service::{ProviderOutput, RawOutput, TranslationProvider};

const DELAY: Duration = Duration::from_millis(800);
const SLOW_DELAY: Duration = Duration::from_secs(10);
/// The mock has no timeout setting; this one is short so the timeout path is quick to check.
const TIMEOUT: Duration = Duration::from_secs(5);
const HANG_DELAY: Duration = Duration::from_secs(3600);
const MODEL: &str = "mock";

pub struct MockProvider;

#[async_trait]
impl TranslationProvider for MockProvider {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Mock
    }

    fn timeout(&self, _settings: &Settings) -> Duration {
        TIMEOUT
    }

    async fn complete(
        &self,
        prompt: &BuiltPrompt,
        _settings: &Settings,
        cancel: CancellationToken,
    ) -> AppResult<ProviderOutput> {
        let source = source_text(&prompt.user).to_lowercase();
        let delay = if source.contains("[mock:timeout]") {
            HANG_DELAY
        } else if source.contains("[mock:slow]") {
            SLOW_DELAY
        } else {
            DELAY
        };
        tokio::select! {
            () = cancel.cancelled() => return Err(AppError::Cancelled),
            () = tokio::time::sleep(delay) => {}
        }

        let raw = if source.contains("[mock:malformed]") {
            "Sorry, I can't help with that request.".to_string()
        } else if source.contains("[mock:empty]") {
            json!({ "output": "", "issues": [] }).to_string()
        } else {
            canned(prompt.mode)
        };
        Ok(ProviderOutput {
            raw: RawOutput::Text(raw),
            model: MODEL.into(),
        })
    }
}

/// The source text from the user message (`<source_text>\n…\n</source_text>`, see
/// `prompt_builder`); the whole message when the tags aren't there.
fn source_text(user: &str) -> &str {
    user.split_once("<source_text>\n")
        .and_then(|(_, rest)| rest.split_once("\n</source_text>"))
        .map_or(user, |(source, _)| source)
}

/// A canned answer in the LLM output format (plan §4.3).
fn canned(mode: Mode) -> String {
    let answer = match mode {
        Mode::Translate => json!({
            "output": "Please fix the bug on the settings screen and keep the current layout.",
            // Translate's schema allows no issues (`output_schema`).
            "issues": []
        }),
        Mode::Improve => json!({
            "output": "Fix the bug on the settings screen. Keep the current layout unchanged.",
            "issues": [
                {
                    "original": "please fix the bug and keep the layout",
                    "corrected": "Fix the bug. Keep the current layout unchanged.",
                    "category": "style",
                    "explanation": "Two short sentences are clearer than one long one."
                },
                {
                    "original": "screen of settings",
                    "corrected": "settings screen",
                    "category": "word_choice",
                    "explanation": "This is the usual name for this screen."
                }
            ]
        }),
        Mode::Correct => json!({
            "output": "I want to add a button that saves the settings.",
            "issues": [
                {
                    "original": "I want add",
                    "corrected": "I want to add",
                    "category": "grammar",
                    "explanation": "After \"want\" we use \"to\" before the verb."
                },
                {
                    "original": "кнопку",
                    "corrected": "a button",
                    "category": "vocabulary",
                    "explanation": "\"Кнопка\" is \"button\" in English."
                },
                {
                    // An unknown category: the parser maps it to `other`.
                    "original": "that save",
                    "corrected": "that saves",
                    "category": "agreement",
                    "explanation": "\"A button\" is one thing, so the verb ends in -s."
                }
            ]
        }),
    };
    answer.to_string()
}
