//! Session management.

use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::message::SessionMessage;
use crate::store::Result;
use crate::store::{MessageStore, SessionStore};
use std::collections::HashMap;
use std::sync::Arc;

/// Session management
///
/// Encapsulates session metadata and storage operations.
pub struct Session {
    /// Session unique ID
    pub id: String,

    /// Creation timestamp (Unix seconds)
    pub created_at: i64,

    /// Session metadata
    /// e.g., user_id, title, etc.
    pub metadata: HashMap<String, String>,

    /// Session storage
    session_store: Arc<dyn SessionStore>,

    /// Message storage
    message_store: Arc<dyn MessageStore>,

    /// Compressed "精华" messages from history.
    compressed_messages: Vec<SessionMessage>,

    /// Timestamp cursor — only fetch messages after this point after compression.
    compressed_after_ts: Option<i64>,

    /// Compression strategy.
    compressor: Arc<dyn MessageCompressor>,
}

impl Session {
    /// Create a new session
    pub fn new(
        id: String,
        session_store: Arc<dyn SessionStore>,
        message_store: Arc<dyn MessageStore>,
    ) -> Self {
        Self {
            id,
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs() as i64,
            metadata: HashMap::new(),
            session_store,
            message_store,
            compressed_messages: Vec::new(),
            compressed_after_ts: None,
            compressor: Arc::new(PositionSampleCompressor::default()),
        }
    }

    /// Add a message
    pub async fn add_message(&self, message: SessionMessage) -> Result<()> {
        self.message_store.save(message).await
    }

    /// Get or create session from parent ID (supports branching)
    pub async fn get_or_create_parent(&self, parent_id: &str) -> Option<Session> {
        self.session_store.get(parent_id).await.ok().flatten()
    }

    /// Add metadata
    pub fn with_metadata(mut self, key: &str, value: &str) -> Self {
        self.metadata.insert(key.to_string(), value.to_string());
        self
    }

    /// Set the compression strategy.
    pub fn with_compressor(mut self, compressor: Arc<dyn MessageCompressor>) -> Self {
        self.compressor = compressor;
        self
    }

    /// Compress the given messages and store the result as "精华".
    /// The input `messages` is what was just returned by get_messages().
    pub async fn compress(&mut self, messages: Vec<SessionMessage>) {
        if messages.is_empty() {
            return;
        }

        // Compress to "精华"
        let compressed = self.compressor.compress(messages).await;

        // Update cursor: last message ts from the input set
        let last_ts = compressed
            .last()
            .map(|m| m.created_at)
            .or_else(|| compressed.first().map(|m| m.created_at));

        self.compressed_messages = compressed;
        if let Some(ts) = last_ts {
            self.compressed_after_ts = Some(ts);
        }
    }

    /// Get historical messages.
    /// Before compression: returns latest messages from storage.
    /// After compression: returns [compressed精华] + [after_cursor最新].
    pub async fn get_messages(&self, limit: usize) -> Result<Vec<SessionMessage>> {
        let mut result = Vec::new();

        // First: compressed "精华" messages
        result.extend(self.compressed_messages.clone());

        // Then: latest messages after cursor (only if compressed)
        if let Some(after_ts) = self.compressed_after_ts {
            let latest = self
                .message_store
                .get_after(&self.id, after_ts, limit)
                .await
                .unwrap_or_default();
            result.extend(latest);
        } else {
            // No compression yet — return normally
            let normal = self
                .message_store
                .get_by_session(&self.id, limit)
                .await
                .unwrap_or_default();
            result.extend(normal);
        }

        Ok(result)
    }
}

impl Clone for Session {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            created_at: self.created_at,
            metadata: self.metadata.clone(),
            session_store: self.session_store.clone(),
            message_store: self.message_store.clone(),
            compressed_messages: self.compressed_messages.clone(),
            compressed_after_ts: self.compressed_after_ts,
            compressor: self.compressor.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory_store::{InMemoryMessageStore, InMemorySessionStore};
    use vol_llm_core::Message;

    #[tokio::test]
    async fn test_session_get_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        );

        let msg = SessionMessage::new("session-1".to_string(), Message::user("Hello"));
        session.add_message(msg).await.unwrap();

        let messages = session.get_messages(10).await.unwrap();
        assert_eq!(messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_with_metadata() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());

        let session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        )
        .with_metadata("user_id", "user-123");

        assert_eq!(
            session.metadata.get("user_id"),
            Some(&"user-123".to_string())
        );
    }

    #[tokio::test]
    async fn test_session_compress_and_get_messages() {
        use crate::compressors::PositionSampleCompressor;
        use std::sync::Arc;

        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());

        let mut session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
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

        // Compress with keep_first=2, sample_every=3
        // keep first 2: [msg-0, msg-1]
        // rest [msg-2..msg-9]: sample every 3rd → indices 0,3,6 → [msg-2, msg-5, msg-8]
        // last msg-9 not in result → add it
        // Expected: [msg-0, msg-1, msg-2, msg-5, msg-8, msg-9] = 6 messages
        session.compressor = Arc::new(PositionSampleCompressor::new(2, 3));
        session.compress(messages).await;

        // After compression: should have 6 compressed messages
        let compressed = session.get_messages(20).await.unwrap();
        assert_eq!(compressed.len(), 6);
        assert_eq!(
            compressed[0].message.content.as_ref().unwrap().as_str(),
            "msg-0"
        );
        assert_eq!(
            compressed.last().unwrap().message.content.as_ref().unwrap().as_str(),
            "msg-9"
        );
    }

    #[tokio::test]
    async fn test_session_compress_empty_messages() {
        let session_store = Arc::new(InMemorySessionStore::new());
        let message_store = Arc::new(InMemoryMessageStore::new());

        let mut session = Session::new(
            "session-1".to_string(),
            session_store.clone(),
            message_store.clone(),
        );

        // Compress with empty input should be no-op
        session.compress(vec![]).await;
        assert!(session.compressed_messages.is_empty());
        assert!(session.compressed_after_ts.is_none());
    }
}
