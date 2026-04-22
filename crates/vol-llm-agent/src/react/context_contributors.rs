//! Agent-local context contributors that depend on vol-session.

use std::sync::Arc;
use std::sync::Mutex;
use async_trait::async_trait;
use vol_llm_context::block::{AttentionAnchor, ContextBlock, estimate_tokens};
use vol_llm_context::contributor::ContextContributor;
use vol_llm_core::Message;
use vol_session::SessionMessage;
use crate::session::Session;

/// Session contributor — retrieves historical messages from a session.
/// Returns them as a single ContextBlock with Middle(0) anchor.
pub struct SessionContributor {
    session: Arc<tokio::sync::Mutex<Session>>,
    max_history: usize,
    cached_blocks: Mutex<Option<Vec<ContextBlock>>>,
}

impl SessionContributor {
    pub fn new(session: Arc<tokio::sync::Mutex<Session>>, max_history: usize) -> Self {
        Self {
            session,
            max_history,
            cached_blocks: Mutex::new(None),
        }
    }
}

#[async_trait]
impl ContextContributor for SessionContributor {
    fn name(&self) -> &str {
        "session"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        // Check cache first
        {
            let guard = self.cached_blocks.lock().unwrap();
            if let Some(ref blocks) = *guard {
                return blocks.clone();
            }
        }

        let history = self
            .session
            .lock()
            .await
            .get_messages(self.max_history)
            .await
            .unwrap_or_default();

        if history.is_empty() {
            return vec![];
        }

        let messages: Vec<Message> = history.into_iter().map(|sm| sm.message).collect();
        let block = ContextBlock::new(messages, AttentionAnchor::Middle(0));
        let blocks = vec![block];

        // Cache for compress() to use
        *self.cached_blocks.lock().unwrap() = Some(blocks.clone());

        blocks
    }

    async fn compress(&mut self) {
        // Clone messages out of the cache before the await point
        let messages: Option<Vec<SessionMessage>> = {
            let guard = self.cached_blocks.lock().unwrap();
            guard.as_ref().map(|blocks| {
                blocks
                    .iter()
                    .flat_map(|b| b.messages.iter().map(|m| SessionMessage::new("".to_string(), m.clone())))
                    .collect()
            })
        };

        if let Some(messages) = messages {
            let mut session = self.session.lock().await;
            session.compress(messages).await;
        }

        // Invalidate cache — next contribute() will get compressed result
        *self.cached_blocks.get_mut().unwrap() = None;
    }

    fn estimate_size(&self) -> usize {
        self.cached_blocks
            .lock()
            .unwrap()
            .as_ref()
            .map(|blocks| blocks.iter().flat_map(|b| &b.messages).map(estimate_tokens).sum())
            .unwrap_or(0)
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(SessionContributor {
            session: self.session.clone(),
            max_history: self.max_history,
            cached_blocks: Mutex::new(self.cached_blocks.lock().unwrap().clone()),
        })
    }
}
