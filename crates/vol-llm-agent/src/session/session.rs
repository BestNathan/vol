//! Session management.

use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Result;
use super::message::SessionMessage;
use super::store::{SessionStore, MessageStore};

/// Session management
///
/// Encapsulates session metadata and storage operations.
pub struct Session {
    /// Session unique ID
    pub id: String,

    /// Creation timestamp (Unix seconds)
    pub created_at: i64,

    /// Session metadata
    /// e.g., user_id, title, etc.
    pub metadata: HashMap<String, String>,

    /// Session storage
    session_store: Arc<dyn SessionStore>,

    /// Message storage
    message_store: Arc<dyn MessageStore>,
}

impl Session {
    /// Create a new session
    pub fn new(
        id: String,
        session_store: Arc<dyn SessionStore>,
        message_store: Arc<dyn MessageStore>,
    ) -> Self {
        Self {
            id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
            session_store,
            message_store,
        }
    }

    /// Get historical messages
    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>> {
        self.message_store.get_by_session(&self.id, limit).await
    }

    /// Add a message
    pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
        self.message_store.save(message).await
    }

    /// Get or create session from parent ID (supports branching)
    pub async fn get_or_create_parent(&self, parent_id: &str) -> Option<Session> {
        self.session_store.get(parent_id).await.ok().flatten()
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at: self.created_at,
            metadata: self.metadata.clone(),
            session_store: self.session_store.clone(),
            message_store: self.message_store.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;
    use crate::session::memory_store::{InMemorySessionStore, InMemoryMessageStore};

    #[tokio::test]
    async fn test_session_get_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        );

        let msg = SessionMessage::new(
            "session-1".to_string(),
            Message::user("Hello"),
        );
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages(10).await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_with_metadata() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        ).with_metadata("user_id", "user-123");

        assert_eq!(session.metadata.get("user_id"), Some(&"user-123".to_string()));
    }
}
