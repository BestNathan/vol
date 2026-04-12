//! File-based message store using JSONL format.

use async_trait::async_trait;
use crate::{SessionMessage};
use crate::store::{MessageStore, MessageStoreError};

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
    async fn store(&self, _message: SessionMessage) -> Result<(), MessageStoreError> {
        // TODO: Implement JSONL file writing
        Ok(())
    }

    async fn get_messages(&self, _session_id: &str) -> Result<Vec<SessionMessage>, MessageStoreError> {
        // TODO: Implement JSONL file reading
        Ok(Vec::new())
    }
}
