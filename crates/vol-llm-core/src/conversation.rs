//! Conversation request and response types.

use crate::{Message, ModelConfig, ToolChoice, ToolDefinition};
use serde::{Deserialize, Serialize};

/// Conversation request
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConversationRequest {
    /// System prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Conversation history
    pub messages: Vec<Message>,
    /// Model parameters
    #[serde(default)]
    pub model_config: ModelConfig,
    /// Tool definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// Tool choice strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Stream response
    #[serde(default)]
    pub stream: bool,
}

/// Conversation response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConversationResponse {
    /// Generated message
    pub message: Message,
    /// Model used
    pub model: String,
    /// Token usage
    pub usage: TokenUsage,
    /// Finish reason
    pub finish_reason: FinishReason,
    /// Raw provider response (for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u32>,
}

/// Finish reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other,
}

impl ConversationRequest {
    /// Create simple request
    pub fn simple(prompt: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(prompt.into())],
            ..Default::default()
        }
    }

    /// Create with system prompt
    pub fn with_system(system: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            system: Some(system.into()),
            messages: vec![Message::user(prompt.into())],
            ..Default::default()
        }
    }

    /// Create with history
    pub fn with_history(system: Option<String>, messages: Vec<Message>) -> Self {
        Self {
            system,
            messages,
            ..Default::default()
        }
    }

    /// Builder: set tools
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }

    /// Builder: set tool_choice
    pub fn with_tool_choice(mut self, tool_choice: ToolChoice) -> Self {
        self.tool_choice = Some(tool_choice);
        self
    }

    /// Builder: set max_tokens
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.model_config.max_tokens = Some(max);
        self
    }

    /// Builder: set temperature
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.model_config.temperature = Some(temp.clamp(0.0, 2.0));
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_request() {
        let req = ConversationRequest::simple("Hello");
        assert_eq!(req.messages.len(), 1);
        assert!(req.system.is_none());
    }

    #[test]
    fn test_builder_pattern() {
        let req = ConversationRequest::simple("Test")
            .with_max_tokens(500)
            .with_temperature(0.7);
        assert_eq!(req.model_config.max_tokens, Some(500));
        assert_eq!(req.model_config.temperature, Some(0.7));
    }
}
