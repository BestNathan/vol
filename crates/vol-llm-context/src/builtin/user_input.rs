use async_trait::async_trait;
use vol_llm_core::Message;

use vol_llm_core::{AttentionAnchor, ContextBlock, ContextContributor, estimate_tokens};

/// User input contributor — wraps the user's query as a Tail-anchored message.
pub struct UserInputContributor {
    input: String,
    cached_block: Option<ContextBlock>,
}

impl UserInputContributor {
    pub fn new(input: impl Into<String>) -> Self {
        Self {
            input: input.into(),
            cached_block: None,
        }
    }
}

#[async_trait]
impl ContextContributor for UserInputContributor {
    fn name(&self) -> &str {
        "user_input"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        if let Some(ref block) = self.cached_block {
            return vec![block.clone()];
        }
        let msg = Message::user(self.input.clone());
        let block = ContextBlock::new(vec![msg], AttentionAnchor::Tail(0));
        vec![block]
    }

    async fn compress(&mut self) {
        // Non-compressible
    }

    fn estimate_size(&self) -> usize {
        self.cached_block
            .as_ref()
            .map(|b| b.messages.iter().map(estimate_tokens).sum())
            .unwrap_or(0)
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(UserInputContributor {
            input: self.input.clone(),
            cached_block: self.cached_block.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_user_input_contributor() {
        let contributor = UserInputContributor::new("fix the bug");
        let blocks = contributor.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Tail(0)));
        assert!(blocks[0].messages[0].content.as_ref().unwrap().as_str().contains("fix the bug"));
    }

    #[tokio::test]
    async fn test_user_input_clone() {
        let contributor = UserInputContributor::new("hello");
        let cloned = contributor.clone_box();
        let blocks = cloned.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].messages[0].content.as_ref().unwrap().as_str().contains("hello"));
    }
}
