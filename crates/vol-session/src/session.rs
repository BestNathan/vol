//! Session management with entry-based persistence.

use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::entry::{CheckpointReason, SessionEntry, SessionEntryData, SessionEntryType};
use crate::message::SessionMessage;
use crate::store::{Result, SessionEntryStore};
use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::Message;

/// Session management
pub struct Session {
    pub id: String,
    pub created_at: i64,
    entry_store: Arc<dyn SessionEntryStore>,
    compressor: Arc<dyn MessageCompressor>,
}

impl Session {
    /// Create a new session — self-generates UUID, current timestamp.
    pub fn new(entry_store: Arc<dyn SessionEntryStore>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            entry_store,
            compressor: Arc::new(PositionSampleCompressor::default()),
        }
    }

    /// Resume from existing session — external ID provided.
    /// Loads created_at from the first entry if available.
    pub async fn resume(id: String, entry_store: Arc<dyn SessionEntryStore>) -> Result<Self> {
        let entries = entry_store.get_entries(&id).await?;
        let created_at = entries.first().map(|e| e.created_at).unwrap_or(0);

        Ok(Self {
            id,
            created_at,
            entry_store,
            compressor: Arc::new(PositionSampleCompressor::default()),
        })
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

    /// Get all messages after the latest checkpoint.
    /// If no checkpoint exists, returns all messages.
    /// Summary entries are converted to synthetic SessionMessage with system role.
    pub async fn get_messages(&self) -> Result<Vec<SessionMessage>> {
        let entries = match self.entry_store.find_latest_checkpoint(&self.id).await? {
            Some(cp) => {
                // Get entries strictly after the checkpoint
                let all = self.entry_store.get_entries(&self.id).await?;
                all.into_iter().filter(|e| e.created_at > cp.created_at).collect()
            }
            None => self.entry_store.get_entries(&self.id).await?,
        };

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

    /// Get resume messages as raw Messages (after latest checkpoint).
    /// Used for repopulating context on session resume.
    pub async fn resume_messages(&self) -> Result<Vec<Message>> {
        let entries = match self.entry_store.find_latest_checkpoint(&self.id).await? {
            Some(cp) => {
                let all = self.entry_store.get_entries(&self.id).await?;
                all.into_iter().filter(|e| e.created_at > cp.created_at).collect()
            }
            None => self.entry_store.get_entries(&self.id).await?,
        };

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
                    // Checkpoints are not included
                }
            }
        }

        Ok(messages)
    }

    /// Compress the given messages and write checkpoint + summary + compressed entries.
    pub async fn compress(&mut self, messages: Vec<SessionMessage>) {
        if messages.is_empty() {
            return;
        }

        // 1. Write checkpoint first (seal old messages) — use explicit timestamp
        let checkpoint_ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let mut cp_entry = SessionEntry::new_checkpoint(self.id.clone(), CheckpointReason::Compression, None);
        cp_entry.created_at = checkpoint_ts;
        if let Err(e) = self.entry_store.save(cp_entry).await {
            tracing::error!("Failed to write checkpoint before compression: {}", e);
            return;
        }

        // 2. Compress input messages
        let compressed = self.compressor.compress(messages).await;
        if compressed.is_empty() {
            return;
        }

        // 3. Build summary text from compressed messages
        let summary = compressed
            .iter()
            .filter_map(|m| m.message.content.as_ref())
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // 4. Write summary entry (timestamp after checkpoint)
        let mut summary_entry = SessionEntry::new_summary(self.id.clone(), summary);
        summary_entry.created_at = checkpoint_ts + 1;
        if let Err(e) = self.entry_store.save(summary_entry).await {
            tracing::error!("Failed to write summary during compression: {}", e);
            return;
        }

        // 5. Write compressed message entries (timestamp after checkpoint)
        for (i, msg) in compressed.iter().enumerate() {
            let mut entry = SessionEntry {
                id: msg.id.clone(),
                session_id: self.id.clone(),
                created_at: msg.created_at.max(checkpoint_ts + 1),
                parent_id: msg.parent_id.clone(),
                r#type: SessionEntryType::Message,
                data: SessionEntryData::Message {
                    message: msg.message.clone(),
                },
            };
            entry.created_at = checkpoint_ts + 1 + (i as i64);
            if let Err(e) = self.entry_store.save(entry).await {
                tracing::error!("Failed to write compressed message: {}", e);
            }
        }
    }

    /// Add metadata (no-op, kept for backward compatibility during transition).
    pub fn with_metadata(self, _key: &str, _value: &str) -> Self {
        self
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at: self.created_at,
            entry_store: self.entry_store.clone(),
            compressor: self.compressor.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_store::InMemoryEntryStore;

    #[tokio::test]
    async fn test_session_new_self_generates_id() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);

        assert!(!session.id.is_empty());
        assert!(session.created_at > 0);
    }

    #[tokio::test]
    async fn test_session_get_messages() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let session = Session::new(entry_store.clone());

        let msg = SessionMessage::new(session.id.clone(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_with_metadata_noop() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store).with_metadata("user_id", "user-123");

        // with_metadata is a no-op now, session should still work
        let messages = session.get_messages().await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_compress_flow() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store.clone());

        // Add 10 messages
        for i in 0..10 {
            let msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        // Before compression: no checkpoint, get all 10
        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 10);

        // Compress
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After compression:
        // 1 checkpoint (not returned as message) + 1 summary + 6 compressed = 7 messages returned
        let after = session.get_messages().await.unwrap();
        assert_eq!(after.len(), 7);

        // First should be the summary as a system message
        assert_eq!(after[0].message.role, vol_llm_core::MessageRole::System);
    }

    #[tokio::test]
    async fn test_session_compress_empty_messages() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store);

        // Compress with empty input should be no-op
        session.compress(vec![]).await;
        let messages = session.get_messages().await.unwrap();
        assert!(messages.is_empty());
    }

    #[tokio::test]
    async fn test_session_resume_messages_includes_summary() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store.clone());

        for i in 0..5 {
            let msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("msg-{}", i)),
            );
            session.add_message(msg).await.unwrap();
        }

        let messages = session.get_messages().await.unwrap();
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // get_messages after compress: returns messages after checkpoint (summary + compressed)
        let msgs = session.get_messages().await.unwrap();
        assert!(!msgs.is_empty());
        // First is summary as system message
        assert_eq!(msgs[0].message.role, vol_llm_core::MessageRole::System);
    }

    #[tokio::test]
    async fn test_session_resume_constructor() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        // Create and populate a session
        let session = Session::new(entry_store.clone());
        let session_id = session.id.clone();

        let msg = SessionMessage::new(session_id.clone(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        // Resume from the same entry_store
        let resumed = Session::resume(session_id.clone(), entry_store.clone()).await.unwrap();
        assert_eq!(resumed.id, session_id);

        // get_messages should return the messages after checkpoint (all, since no checkpoint)
        let messages = resumed.get_messages().await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_multiple_compressions() {
        let entry_store = Arc::new(InMemoryEntryStore::new());

        let mut session = Session::new(entry_store.clone());

        // First batch: 6 messages with explicit timestamps
        for i in 0..6 {
            let mut msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("batch1-{}", i)),
            );
            msg.created_at = 100 + i;
            session.add_message(msg).await.unwrap();
        }
        let messages = session.get_messages().await.unwrap();
        assert_eq!(messages.len(), 6);
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After first compress: summary + 4 compressed (keep_first=2, sample_every=3 on 6 msgs) = 5 messages
        let after1 = session.get_messages().await.unwrap();
        assert_eq!(after1.len(), 5);
        assert_eq!(after1[0].message.role, vol_llm_core::MessageRole::System);

        // Get the checkpoint timestamp to calculate new message timestamps after it
        let checkpoint_ts = entry_store
            .find_latest_checkpoint(&session.id)
            .await
            .unwrap()
            .unwrap()
            .created_at;

        // Add 3 more messages with timestamps after the compression entries
        for i in 0..3 {
            let mut msg = SessionMessage::new(
                session.id.clone(),
                Message::user(format!("batch2-{}", i)),
            );
            msg.created_at = checkpoint_ts + 10 + i;
            session.add_message(msg).await.unwrap();
        }

        // Now: summary + 4 compressed + 3 new = 8 messages
        let messages2 = session.get_messages().await.unwrap();
        assert_eq!(messages2.len(), 8);

        // Compress again
        session.compress(messages2).await;

        // After second compress: new checkpoint + summary + compressed
        let after2 = session.get_messages().await.unwrap();
        // The previous summary+compressed (4) + new messages (3) = 7 total
        // Compressor keeps 3 → summary + 3 compressed
        assert!(!after2.is_empty());
    }
}
