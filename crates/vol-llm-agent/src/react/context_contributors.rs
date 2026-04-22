//! Agent-local context contributors that depend on vol-session.

use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_context::block::{AttentionAnchor, ContextBlock, estimate_tokens};
use vol_llm_context::contributor::ContextContributor;
use vol_llm_core::Message;
use crate::session::Session;

/// Session contributor — retrieves historical messages from a session.
/// Returns them as a single ContextBlock with Middle(0) anchor.
pub struct SessionContributor {
    session: Arc<Session>,
    max_history: usize,
    cached_blocks: Option<Vec<ContextBlock>>,
}

impl SessionContributor {
    pub fn new(session: Arc<Session>, max_history: usize) -> Self {
        Self {
            session,
            max_history,
            cached_blocks: None,
        }
    }
}

#[async_trait]
impl ContextContributor for SessionContributor {
    fn name(&self) -> &str {
        "session"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        if let Some(ref blocks) = self.cached_blocks {
            return blocks.clone();
        }

        let history = self
            .session
            .get_messages(self.max_history)
            .await
            .unwrap_or_default();

        if history.is_empty() {
            return vec![];
        }

        let messages: Vec<Message> = history.into_iter().map(|sm| sm.message).collect();
        let block = ContextBlock::new(messages, AttentionAnchor::Middle(0));
        vec![block]
    }

    async fn compress(&mut self) {
        // Session history is non-compressible
    }

    fn estimate_size(&self) -> usize {
        self.cached_blocks
            .as_ref()
            .map(|blocks| blocks.iter().flat_map(|b| &b.messages).map(estimate_tokens).sum())
            .unwrap_or(0)
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(SessionContributor {
            session: self.session.clone(),
            max_history: self.max_history,
            cached_blocks: self.cached_blocks.clone(),
        })
    }
}
