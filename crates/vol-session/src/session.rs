//! Session management with entry-based persistence.

use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
use crate::message::SessionMessage;
use crate::store::{Result, SessionEntryStore, SessionStore};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Message;

/// Session management
pub struct Session {
    pub id: String,
    pub created_at: i64,
    pub metadata: HashMap<String, String>,
    session_store: Arc<dyn SessionStore>,
    entry_store: Arc<dyn SessionEntryStore>,
    compressor: Arc<dyn MessageCompressor>,
}

impl Session {
    /// Create a new session.
    pub fn new(
        id: String,
        session_store: Arc<dyn SessionStore>,
        entry_store: Arc<dyn SessionEntryStore>,
    ) -> Self {
        Self {
            id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
            session_store,
            entry_store,
            compressor: Arc::new(PositionSampleCompressor::default()),
        }
    }

    /// Set the compression strategy.
    pub fn with_compressor(mut self, compressor: Arc<dyn MessageCompressor>) -> Self {
        self.compressor = compressor;
        self
    }

    /// Add a message entry.
    pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
        let entry = SessionEntry {
            id: message.id.clone(),
            session_id: message.session_id.clone(),
            created_at: message.created_at,
            parent_id: message.parent_id.clone(),
            r#type: SessionEntryType::Message,
            data: SessionEntryData::Message {
                message: message.message,
            },
        };
        self.entry_store.save(entry).await
    }

    /// Write a checkpoint entry.
    pub async fn checkpoint(&self, reason: CheckpointReason, note: Option<String>) -> Result<()> {
        let entry = SessionEntry::new_checkpoint(self.id.clone(), reason, note);
        self.entry_store.save(entry).await
    }

    /// Write a summary entry (from compression).
    pub async fn add_summary(&self, summary: String) -> Result<()> {
        let entry = SessionEntry::new_summary(self.id.clone(), summary);
        self.entry_store.save(entry).await
    }

    /// Get messages — returns Message entries as SessionMessage list.
    /// Summary entries are converted to synthetic SessionMessage with system role.
    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>> {
        let entries = self.entry_store.get_entries(limit).await?;
        let mut messages = Vec::new();

        for entry in entries {
            match entry.data {
                SessionEntryData::Message { message } => {
                    messages.push(SessionMessage {
                        id: entry.id,
                        session_id: entry.session_id,
                        message,
                        parent_id: entry.parent_id,
                        created_at: entry.created_at,
                        metadata: HashMap::new(),
                    });
                }
                SessionEntryData::Summary { summary } => {
                    // Summary becomes a synthetic system message
                    messages.push(SessionMessage {
                        id: entry.id,
                        session_id: entry.session_id,
                        message: Message::system(summary),
                        parent_id: entry.parent_id,
                        created_at: entry.created_at,
                        metadata: HashMap::new(),
                    });
                }
                SessionEntryData::Checkpoint { .. } => {
                    // Checkpoints are not returned as messages
                }
            }
        }

        Ok(messages)
    }

    /// Get resume entries — all entries after the latest checkpoint.
    /// If no checkpoint exists, returns all entries.
    pub async fn resume_entries(&self) -> Result<Vec<SessionEntry>> {
        match self.entry_store.find_latest_checkpoint().await? {
            Some(cp) => self.entry_store.get_after(cp.created_at, usize::MAX).await,
            None => self.entry_store.get_entries(usize::MAX).await,
        }
    }

    /// Convert resume entries to Message Vec for context rebuilding.
    /// Summary entries become synthetic system messages.
    pub async fn resume_messages(&self) -> Result<Vec<Message>> {
        let entries = self.resume_entries().await?;
        let mut messages = Vec::new();

        for entry in entries {
            match entry.data {
                SessionEntryData::Message { message } => {
                    messages.push(message);
                }
                SessionEntryData::Summary { summary } => {
                    messages.push(Message::system(summary));
                }
                SessionEntryData::Checkpoint { .. } => {
                    // Checkpoints are not messages
                }
            }
        }

        Ok(messages)
    }

    /// Compress the given messages and write summary + checkpoint entries.
    pub async fn compress(&mut self, messages: Vec<SessionMessage>) {
        if messages.is_empty() {
            return;
        }

        // Compress to summary text
        let compressed = self.compressor.compress(messages).await;
        if compressed.is_empty() {
            return;
        }

        // Build summary text from compressed messages
        let summary = compressed
            .iter()
            .filter_map(|m| m.message.content.as_ref())
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Delete all entries, then re-save: summary first, then compressed messages, then checkpoint
        let _ = self.entry_store.delete_session().await;
        let _ = self.add_summary(summary).await;
        for msg in &compressed {
            let _ = self.add_message(msg.clone()).await;
        }
        let _ = self.checkpoint(CheckpointReason::Compression, None).await;
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
            entry_store: self.entry_store.clone(),
            compressor: self.compressor.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_store::{InMemoryEntryStore, InMemorySessionStore};

    #[tokio::test]
    async fn test_session_get_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        let msg = SessionMessage::new("session-1".to_string(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages(10).await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_with_metadata() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        )
        .with_metadata("user_id", "user-123");

        assert_eq!(
            session.metadata.get("user_id"),
            Some(&"user-123".to_string())
        );
    }

    #[tokio::test]
    async fn test_session_compress_and_get_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        // Add 10 messages
        for i in 0..10 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        // Before compression, get all 10
        let messages = session.get_messages(20).await.unwrap();
        assert_eq!(messages.len(), 10);

        // Compress
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After compression: should have compressed messages + summary
        let after = session.get_messages(20).await.unwrap();
        // 6 compressed messages + 1 summary message
        assert_eq!(after.len(), 7);
    }

    #[tokio::test]
    async fn test_session_compress_empty_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        // Compress with empty input should be no-op
        session.compress(vec![]).await;
        let messages = session.get_messages(20).await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_resume_entries_no_checkpoint() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        for i in 0..3 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        let entries = session.resume_entries().await.unwrap();
        // No checkpoint, so should return all entries
        assert_eq!(entries.len(), 3);
    }

    #[tokio::test]
    async fn test_session_resume_messages_includes_summary() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            entry_store.clone(),
        );

        for i in 0..5 {
            let msg = SessionMessage::new(
                "session-1".to_string(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        let messages = session.get_messages(20).await.unwrap();
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        let resume_msgs = session.resume_messages().await.unwrap();
        // Summary entries become synthetic system messages
        assert!(!resume_msgs.is_empty());
        // First should be the summary as a system message
        assert_eq!(resume_msgs[0].role, vol_llm_core::MessageRole::System);
    }
}
