//! Role-based filtering compressor.
//!
//! Keeps messages from selected roles only (e.g., User + final Assistant),
//! filtering out intermediate tool calls.

use crate::compressor::MessageCompressor;
use crate::message::SessionMessage;
use vol_llm_core::message::MessageRole;

/// Compressor that filters messages by role.
pub struct RoleFilterCompressor {
    /// Roles to keep. If empty, defaults to [User, Assistant].
    keep_roles: Vec<MessageRole>,
}

impl RoleFilterCompressor {
    pub fn new(keep_roles: Vec<MessageRole>) -> Self {
        Self { keep_roles }
    }
}

impl Default for RoleFilterCompressor {
    fn default() -> Self {
        Self {
            keep_roles: vec![MessageRole::User, MessageRole::Assistant],
        }
    }
}

#[async_trait::async_trait]
impl MessageCompressor for RoleFilterCompressor {
    async fn compress(&self, messages: Vec<SessionMessage>) -> Vec<SessionMessage> {
        if messages.is_empty() {
            return vec![];
        }

        let default_roles = [MessageRole::User, MessageRole::Assistant];
        let keep: &[MessageRole] = if self.keep_roles.is_empty() {
            &default_roles
        } else {
            &self.keep_roles
        };

        messages
            .into_iter()
            .filter(|m| keep.contains(&m.message.role))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    fn make_msg_with_role(role: MessageRole) -> SessionMessage {
        let mut msg = SessionMessage::new("test".to_string(), Message::user("msg"));
        msg.message.role = role;
        msg
    }

    #[tokio::test]
    async fn test_default_keeps_user_and_assistant() {
        let compressor = RoleFilterCompressor::default();
        let messages = vec![
            make_msg_with_role(MessageRole::User),
            make_msg_with_role(MessageRole::Tool),
            make_msg_with_role(MessageRole::Assistant),
            make_msg_with_role(MessageRole::Tool),
        ];
        let result = compressor.compress(messages).await;
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].message.role, MessageRole::User);
        assert_eq!(result[1].message.role, MessageRole::Assistant);
    }

    #[tokio::test]
    async fn test_empty_input() {
        let compressor = RoleFilterCompressor::default();
        let result = compressor.compress(vec![]).await;
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_custom_roles() {
        let compressor = RoleFilterCompressor::new(vec![MessageRole::System]);
        let messages = vec![
            make_msg_with_role(MessageRole::User),
            make_msg_with_role(MessageRole::System),
        ];
        let result = compressor.compress(messages).await;
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].message.role, MessageRole::System);
    }
}
