//! Translation providers.

pub mod claude_code;
pub mod mock;
pub mod ollama;

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use super::service::TranslationProvider;

/// Claude Code's state. Filled by `claude_code::status`.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClaudeStatus {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    /// `None` when not known.
    pub signed_in: Option<bool>,
}

/// What the `provider_status` command returns.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ProviderStatus {
    pub ollama: ollama::OllamaStatus,
    /// `None` only if the check could not run.
    pub claude: Option<ClaudeStatus>,
}

/// Every provider the app registers with the translation service. The service still refuses
/// the mock outside debug builds unless `devMockProvider` is on. `pids` and `housekeeping` are
/// handles to the app's shared translator-run registry and run counter.
pub fn registry(
    pids: claude_code::TranslatorPids,
    housekeeping: claude_code::Housekeeping,
) -> Vec<Arc<dyn TranslationProvider>> {
    vec![
        Arc::new(mock::MockProvider),
        Arc::new(ollama::OllamaProvider::new()),
        Arc::new(claude_code::ClaudeCodeProvider::new(pids, housekeeping)),
    ]
}
