//! Model configuration and info types.

use serde::{Deserialize, Serialize};

/// Model parameters
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelConfig {
    /// Maximum generation tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Top-p (nucleus sampling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Top-k
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Frequency penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    /// Presence penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Random seed for reproducibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Logprobs level (0 - 20)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<u32>,
}

/// Model information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    /// Model name
    pub name: String,
    /// Maximum context tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_context_tokens: Option<u32>,
    /// Maximum output tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_output_tokens: Option<u32>,
    /// Supports tool calling
    pub supports_tools: bool,
    /// Supports streaming responses
    pub supports_streaming: bool,
    /// Supports vision (images)
    pub supports_vision: bool,
}

impl ModelInfo {
    /// Create a new model info
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            max_context_tokens: None,
            max_output_tokens: None,
            supports_tools: false,
            supports_streaming: false,
            supports_vision: false,
        }
    }

    /// Set context tokens
    pub fn context_tokens(mut self, tokens: u32) -> Self {
        self.max_context_tokens = Some(tokens);
        self
    }

    /// Set output tokens
    pub fn output_tokens(mut self, tokens: u32) -> Self {
        self.max_output_tokens = Some(tokens);
        self
    }

    /// Set tool support
    pub fn tools(mut self, supported: bool) -> Self {
        self.supports_tools = supported;
        self
    }

    /// Set streaming support
    pub fn streaming(mut self, supported: bool) -> Self {
        self.supports_streaming = supported;
        self
    }

    /// Set vision support
    pub fn vision(mut self, supported: bool) -> Self {
        self.supports_vision = supported;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert!(config.max_tokens.is_none());
        assert!(config.temperature.is_none());
    }

    #[test]
    fn test_model_config_with_values() {
        let config = ModelConfig {
            max_tokens: Some(1024),
            temperature: Some(0.7),
            ..Default::default()
        };
        assert_eq!(config.max_tokens, Some(1024));
        assert_eq!(config.temperature, Some(0.7));
    }

    #[test]
    fn test_model_info_builder() {
        let info = ModelInfo::new("claude-3-sonnet")
            .context_tokens(200_000)
            .output_tokens(4096)
            .tools(true)
            .streaming(true);

        assert_eq!(info.name, "claude-3-sonnet");
        assert_eq!(info.max_context_tokens, Some(200_000));
        assert_eq!(info.max_output_tokens, Some(4096));
        assert!(info.supports_tools);
        assert!(info.supports_streaming);
    }

    #[test]
    fn test_model_info_vision_builder() {
        let default = ModelInfo::new("gpt-4o");
        assert!(!default.supports_vision);

        let with_vision = ModelInfo::new("gpt-4o").vision(true);
        assert!(with_vision.supports_vision);

        let without_vision = ModelInfo::new("gpt-4o").vision(false);
        assert!(!without_vision.supports_vision);
    }

    #[test]
    fn test_model_config_serde_roundtrip() {
        let config = ModelConfig {
            max_tokens: Some(2048),
            temperature: Some(0.2),
            top_p: Some(0.9),
            top_k: Some(40),
            frequency_penalty: Some(-0.5),
            presence_penalty: Some(1.5),
            stop: Some(vec!["END".to_string()]),
            seed: Some(42),
            logprobs: Some(5),
        };
        let json = serde_json::to_string(&config).unwrap();
        let parsed: ModelConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_tokens, Some(2048));
        assert_eq!(parsed.temperature, Some(0.2));
        assert_eq!(parsed.top_p, Some(0.9));
        assert_eq!(parsed.top_k, Some(40));
        assert_eq!(parsed.frequency_penalty, Some(-0.5));
        assert_eq!(parsed.presence_penalty, Some(1.5));
        assert_eq!(parsed.stop, Some(vec!["END".to_string()]));
        assert_eq!(parsed.seed, Some(42));
        assert_eq!(parsed.logprobs, Some(5));
        // None fields are skipped on serialization
        let empty = ModelConfig::default();
        assert_eq!(serde_json::to_string(&empty).unwrap(), "{}");
        let empty_parsed: ModelConfig = serde_json::from_str("{}").unwrap();
        assert_eq!(empty_parsed.temperature, None);
    }

    #[test]
    fn test_model_info_serde_roundtrip() {
        let info = ModelInfo::new("qwen3.6-plus")
            .context_tokens(128_000)
            .output_tokens(8192)
            .tools(true)
            .streaming(true)
            .vision(true);
        let json = serde_json::to_string(&info).unwrap();
        let parsed: ModelInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.name, "qwen3.6-plus");
        assert_eq!(parsed.max_context_tokens, Some(128_000));
        assert_eq!(parsed.max_output_tokens, Some(8192));
        assert!(parsed.supports_tools);
        assert!(parsed.supports_streaming);
        assert!(parsed.supports_vision);
    }
}
