//! Message types for LLM conversation.

use crate::tool::ToolCall;
use serde::{Deserialize, Serialize};

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message - sets behavior and context
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool response message
    Tool,
}

/// Content part for multi-part messages (images, etc.)
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    Text { text: String },
    Image { image_url: ImageUrl },
}

/// Image URL for multi-part content
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Message content - text or multi-part
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text
    Text(String),
    /// Multi-part content
    MultiPart(Vec<ContentPart>),
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}

impl MessageContent {
    /// Get content as string (for text content)
    pub fn as_str(&self) -> &str {
        match self {
            MessageContent::Text(s) => s,
            MessageContent::MultiPart(_) => "",
        }
    }

    /// Text representation including image markers.
    /// Multi-part content renders each image part as `[image]`, joined with newlines.
    pub fn display_text(&self) -> String {
        match self {
            MessageContent::Text(s) => s.clone(),
            MessageContent::MultiPart(parts) => parts
                .iter()
                .map(|part| match part {
                    ContentPart::Text { text } => text.as_str(),
                    ContentPart::Image { .. } => "[image]",
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// Conversation message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    /// Message role
    pub role: MessageRole,
    /// Message content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    /// Tool calls (assistant messages only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID (tool messages only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optional name for the participant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Thinking content (assistant messages only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thinking: Option<String>,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::System,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            thinking: None,
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::User,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            thinking: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
            thinking: None,
        }
    }

    /// Create an assistant message with tool calls
    pub fn assistant_with_tools(
        content: impl Into<MessageContent>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Some(content.into()),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
            thinking: None,
        }
    }

    /// Create a tool response message
    pub fn tool(content: impl Into<MessageContent>, call_id: String) -> Self {
        Self {
            role: MessageRole::Tool,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(call_id),
            name: None,
            thinking: None,
        }
    }

    /// Attach thinking content to this message.
    pub fn with_thinking(mut self, thinking: String) -> Self {
        self.thinking = Some(thinking);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::system("You are helpful");
        assert_eq!(msg.role, MessageRole::System);
        assert!(msg.content.is_some());
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, MessageRole::User);
    }

    #[test]
    fn test_message_content_from_str() {
        let content: MessageContent = "test".into();
        assert_eq!(content.as_str(), "test");
    }

    #[test]
    fn test_display_text_plain() {
        let content = MessageContent::Text("hello".to_string());
        assert_eq!(content.display_text(), "hello");
    }

