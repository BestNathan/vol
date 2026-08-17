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
    use crate::MessageRole;

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

    #[test]
    fn test_with_system() {
        let req = ConversationRequest::with_system("You are a bot", "Hello");
        assert_eq!(req.system.as_deref(), Some("You are a bot"));
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, MessageRole::User);
        assert_eq!(req.messages[0].content.as_ref().unwrap().as_str(), "Hello");
    }

    #[test]
    fn test_with_history() {
        let history = vec![
            Message::user("q1"),
            Message::assistant("a1"),
            Message::user("q2"),
        ];
        let req = ConversationRequest::with_history(Some("sys".to_string()), history.clone());
        assert_eq!(req.system.as_deref(), Some("sys"));
        assert_eq!(req.messages.len(), 3);
        assert_eq!(req.messages[1].role, MessageRole::Assistant);

        let no_system = ConversationRequest::with_history(None, history.clone());
        assert!(no_system.system.is_none());
        assert_eq!(no_system.messages.len(), 3);
    }

    #[test]
    fn test_with_tools() {
        let tools = vec![
            ToolDefinition {
                name: "get_weather".to_string(),
                description: Some("Get weather".to_string()),
                parameters: None,
            },
            ToolDefinition {
                name: "get_time".to_string(),
                description: None,
                parameters: Some(serde_json::json!({"type": "object"})),
            },
        ];
        let req = ConversationRequest::simple("x").with_tools(tools.clone());
        let configured = req.tools.as_ref().unwrap();
        assert_eq!(configured.len(), 2);
        assert_eq!(configured[0].name, "get_weather");
        assert_eq!(configured[0].description.as_deref(), Some("Get weather"));
        assert_eq!(configured[1].name, "get_time");
        assert!(configured[1].description.is_none());
        assert!(req.tool_choice.is_none());
    }

    #[test]
    fn test_with_tool_choice() {
        let auto = ConversationRequest::simple("x").with_tool_choice(ToolChoice::Auto);
        assert!(matches!(auto.tool_choice, Some(ToolChoice::Auto)));

        let specific = ConversationRequest::simple("x").with_tool_choice(ToolChoice::Specific {
            name: "get_weather".to_string(),
        });
        assert!(matches!(
            specific.tool_choice,
            Some(ToolChoice::Specific { name }) if name == "get_weather"
        ));
    }

    #[test]
    fn test_temperature_clamped() {
        let req = ConversationRequest::simple("x").with_temperature(3.5);
        assert_eq!(req.model_config.temperature, Some(2.0));
        let req = ConversationRequest::simple("x").with_temperature(-1.0);
        assert_eq!(req.model_config.temperature, Some(0.0));
    }

    #[test]
    fn test_conversation_request_serde_roundtrip() {
        let req = ConversationRequest::with_system("sys", "hi")
            .with_max_tokens(100)
            .with_tools(vec![ToolDefinition {
                name: "t".to_string(),
                description: None,
                parameters: None,
            }])
            .with_tool_choice(ToolChoice::Required);
        let json = serde_json::to_string(&req).unwrap();
        let parsed: ConversationRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.system, req.system);
        assert_eq!(parsed.messages.len(), 1);
        assert_eq!(parsed.model_config.max_tokens, Some(100));
        assert_eq!(parsed.tools.as_ref().unwrap()[0].name, "t");
        assert!(matches!(parsed.tool_choice, Some(ToolChoice::Required)));
        assert!(!parsed.stream);

        // Defaults for omitted fields
        let parsed: ConversationRequest = serde_json::from_str(r#"{"messages": []}"#).unwrap();
        assert!(parsed.system.is_none());
        assert!(parsed.tools.is_none());
        assert!(!parsed.stream);
    }

    #[test]
    fn test_finish_reason_serde_roundtrip() {
        for reason in [
            FinishReason::Stop,
            FinishReason::Length,
            FinishReason::ToolCalls,
            FinishReason::ContentFilter,
            FinishReason::Other,
        ] {
            let json = serde_json::to_string(&reason).unwrap();
            let parsed: FinishReason = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, reason);
        }
        assert_eq!(
            serde_json::to_string(&FinishReason::ToolCalls).unwrap(),
            r#""toolcalls""#
        );
    }

    #[test]
    fn test_token_usage_serde_roundtrip() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cached_tokens: Some(5),
        };
        let json = serde_json::to_string(&usage).unwrap();
        let parsed: TokenUsage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.prompt_tokens, 10);
        assert_eq!(parsed.completion_tokens, 20);
        assert_eq!(parsed.total_tokens, 30);
        assert_eq!(parsed.cached_tokens, Some(5));

        // cached_tokens omitted when None
        let no_cache = TokenUsage::default();
        let json = serde_json::to_string(&no_cache).unwrap();
        assert!(!json.contains("cached_tokens"));
    }

    #[test]
    fn test_conversation_response_serde_roundtrip() {
        let resp = ConversationResponse {
            message: Message::assistant("hi"),
            model: "mock".to_string(),
            usage: TokenUsage::default(),
            finish_reason: FinishReason::Stop,
            raw: Some(serde_json::json!({"id": "abc"})),
        };
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: ConversationResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.message.role, MessageRole::Assistant);
        assert_eq!(parsed.model, "mock");
        assert_eq!(parsed.finish_reason, FinishReason::Stop);
        assert_eq!(parsed.raw.as_ref().unwrap()["id"], "abc");
    }
}
