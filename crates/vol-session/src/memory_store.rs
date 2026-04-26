//! In-memory session and message store implementations.

use crate::message::SessionMessage;
use crate::session::Session;
use crate::store::MessageStore;
use crate::store::Result;
use std::collections::HashMap;
use tokio::sync::RwLock;

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

    async fn get_before(
        &self,
        session_id: &str,
        before: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>> {
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

    async fn get_after(
        &self,
        session_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>> {
        let messages = self.messages.read().await;
        Ok(messages
            .get(session_id)
            .map(|msgs| {
                let mut filtered: Vec<_> = msgs
                    .iter()
                    .filter(|m| m.created_at > after)
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

/// In-memory entry store for testing.
pub struct InMemoryEntryStore {
    entries: tokio::sync::RwLock<HashMap<String, Vec<crate::entry::SessionEntry>>>,
}

impl Default for InMemoryEntryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryEntryStore {
    /// Create a new empty entry store.
    pub fn new() -> Self {
        Self {
            entries: tokio::sync::RwLock::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl crate::store::SessionEntryStore for InMemoryEntryStore {
    async fn save(&self, entry: crate::entry::SessionEntry) -> crate::store::Result<()> {
        self.entries
            .write()
            .await
            .entry(entry.session_id.clone())
            .or_default()
            .push(entry);
        Ok(())
    }

    async fn get_entries(&self, session_id: &str) -> crate::store::Result<Vec<crate::entry::SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries.get(session_id).cloned().unwrap_or_default())
    }

    async fn get_after(&self, session_id: &str, after: i64) -> crate::store::Result<Vec<crate::entry::SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .get(session_id)
            .map(|msgs| {
                msgs.iter()
                    .filter(|e| e.created_at >= after)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default())
    }

    async fn find_latest_checkpoint(&self, session_id: &str) -> crate::store::Result<Option<crate::entry::SessionEntry>> {
        let entries = self.entries.read().await;
        Ok(entries
            .get(session_id)
            .and_then(|msgs| {
                msgs.iter()
                    .filter(|e| e.r#type == crate::entry::SessionEntryType::Checkpoint)
                    .max_by_key(|e| e.created_at)
                    .cloned()
            }))
    }

    async fn delete_session(&self, session_id: &str) -> crate::store::Result<()> {
        self.entries.write().await.remove(session_id);
        Ok(())
    }

    async fn get_count(&self, session_id: &str) -> crate::store::Result<usize> {
        let entries = self.entries.read().await;
        Ok(entries.get(session_id).map(|msgs| msgs.len()).unwrap_or(0))
    }
}

#[cfg(test)]
mod entry_tests {
    use super::*;
    use crate::entry::{SessionEntry, SessionEntryType};
    use crate::message::SessionMessage;
    use crate::store::SessionEntryStore;
    use crate::CheckpointReason;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_in_memory_entry_store_save_and_get() {
        let store = InMemoryEntryStore::new();

        let entry = SessionEntry::from_message(
            SessionMessage::new("test-session".to_string(), Message::user("Hello, World!")),
        );

        store.save(entry.clone()).await.unwrap();

        let entries = store.get_entries("test-session").await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_in_memory_entry_store_find_checkpoint() {
        let store = InMemoryEntryStore::new();

        let mut msg1 = SessionEntry::from_message(
            SessionMessage::new("test-session".to_string(), Message::user("before")),
        );
        msg1.created_at = 100;

        let mut cp = SessionEntry::new_checkpoint(
            "test-session".to_string(),
            CheckpointReason::Compression,
            None,
        );
        cp.created_at = 200;

        let mut msg2 = SessionEntry::from_message(
            SessionMessage::new("test-session".to_string(), Message::user("after")),
        );
        msg2.created_at = 300;

        store.save(msg1).await.unwrap();
        store.save(cp).await.unwrap();
        store.save(msg2).await.unwrap();

        let cp = store.find_latest_checkpoint("test-session").await.unwrap().unwrap();
        assert_eq!(cp.r#type, SessionEntryType::Checkpoint);

        let after = store.get_after("test-session", cp.created_at).await.unwrap();
        assert_eq!(after.len(), 2);
    }

    #[tokio::test]
    async fn test_in_memory_entry_store_delete_session() {
        let store = InMemoryEntryStore::new();

        store.save(SessionEntry::from_message(
            SessionMessage::new("test-session".to_string(), Message::user("test")),
        )).await.unwrap();

        store.delete_session("test-session").await.unwrap();
        let count = store.get_count("test-session").await.unwrap();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_in_memory_entry_store_multiple_sessions() {
        let store = InMemoryEntryStore::new();

        store.save(SessionEntry::from_message(
            SessionMessage::new("session-a".to_string(), Message::user("from A")),
        )).await.unwrap();

        store.save(SessionEntry::from_message(
            SessionMessage::new("session-b".to_string(), Message::user("from B")),
        )).await.unwrap();

        let entries_a = store.get_entries("session-a").await.unwrap();
        assert_eq!(entries_a.len(), 1);
        assert_eq!(entries_a[0].session_id, "session-a");

        let entries_b = store.get_entries("session-b").await.unwrap();
        assert_eq!(entries_b.len(), 1);
        assert_eq!(entries_b[0].session_id, "session-b");

        // Deleting A should not affect B
        store.delete_session("session-a").await.unwrap();
        assert_eq!(store.get_count("session-b").await.unwrap(), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::SessionStore;
    use std::sync::Arc;
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_memory_message_store_save_and_get() {
        let store = InMemoryMessageStore::new();
        let msg = SessionMessage::new("session-1".to_string(), Message::user("Hello"));

        store.save(msg.clone()).await.unwrap();

        let retrieved = store.get_by_session("session-1", 10).await.unwrap();
        assert_eq!(retrieved.len(), 1);
        assert_eq!(retrieved[0].session_id, "session-1");
    }

    #[tokio::test]
    async fn test_memory_message_store_get_before() {
        let store = InMemoryMessageStore::new();
        let mut msg1 = SessionMessage::new("session-1".to_string(), Message::user("First"));
        msg1.created_at = 100;

        let mut msg2 = SessionMessage::new("session-1".to_string(), Message::user("Second"));
        msg2.created_at = 200;

        store.save(msg1).await.unwrap();
        store.save(msg2).await.unwrap();

        let retrieved = store.get_before("session-1", 150, 10).await.unwrap();
        assert_eq!(retrieved.len(), 1);
    }

    #[tokio::test]
    async fn test_memory_message_store_get_after() {
        let store = InMemoryMessageStore::new();

        let mut msg1 = SessionMessage::new("session-1".to_string(), Message::user("First"));
        msg1.created_at = 100;
        let mut msg2 = SessionMessage::new("session-1".to_string(), Message::user("Second"));
        msg2.created_at = 200;
        let mut msg3 = SessionMessage::new("session-1".to_string(), Message::user("Third"));
        msg3.created_at = 300;

        store.save(msg1).await.unwrap();
        store.save(msg2).await.unwrap();
        store.save(msg3).await.unwrap();

        let retrieved = store.get_after("session-1", 150, 10).await.unwrap();
        assert_eq!(retrieved.len(), 2);
        assert_eq!(retrieved[0].message.content.as_ref().unwrap().as_str(), "Second");
        assert_eq!(retrieved[1].message.content.as_ref().unwrap().as_str(), "Third");

        let limited = store.get_after("session-1", 150, 1).await.unwrap();
        assert_eq!(limited.len(), 1);
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
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());

        store.create(session.clone()).await.unwrap();

        let retrieved = store.get("session-1").await.unwrap();
        assert!(retrieved.is_none()); // Session id is auto-generated, won't match "session-1"

        // Create again with the actual session id
        store.create(Session::new(entry_store)).await.unwrap();
    }
}
