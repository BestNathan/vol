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
}