    #[test]
    fn test_display_text_multipart_marks_images() {
        let content = MessageContent::MultiPart(vec![
            ContentPart::Text {
                text: "before".to_string(),
            },
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png;base64,AAAA".to_string(),
                    detail: None,
                },
            },
            ContentPart::Text {
                text: "after".to_string(),
            },
        ]);
        assert_eq!(content.display_text(), "before\n[image]\nafter");
    }

    #[test]
    fn test_display_text_image_only() {
        let content = MessageContent::MultiPart(vec![ContentPart::Image {
            image_url: ImageUrl {
                url: "https://example.test/a.png".to_string(),
                detail: None,
            },
        }]);
        assert_eq!(content.display_text(), "[image]");
    }

    #[test]
    fn test_display_text_empty_multipart() {
        let content = MessageContent::MultiPart(vec![]);
        assert_eq!(content.display_text(), "");
    }

    #[test]
    fn test_as_str_multipart_returns_empty() {
        let content = MessageContent::MultiPart(vec![ContentPart::Text {
            text: "x".to_string(),
        }]);
        assert_eq!(content.as_str(), "");
    }

    #[test]
    fn test_message_content_from_string() {
        let content: MessageContent = String::from("hello").into();
        assert_eq!(content, MessageContent::Text("hello".to_string()));
    }

    #[test]
    fn test_message_assistant() {
        let msg = Message::assistant("Sure!");
        assert_eq!(msg.role, MessageRole::Assistant);
        assert_eq!(msg.content.as_ref().unwrap().as_str(), "Sure!");
        assert!(msg.tool_calls.is_none());
        assert!(msg.tool_call_id.is_none());
        assert!(msg.thinking.is_none());
    }

    #[test]
    fn test_message_assistant_with_tools() {
        let tools = vec![
            ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                arguments: r#"{"city":"Beijing"}"#.to_string(),
                r#type: "function".to_string(),
            },
            ToolCall {
                id: "call_2".to_string(),
                name: "get_time".to_string(),
                arguments: "{}".to_string(),
                r#type: "function".to_string(),
            },
        ];
        let msg = Message::assistant_with_tools("", tools.clone());
        assert_eq!(msg.role, MessageRole::Assistant);
        let calls = msg.tool_calls.as_ref().unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "get_weather");
        assert_eq!(calls[0].arguments, r#"{"city":"Beijing"}"#);
        assert_eq!(calls[1].name, "get_time");
        assert!(msg.tool_call_id.is_none());
    }

    #[test]
    fn test_message_tool() {
        let msg = Message::tool("temperature=20", "call_7".to_string());
        assert_eq!(msg.role, MessageRole::Tool);
        assert_eq!(msg.content.as_ref().unwrap().as_str(), "temperature=20");
        assert_eq!(msg.tool_call_id.as_deref(), Some("call_7"));
    }

    #[test]
    fn test_message_with_thinking() {
        let msg = Message::assistant("answer").with_thinking("reasoning".to_string());
        assert_eq!(msg.thinking.as_deref(), Some("reasoning"));
    }

    #[test]
    fn test_message_serde_roundtrip_full() {
        let msg = Message {
            role: MessageRole::Assistant,
            content: Some(MessageContent::Text("hello".to_string())),
            tool_calls: Some(vec![ToolCall {
                id: "call_1".to_string(),
                name: "get_weather".to_string(),
                arguments: "{}".to_string(),
                r#type: "function".to_string(),
            }]),
            tool_call_id: None,
            name: Some("assistant-1".to_string()),
            thinking: Some("hmm".to_string()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains(r#""role":"assistant""#));
        assert!(json.contains(r#""name":"assistant-1""#));
        assert!(json.contains(r#""thinking":"hmm""#));
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.role, MessageRole::Assistant);
        assert_eq!(parsed.tool_calls.as_ref().unwrap()[0].name, "get_weather");
        assert_eq!(parsed.name.as_deref(), Some("assistant-1"));
        assert_eq!(parsed.thinking.as_deref(), Some("hmm"));
    }

    #[test]
    fn test_message_serde_omits_none_fields() {
        let msg = Message::system("sys");
        let json = serde_json::to_string(&msg).unwrap();
        assert!(!json.contains("tool_calls"));
        assert!(!json.contains("tool_call_id"));
        assert!(!json.contains("name"));
        assert!(!json.contains("thinking"));
        let parsed: Message = serde_json::from_str(&json).unwrap();
        assert!(parsed.tool_calls.is_none());
        assert!(parsed.name.is_none());
    }

    #[test]
    fn test_message_role_serde_roundtrip() {
        for role in [
            MessageRole::System,
            MessageRole::User,
            MessageRole::Assistant,
            MessageRole::Tool,
        ] {
            let json = serde_json::to_string(&role).unwrap();
            let parsed: MessageRole = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, role);
        }
        assert_eq!(
            serde_json::to_string(&MessageRole::User).unwrap(),
            r#""user""#
        );
    }

    #[test]
    fn test_content_part_serde_roundtrip() {
        let text = ContentPart::Text {
            text: "hi".to_string(),
        };
        let json = serde_json::to_string(&text).unwrap();
        assert!(json.contains(r#""type":"text""#));
        let parsed: ContentPart = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, text);

        let image = ContentPart::Image {
            image_url: ImageUrl {
                url: "https://example.test/i.png".to_string(),
                detail: Some("high".to_string()),
            },
        };
        let json = serde_json::to_string(&image).unwrap();
        assert!(json.contains(r#""type":"image""#));
        assert!(json.contains(r#""detail":"high""#));
        let parsed: ContentPart = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, image);
    }

    #[test]
    fn test_image_url_detail_omitted_when_none() {
        let url = ImageUrl {
            url: "data:image/png;base64,AAAA".to_string(),
            detail: None,
        };
        let json = serde_json::to_string(&url).unwrap();
        assert!(!json.contains("detail"));
        let parsed: ImageUrl = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.detail, None);
    }

    #[test]
    fn test_message_content_serde_roundtrip() {
        let text: MessageContent = MessageContent::Text("t".to_string());
        let json = serde_json::to_string(&text).unwrap();
        assert_eq!(json, r#""t""#);
        let parsed: MessageContent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, text);

        let multi = MessageContent::MultiPart(vec![ContentPart::Text {
            text: "a".to_string(),
        }]);
        let json = serde_json::to_string(&multi).unwrap();
        let parsed: MessageContent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, multi);
    }
}
