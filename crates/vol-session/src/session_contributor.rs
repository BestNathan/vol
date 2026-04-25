use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor};
use vol_llm_core::Message;

use crate::{Session, SessionMessage};

/// Session contributor — retrieves historical messages from a session.
/// Returns them as a single ContextBlock with Middle(0) anchor.
pub struct SessionContributor {
    session: Arc<tokio::sync::Mutex<Session>>,
    max_history: usize,
}

impl SessionContributor {
    pub fn new(session: Arc<tokio::sync::Mutex<Session>>, max_history: usize) -> Self {
        Self {
            session,
            max_history,
        }
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

        let block = ContextBlock::new(messages, AttentionAnchor::Middle(0));
        Ok(vec![block])
    }

    async fn compress(&mut self) {
        let messages: Option<Vec<SessionMessage>> = self
            .session
            .lock()
            .await
            .get_messages()
            .await
            .ok()
            .map(|history| history);

        if let Some(messages) = messages {
            let mut session = self.session.lock().await;
            session.compress(messages).await;
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
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{InMemoryEntryStore, SessionMessage};

    #[tokio::test]
    async fn test_session_contributor_contribute() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);
        let session_msg = SessionMessage::new(session.id.clone(), Message::user("hello"));
        session.add_message(session_msg).await.unwrap();

        let contributor = SessionContributor::new(Arc::new(tokio::sync::Mutex::new(session)), 10);
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

        let contributor = SessionContributor::new(Arc::new(tokio::sync::Mutex::new(session)), 3);
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks[0].messages.len(), 3);
    }

    #[tokio::test]
    async fn test_session_contributor_empty() {
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store);

        let contributor = SessionContributor::new(Arc::new(tokio::sync::Mutex::new(session)), 10);
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
        let mut contributor = SessionContributor::new(session.clone(), 10);

        // Before compression
        let blocks = contributor.contribute().await.unwrap();
        assert_eq!(blocks[0].messages.len(), 10);

        // Compress
        contributor.compress().await;

        // After compression — fewer messages
        let blocks = contributor.contribute().await.unwrap();
        assert!(blocks[0].messages.len() < 10);
    }
}
