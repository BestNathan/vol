use async_trait::async_trait;

use crate::ContextBlock;

/// Error type for context operations.
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("contributor {0} failed: {1}")]
    ContributorError(String, String),
    #[error("token budget exceeded: {0}")]
    BudgetExceeded(usize),
}

/// Trait for context contributors that produce context blocks.
#[async_trait]
pub trait ContextContributor: Send + Sync {
    /// Human-readable name for debugging/logging.
    fn name(&self) -> &str;

    /// Produce context blocks for the builder.
    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError>;

    /// Compress internal state. After compression, call `contribute()` again
    /// to get the reduced blocks.
    async fn compress(&mut self);

    /// Estimate token size of this contributor's output.
    fn estimate_size(&self) -> usize;

    /// Clone into a boxed trait object.
    fn clone_box(&self) -> Box<dyn ContextContributor>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context_block::AttentionAnchor;
    use vol_llm_core::Message;

    struct TestContributor {
        content: String,
    }

    #[async_trait]
    impl ContextContributor for TestContributor {
        fn name(&self) -> &str {
            "test"
        }

        async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
            Ok(vec![ContextBlock::new(
                vec![Message::user(self.content.clone())],
                AttentionAnchor::Middle(5),
            )])
        }

        async fn compress(&mut self) {
            self.content = format!("[compressed] {}", self.content.chars().take(20).collect::<String>());
        }

        fn estimate_size(&self) -> usize {
            self.content.len() / 4
        }

        fn clone_box(&self) -> Box<dyn ContextContributor> {
            Box::new(TestContributor {
                content: self.content.clone(),
            })
        }
    }

    #[tokio::test]
    async fn test_contributor_contribute() {
        let c = TestContributor { content: "Hello world".to_string() };
        let blocks = c.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].messages.len(), 1);
    }

    #[tokio::test]
    async fn test_contributor_compress_then_contribute() {
        let mut c = TestContributor { content: "This is a long piece of text that should be compressed".to_string() };
        c.compress().await;
        let blocks = c.contribute().await.unwrap();
        assert!(blocks[0].messages.len() == 1);
    }
}
