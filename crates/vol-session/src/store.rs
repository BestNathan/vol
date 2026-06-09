//! Session and Entry store traits.

use crate::entry::SessionEntry;
use crate::message::SessionMessage;
use crate::session::Session;
use async_trait::async_trait;
use thiserror::Error;

/// Store operation error
#[derive(Debug, Error)]
pub enum StoreError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Session agent scope conflict for {session_id}: expected {expected}, actual {actual}")]
    SessionAgentScopeConflict {
        session_id: String,
        expected: String,
        actual: String,
    },
}

pub type Result<T> = std::result::Result<T, StoreError>;

/// Session storage interface
#[async_trait]
pub trait SessionStore: Send + Sync {
    /// Create a session
    async fn create(&self, session: Session) -> Result<()>;

    /// Get a session by ID
    async fn get(&self, session_id: &str) -> Result<Option<Session>>;

    /// Delete a session
    async fn delete(&self, session_id: &str) -> Result<()>;

    /// Update a session
    async fn update(&self, session: Session) -> Result<()>;
}

/// Entry storage interface — supports Message, Checkpoint, and Summary entry types.
/// Multi-session: methods accept session_id to scope operations.
#[async_trait]
pub trait SessionEntryStore: Send + Sync {
    /// Append an entry (entry already carries session_id).
    async fn save(&self, entry: SessionEntry) -> Result<()>;

    /// Get all entries for a session.
    async fn get_entries(&self, session_id: &str) -> Result<Vec<SessionEntry>>;

    /// Get entries after a timestamp for a session.
    async fn get_after(&self, session_id: &str, after: i64) -> Result<Vec<SessionEntry>>;

    /// Find the latest checkpoint entry for a session.
    async fn find_latest_checkpoint(&self, session_id: &str) -> Result<Option<SessionEntry>>;

    /// Delete all entries for a session.
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Get entry count for a session.
    async fn get_count(&self, session_id: &str) -> Result<usize>;
}

/// Legacy MessageStore trait — kept for backward compatibility.
/// New code should use SessionEntryStore instead.
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// Save a message
    async fn save(&self, message: SessionMessage) -> Result<()>;

    /// Get messages by session ID
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>>;

    /// Get messages before a timestamp (pagination)
    async fn get_before(
        &self,
        session_id: &str,
        before: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>>;

    /// Get messages after a timestamp (for compressed history)
    async fn get_after(
        &self,
        session_id: &str,
        after: i64,
        limit: usize,
    ) -> Result<Vec<SessionMessage>>;

    /// Delete all messages for a session
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Update a message
    async fn update(&self, id: &str, message: SessionMessage) -> Result<()>;

    /// Get message count for a session
    async fn get_count(&self, session_id: &str) -> Result<usize>;

    /// Cleanup expired messages
    async fn cleanup_expired(&self, before: i64) -> Result<()>;
}
