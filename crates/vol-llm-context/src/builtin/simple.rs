use async_trait::async_trait;
use vol_llm_core::Message;

use crate::{AttentionAnchor, ContextBlock, ContextContributor, ContextError, estimate_tokens};

/// A simple contributor for ad-hoc context blocks.
pub struct SimpleContributor {
    messages: Vec<Message>,
    anchor: AttentionAnchor,
    name: String,
}

impl SimpleContributor {
    pub fn new(name: impl Into<String>, messages: Vec<Message>, anchor: AttentionAnchor) -> Self {
        Self {
            messages,
            anchor,
            name: name.into(),
        }
    }

    /// Create a system prompt contributor (Head zone, position 0).
    pub fn system(content: String) -> Self {
        Self::new(
            "system",
            vec![Message::system(content)],
            AttentionAnchor::Head(0),
        )
    }
}

#[async_trait]
impl ContextContributor for SimpleContributor {
    fn name(&self) -> &str {
        &self.name
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        Ok(vec![ContextBlock::new(self.messages.clone(), self.anchor.clone())])
    }

    async fn compress(&mut self) {
        // No-op
    }

    fn estimate_size(&self) -> usize {
        self.messages.iter().map(estimate_tokens).sum()
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(SimpleContributor {
            messages: self.messages.clone(),
            anchor: self.anchor.clone(),
            name: self.name.clone(),
        })
    }
}
