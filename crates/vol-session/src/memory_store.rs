//! In-memory session and message store implementations.

use std::collections::HashMap;
use tokio::sync::RwLock;
use crate::store::Result;
use crate::session::Session;
use crate::message::SessionMessage;
use crate::store::MessageStore;

/// In-memory session storage
pub struct InMemorySessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemorySessionStore {
    /// Create a new empty session store
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl crate::store::SessionStore for InMemorySessionStore {
    async fn create(&self, session: Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
        Ok(())
    }

    async fn get(&self, session_id: &str) -> Result<Option<Session>> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned())
    }

    async fn delete(&self, session_id: &str) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }

    async fn update(&self, session: Session) -> Result<()> {
        let mut sessions = self.sessions.write().await;
        sessions.insert(session.id.clone(), session);
        Ok(())
    }
}

/// In-memory message storage
pub struct InMemoryMessageStore {
    messages: RwLock<HashMap<String, Vec<SessionMessage>>>,
}

impl Default for InMemoryMessageStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryMessageStore {
    /// Create a new empty message store
    pub fn new() -> Self {
        Self {
            messages: RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl MessageStore for InMemoryMessageStore {
    async fn save(&self, message: SessionMessage) -> Result<()> {
        let mut messages = self.messages.write().await;
        messages
            .entry(message.session_id.clone())
            .or_insert_with(Vec::new)
            .push(message);
        Ok(())
    }

    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>> {
        let messages = self.messages.read().await;
        Ok(messages
            .get(session_id)
            .map(|msgs| {
                let mut sorted = msgs.clone();
                sorted.sort_by_key(|m| m.created_at);
                sorted.into_iter().take(limit).collect()
            })
            .unwrap_or_default())
    }

    async fn get_before(&self, session_id: &str, before: i64, limit: usize) -> Result<Vec<SessionMessage>> {
        let messages = self.messages.read().await;
        Ok(messages
            .get(session_id)
            .map(|msgs| {
                let mut filtered: Vec<_> = msgs
                    .iter()
                    .filter(|m| m.created_at < before)
                    .cloned()
                    .collect();
                filtered.sort_by_key(|m| m.created_at);
                filtered.into_iter().take(limit).collect()
            })
            .unwrap_or_default())
    }

    async fn delete_session(&self, session_id: &str) -> Result<()> {
        let mut messages = self.messages.write().await;
        messages.remove(session_id);
        Ok(())
    }

    async fn update(&self, id: &str, message: SessionMessage) -> Result<()> {
        let mut messages = self.messages.write().await;
        if let Some(msgs) = messages.get_mut(&message.session_id) {
            if let Some(pos) = msgs.iter().position(|m| m.id == id) {
                msgs[pos] = message;
            }
        }
        Ok(())
    }

    async fn get_count(&self, session_id: &str) -> Result<usize> {
        let messages = self.messages.read().await;
        Ok(messages.get(session_id).map(|msgs| msgs.len()).unwrap_or(0))
    }

    async fn cleanup_expired(&self, before: i64) -> Result<()> {
        let mut messages = self.messages.write().await;
        for msgs in messages.values_mut() {
            msgs.retain(|m| m.created_at >= before);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;
    use std::sync::Arc;
    use crate::store::SessionStore;

    #[tokio::test]
    async fn test_memory_message_store_save_and_get() {
        let store = InMemoryMessageStore::new();
        let msg = SessionMessage::new(
            "session-1".to_string(),
            Message::user("Hello"),
        );

        store.save(msg.clone()).await.unwrap();

        let retrieved = store.get_by_session("session-1", 10).await.unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].session_id, "session-1");
    }

    #[tokio::test]
    async fn test_memory_message_store_get_before() {
        let store = InMemoryMessageStore::new();
        let mut msg1 = SessionMessage::new(
            "session-1".to_string(),
            Message::user("First"),
        );
        msg1.created_at = 100;

        let mut msg2 = SessionMessage::new(
            "session-1".to_string(),
            Message::user("Second"),
        );
        msg2.created_at = 200;

        store.save(msg1).await.unwrap();
        store.save(msg2).await.unwrap();

        let retrieved = store.get_before("session-1", 150, 10).await.unwrap();
        assert_eq!(retrieved.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_message_store_count() {
        let store = InMemoryMessageStore::new();

        for i in 0..5 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("Message {}", i)),
            );
            store.save(msg).await.unwrap();
        }

        let count = store.get_count("session-1").await.unwrap();
        assert_eq!(count, 5);
    }

    #[tokio::test]
    async fn test_memory_session_store_crud() {
        let store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());
        let session = Session::new(
            "session-1".to_string(),
            store.clone(),
            message_store,
        );

        store.create(session.clone()).await.unwrap();

        let retrieved = store.get("session-1").await.unwrap();
        assert!(retrieved.is_some());

        store.delete("session-1").await.unwrap();
        let deleted = store.get("session-1").await.unwrap();
        assert!(deleted.is_none());
    }
}
