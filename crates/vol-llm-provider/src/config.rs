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

    #[test]
    fn test_config_new_and_with_body_headers() {
        let mut body = HashMap::new();
        body.insert("max_tokens".to_string(), serde_json::json!(4096));
        let mut headers = HashMap::new();
        headers.insert("anthropic-version".to_string(), "2023-06-01".to_string());

        let config = LLMConfig::new(
            LLMProvider::Anthropic,
            "claude-test",
            Secret::literal("sk-key"),
            "https://api.test.com",
        )
        .with_body(body.clone())
        .with_headers(headers.clone());

        assert_eq!(config.provider, LLMProvider::Anthropic);
        assert_eq!(config.model, "claude-test");
        assert_eq!(config.base_url, "https://api.test.com");
        assert_eq!(config.resolve_api_key().unwrap(), "sk-key");
        assert_eq!(config.body.as_ref().unwrap(), &body);
        assert_eq!(config.headers.as_ref().unwrap(), &headers);
    }

    #[test]
    fn test_config_resolve_api_key_missing_env_fails() {
        std::env::remove_var("CONFIG_MISSING_TEST_KEY");
        let config = LLMConfig::with_env_key(
            LLMProvider::Anthropic,
            "claude-test",
            "CONFIG_MISSING_TEST_KEY",
            "https://api.test.com",
        );
        let err = config.resolve_api_key().unwrap_err();
        assert!(matches!(err, vol_llm_core::LLMError::Auth(_)));
        assert!(err.to_string().contains("CONFIG_MISSING_TEST_KEY"));
    }

    #[test]
    fn test_config_serde_with_body_and_headers() {
        let mut body = HashMap::new();
        body.insert("temperature".to_string(), serde_json::json!(0.3));
        let mut headers = HashMap::new();
        headers.insert("x-trace".to_string(), "abc".to_string());

        let config = LLMConfig::with_literal_key(
            LLMProvider::OpenAI,
            "gpt-4o",
            "sk-test",
            "https://api.test.com",
        )
        .with_body(body)
        .with_headers(headers);

        // Serialization must include body/headers and a literal api_key
        let json = serde_json::to_string(&config).unwrap();
        assert!(json.contains(r#""provider":"openai""#));
        assert!(json.contains(r#""model":"gpt-4o""#));
        assert!(json.contains(r#""body":{"temperature":0.3}"#));
        assert!(json.contains(r#""headers":{"x-trace":"abc"}"#));
        assert!(json.contains(r#""Literal":"sk-test""#));

        // Deserialization accepts the literal-key form
        let json_literal = json.replace(r#"{"Literal":"sk-test"}"#, r#""sk-test""#);
        let parsed: LLMConfig = serde_json::from_str(&json_literal).unwrap();
        assert_eq!(parsed.provider, LLMProvider::OpenAI);
        assert_eq!(parsed.model, "gpt-4o");
        assert_eq!(parsed.base_url, "https://api.test.com");
        assert_eq!(parsed.body.as_ref().unwrap()["temperature"], 0.3);
        assert_eq!(parsed.headers.as_ref().unwrap()["x-trace"], "abc");
        assert_eq!(parsed.resolve_api_key().unwrap(), "sk-test");
    }

    #[test]
    fn test_provider_file_config_to_llm_config() {
        let file_config: ProviderFileConfig = toml::from_str(
            r#"
provider = "openai"
model = "gpt-4o"
api_key = "${FILE_KEY}"
base_url = "https://api.file.com"

[body]
max_tokens = 2048

[headers]
"x-file" = "1"
"#,
        )
        .unwrap();

        let config = file_config.to_llm_config();
        assert_eq!(config.provider, LLMProvider::OpenAI);
        assert_eq!(config.model, "gpt-4o");
        assert_eq!(config.base_url, "https://api.file.com");
        assert_eq!(config.body.as_ref().unwrap()["max_tokens"], 2048);
        assert_eq!(config.headers.as_ref().unwrap()["x-file"], "1");

        std::env::set_var("FILE_KEY", "file-key-value");
        assert_eq!(config.resolve_api_key().unwrap(), "file-key-value");
        std::env::remove_var("FILE_KEY");
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
