//! File-based message store using JSONL format.

use async_trait::async_trait;
use vol_llm_core::Result;
use crate::message::SessionMessage;
use crate::store::MessageStore;

/// File-based message store using JSONL format.
pub struct FileMessageStore {
    /// Path to the JSONL file.
    #[allow(dead_code)]
    path: String,
}

impl FileMessageStore {
    /// Create a new file message store.
    pub fn new(path: String) -> Self {
        Self { path }
    }
}

#[async_trait]
impl MessageStore for FileMessageStore {
    async fn save(&self, _message: SessionMessage) -> Result<()> {
        // TODO: Implement JSONL file writing
        Ok(())
    }

    async fn get_by_session(&self, _session_id: &str, _limit: usize) -> Result<Vec<SessionMessage>> {
        // TODO: Implement JSONL file reading
        Ok(Vec::new())
    }

    async fn get_before(&self, _session_id: &str, _before: i64, _limit: usize) -> Result<Vec<SessionMessage>> {
        Ok(Vec::new())
    }

    async fn delete_session(&self, _session_id: &str) -> Result<()> {
        Ok(())
    }

    async fn update(&self, _id: &str, _message: SessionMessage) -> Result<()> {
        Ok(())
    }

    async fn get_count(&self, _session_id: &str) -> Result<usize> {
        Ok(0)
    }

    async fn cleanup_expired(&self, _before: i64) -> Result<()> {
        Ok(())
    }
}
