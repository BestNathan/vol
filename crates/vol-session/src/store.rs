//! Session and message store traits.

use async_trait::async_trait;
use crate::{Session, SessionMessage};

/// Error type for session store operations.
#[derive(Debug, thiserror::Error)]
pub enum SessionStoreError {
    #[error("Session not found: {0}")]
    NotFound(String),
    #[error("Store error: {0}")]
    StoreError(String),
}

/// Error type for message store operations.
#[derive(Debug, thiserror::Error)]
pub enum MessageStoreError {
    #[error("Message not found: {0}")]
    NotFound(String),
    #[error("Store error: {0}")]
    StoreError(String),
}

/// Trait for session storage.
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Create a new session.
    async fn create(&self, session: Session) -> Result<(), SessionStoreError>;

    /// Get a session by ID.
    async fn get(&self, id: &str) -> Result<Session, SessionStoreError>;

    /// Delete a session.
    async fn delete(&self, id: &str) -> Result<(), SessionStoreError>;
}

/// Trait for message storage.
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// Store a message.
    async fn store(&self, message: SessionMessage) -> Result<(), MessageStoreError>;

    /// Retrieve messages for a session.
    async fn get_messages(&self, session_id: &str) -> Result<Vec<SessionMessage>, MessageStoreError>;
}
