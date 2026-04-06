//! LLM configuration.

use serde::{Deserialize, Serialize};
use vol_llm_core::{LLMProvider, LLMError};

/// LLM configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMConfig {
    /// Provider type
    pub provider: LLMProvider,
    /// Model name
    pub model: String,
    /// API key environment variable
    pub api_key_env: String,
    /// Custom endpoint (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

impl LLMConfig {
    /// Get API key from environment
    pub fn api_key(&self) -> Result<String, LLMError> {
        std::env::var(&self.api_key_env)
            .map_err(|_| LLMError::Auth(format!(
                "API key environment variable '{}' not set",
                self.api_key_env
            )))
    }
}
