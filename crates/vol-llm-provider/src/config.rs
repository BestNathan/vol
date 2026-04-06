//! LLM configuration.

use serde::{Deserialize, Serialize};
use vol_llm_core::LLMProvider;
use crate::secret::Secret;

/// LLM configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMConfig {
    /// Provider type
    pub provider: LLMProvider,
    /// Model name
    pub model: String,
    /// API key (literal or environment variable reference)
    pub api_key: Secret,
    /// Base URL for API endpoint
    pub base_url: String,
}

impl LLMConfig {
    /// Create a new LLMConfig
    pub fn new(
        provider: LLMProvider,
        model: impl Into<String>,
        api_key: Secret,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            api_key,
            base_url: base_url.into(),
        }
    }

    /// Resolve API key from secret
    pub fn resolve_api_key(&self) -> Result<String, vol_llm_core::LLMError> {
        self.api_key.resolve()
    }

    /// Create config with literal API key (convenience for testing)
    pub fn with_literal_key(
        provider: LLMProvider,
        model: impl Into<String>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self::new(provider, model, Secret::literal(api_key), base_url)
    }

    /// Create config with environment variable (convenience for production)
    pub fn with_env_key(
        provider: LLMProvider,
        model: impl Into<String>,
        env_var: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self::new(provider, model, Secret::env(env_var), base_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::LLMProvider;

    #[test]
    fn test_config_with_literal_key() {
        let config = LLMConfig::with_literal_key(
            LLMProvider::Anthropic,
            "claude-test",
            "sk-test-key",
            "https://api.test.com",
        );
        assert_eq!(config.resolve_api_key().unwrap(), "sk-test-key");
    }

    #[test]
    fn test_config_with_env_key() {
        std::env::set_var("TEST_API_KEY", "env-key");
        let config = LLMConfig::with_env_key(
            LLMProvider::Anthropic,
            "claude-test",
            "TEST_API_KEY",
            "https://api.test.com",
        );
        assert_eq!(config.resolve_api_key().unwrap(), "env-key");
    }
}
