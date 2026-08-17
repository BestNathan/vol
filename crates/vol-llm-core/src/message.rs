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
}
