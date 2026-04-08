//! Session and Message store traits.
//!
//! This file will be fully implemented once Session struct is created.
//! For now, only MessageStore trait is defined.

use async_trait::async_trait;
use vol_llm_core::Result;
use super::message::SessionMessage;

/// Message storage interface
#[async_trait]
pub trait MessageStore: Send + Sync {
    /// Save a message
    async fn save(&self, message: SessionMessage) -> Result<()>;

    /// Get messages by session ID
    async fn get_by_session(&self, session_id: &str, limit: usize) -> Result<Vec<SessionMessage>>;

    /// Get messages before a timestamp (pagination)
    async fn get_before(&self, session_id: &str, before: i64, limit: usize) -> Result<Vec<SessionMessage>>;

    /// Delete all messages for a session
    async fn delete_session(&self, session_id: &str) -> Result<()>;

    /// Update a message
    async fn update(&self, id: &str, message: SessionMessage) -> Result<()>;

    /// Get message count for a session
    async fn get_count(&self, session_id: &str) -> Result<usize>;

    /// Cleanup expired messages
    async fn cleanup_expired(&self, before: i64) -> Result<()>;
}

// SessionStore trait will be added once Session struct is implemented
