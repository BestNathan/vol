//! In-memory session and message stores.

use std::collections::HashMap;
use tokio::sync::RwLock;
use async_trait::async_trait;
use crate::{Session, SessionMessage};
use crate::store::{SessionStore, SessionStoreError, MessageStore, MessageStoreError};

/// In-memory session store.
pub struct InMemorySessionStore {
    sessions: RwLock<HashMap<String, Session>>,
}

impl InMemorySessionStore {
    /// Create a new in-memory session store.
    pub fn new() -> Self {
        Self {
            sessions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemorySessionStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl SessionStore for InMemorySessionStore {
    async fn create(&self, session: Session) -> Result<(), SessionStoreError> {
        self.sessions.write().await.insert(session.id.clone(), session);
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Session, SessionStoreError> {
        self.sessions
            .read()
            .await
            .get(id)
            .cloned()
            .ok_or_else(|| SessionStoreError::NotFound(id.to_string()))
    }

    async fn delete(&self, id: &str) -> Result<(), SessionStoreError> {
        self.sessions.write().await.remove(id);
        Ok(())
    }
}

/// In-memory message store.
pub struct InMemoryMessageStore {
    messages: RwLock<HashMap<String, Vec<SessionMessage>>>,
}

impl InMemoryMessageStore {
    /// Create a new in-memory message store.
    pub fn new() -> Self {
        Self {
            messages: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryMessageStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MessageStore for InMemoryMessageStore {
    async fn store(&self, message: SessionMessage) -> Result<(), MessageStoreError> {
        let mut messages = self.messages.write().await;
        messages
            .entry(message.id.clone())
            .or_default()
            .push(message);
        Ok(())
    }

    async fn get_messages(&self, session_id: &str) -> Result<Vec<SessionMessage>, MessageStoreError> {
        Ok(self
            .messages
            .read()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default())
    }
}
