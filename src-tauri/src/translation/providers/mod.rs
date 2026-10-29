//! Translation providers.

pub mod claude_code;
pub mod mock;
pub mod ollama;

use std::sync::Arc;

use serde::Serialize;
use ts_rs::TS;

use super::service::TranslationProvider;

/// Claude Code's state. T17a fills it in; until then `ProviderStatus.claude` stays `None`.
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
    /// `None` until T17a adds the Claude check.
    pub claude: Option<ClaudeStatus>,
}

/// Every provider the app registers with the translation service. The service still refuses
/// the mock outside debug builds unless `devMockProvider` is on.
pub fn registry() -> Vec<Arc<dyn TranslationProvider>> {
    vec![
        Arc::new(mock::MockProvider),
        Arc::new(ollama::OllamaProvider::new()),
    ]
}
