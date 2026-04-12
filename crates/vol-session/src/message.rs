//! Session message wrapper.
//!
//! Wraps `vol_llm_core::Message` with session-related fields.

use std::collections::HashMap;
use vol_llm_core::Message;

/// Session message wrapper
///
/// Wraps `vol_llm_core::Message` with session-related fields.
#[derive(Clone, Debug)]
pub struct SessionMessage {
    /// Message unique ID (UUID)
    pub id: String,

    /// Session ID this message belongs to
    pub session_id: String,

    /// Core message body
    pub message: Message,

    /// Parent message ID, supports tree conversation structure
    /// None means root message (conversation start)
    pub parent_id: Option<String>,

    /// Creation timestamp (Unix seconds)
    pub created_at: i64,

    /// Metadata for extensible purposes
    /// e.g., user_id, tags, etc.
    pub metadata: HashMap<String, String>,
}

impl SessionMessage {
    /// Create a new session message
    pub fn new(session_id: String, message: Message) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            message,
            parent_id: None,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
        }
    }

    /// Set parent message ID
    pub fn with_parent_id(mut self, parent_id: String) -> Self {
        self.parent_id = Some(parent_id);
        self
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_message_creation() {
        let msg = SessionMessage::new("session-123".to_string(), Message::user("Hello"));

        assert_eq!(msg.session_id, "session-123");
        assert!(msg.parent_id.is_none());
        assert!(!msg.id.is_empty());
    }

    #[test]
    fn test_session_message_with_parent() {
        let msg = SessionMessage::new("session-123".to_string(), Message::user("Reply"))
            .with_parent_id("msg-456".to_string());

        assert_eq!(msg.parent_id, Some("msg-456".to_string()));
    }

    #[test]
    fn test_session_message_metadata() {
        let msg = SessionMessage::new("session-123".to_string(), Message::user("Test"))
            .with_metadata("user_id", "user-1");

        assert_eq!(msg.metadata.get("user_id"), Some(&"user-1".to_string()));
    }
}
