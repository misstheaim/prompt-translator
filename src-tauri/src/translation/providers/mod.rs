//! Translation providers.

pub mod claude_code;
pub mod mock;
pub mod ollama;

use std::sync::Arc;

use super::service::TranslationProvider;

/// Every provider the app registers with the translation service. The service still refuses
/// the mock outside debug builds unless `devMockProvider` is on.
pub fn registry() -> Vec<Arc<dyn TranslationProvider>> {
    vec![Arc::new(mock::MockProvider)]
}
