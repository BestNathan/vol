use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor};
use vol_llm_core::Message;

use crate::compressor::MessageCompressor;
use crate::compressors::PositionSampleCompressor;
use crate::entry::{CheckpointReason, SessionEntry};
use crate::{Session, SessionMessage};

/// Session contributor — retrieves historical messages from a session
/// and supports compression to manage context size.
pub struct SessionContributor {
    session: Arc<tokio::sync::Mutex<Session>>,
    max_history: usize,
    compressor: Arc<dyn MessageCompressor>,
    anchor: AttentionAnchor,
}

impl SessionContributor {
    pub fn new(
        session: Arc<tokio::sync::Mutex<Session>>,
        max_history: usize,
        anchor: AttentionAnchor,
    ) -> Self {
        Self {
            session,
            max_history,
            compressor: Arc::new(PositionSampleCompressor::default()),
            anchor,
        }
    }

    /// Set a custom compression strategy.
    pub fn with_compressor(mut self, compressor: Arc<dyn MessageCompressor>) -> Self {
        self.compressor = compressor;
        self
    }
}

#[async_trait]
impl ContextContributor for SessionContributor {
    fn name(&self) -> &str {
        "session"
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, vol_llm_context::ContextError> {
        let history = self
            .session
            .lock()
            .await
            .get_messages()
            .await
            .unwrap_or_default();

        if history.is_empty() {
            return Ok(vec![]);
        }

        let mut messages: Vec<Message> = history.into_iter().map(|sm| sm.message).collect();

        // Apply max_history limit: keep last N messages
        if messages.len() > self.max_history {
            messages = messages.split_off(messages.len() - self.max_history);
        }

        let block = ContextBlock::new(messages, self.anchor.clone());
        Ok(vec![block])
    }

    async fn compress(&mut self) {
        // 1. Get current messages from session
        let (session_id, messages) = {
            let session = self.session.lock().await;
            let id = session.id.clone();
            let msgs = session.get_messages().await.unwrap_or_default();
            (id, msgs)
        };
        if messages.is_empty() {
            return;
        }

        // 2. Compress the messages
        let compressed = self.compressor.compress(messages).await;
        if compressed.is_empty() {
            return;
        }

        // 3. Write checkpoint (seal old messages)
        let session = self.session.lock().await;
        let mut cp_entry = SessionEntry::new_checkpoint(
            session_id.clone(),
            CheckpointReason::Compression,
            None,
        );
        let base_ts = cp_entry.created_at;
        if let Err(e) = session.entry_store.save(cp_entry).await {
            tracing::error!("Failed to write checkpoint before compression: {}", e);
            return;
        }

        // 4. Build summary text from compressed messages
        let summary = compressed
            .iter()
            .filter_map(|m| m.message.content.as_ref())
            .map(|c| c.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // 5. Write summary entry (timestamp after checkpoint)
        let mut summary_entry = SessionEntry::new_summary(
            session_id.clone(),
            summary,
        );
        summary_entry.created_at = base_ts + 1;
        if let Err(e) = session.entry_store.save(summary_entry).await {
            tracing::error!("Failed to write summary during compression: {}", e);
            return;
        }

        // 6. Write compressed message entries (timestamp after checkpoint)
        for (i, msg) in compressed.iter().enumerate() {
            let mut entry = SessionEntry::from_message(msg.clone());
            entry.created_at = base_ts + 1 + (i as i64);
            if let Err(e) = session.entry_store.save(entry).await {
                tracing::error!("Failed to write compressed message: {}", e);
            }
        }
    }

    fn estimate_size(&self) -> usize {
        // Best-effort unknown without reading Session
        0
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(SessionContributor {
            session: self.session.clone(),
            max_history: self.max_history,
            compressor: self.compressor.clone(),
            anchor: self.anchor.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::SessionMessage;
    use crate::InMemoryEntryStore;

    #[tokio::test]
    async fn test_session_contributor_contribute() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);
        let session_msg = SessionMessage::new(session.id.clone(), Message::user("hello"));
        session.add_message(session_msg).await.unwrap();

        let contributor = SessionContributor::new(Arc::new(tokio::sync::Mutex::new(session)), 10, AttentionAnchor::Middle(0));
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_contributor_max_history() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);

        for i in 0..5 {
            let msg = SessionMessage::new(session.id.clone(), Message::user(format!("msg-{}", i)));
            session.add_message(msg).await.unwrap();
        }

        let contributor = SessionContributor::new(Arc::new(tokio::sync::Mutex::new(session)), 3, AttentionAnchor::Middle(0));
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks[0].messages.len(), 3);
    }

    #[tokio::test]
    async fn test_session_contributor_empty() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);

        let contributor = SessionContributor::new(Arc::new(tokio::sync::Mutex::new(session)), 10, AttentionAnchor::Middle(0));
        let blocks = contributor.contribute().await.unwrap();
        assert!(blocks.is_empty());
    }

    #[tokio::test]
    async fn test_session_contributor_compress() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);

        for i in 0..10 {
            let msg = SessionMessage::new(session.id.clone(), Message::user(format!("msg-{}", i)));
            session.add_message(msg).await.unwrap();
        }

        let session = Arc::new(tokio::sync::Mutex::new(session));
        let mut contributor = SessionContributor::new(session.clone(), 10, AttentionAnchor::Middle(0));

        // Before compression
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks[0].messages.len(), 10);

        // Compress
        contributor.compress().await;

        // After compression — fewer messages
        let blocks = contributor.contribute().await.unwrap();
        assert!(blocks[0].messages.len() < 10);
        // First message should be the summary (system role)
        assert_eq!(blocks[0].messages[0].role, vol_llm_core::MessageRole::System);
    }

    #[tokio::test]
    async fn test_session_contributor_compress_empty() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);
        let session = Arc::new(tokio::sync::Mutex::new(session));

        let mut contributor = SessionContributor::new(session.clone(), 10, AttentionAnchor::Middle(0));

        // Compress on empty session — no-op
        contributor.compress().await;

        let blocks = contributor.contribute().await.unwrap();
        assert!(blocks.is_empty());
    }
}
