//! LLM configuration.

use crate::secret::Secret;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use vol_llm_core::LLMProvider;

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
    /// Default body parameters (provider-specific), merged at runtime
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<HashMap<String, serde_json::Value>>,
    /// Custom HTTP headers, attached to every request
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
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
            body: None,
            headers: None,
        }
    }

    /// Set default body parameters
    pub fn with_body(mut self, body: HashMap<String, serde_json::Value>) -> Self {
        self.body = Some(body);
        self
    }

    /// Set custom headers
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = Some(headers);
        self
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

/// File-level provider configuration, parsed from a single TOML file.
/// Filename (without .toml extension) is the provider ID.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderFileConfig {
    pub provider: LLMProvider,
    pub model: String,
    pub api_key: Secret,
    pub base_url: String,
    #[serde(default)]
    pub body: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

impl ProviderFileConfig {
    /// Convert to LLMConfig (for backward compatibility with existing factory)
    pub fn to_llm_config(&self) -> LLMConfig {
        LLMConfig {
            provider: self.provider,
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            body: self.body.clone(),
            headers: self.headers.clone(),
        }
    }
}
