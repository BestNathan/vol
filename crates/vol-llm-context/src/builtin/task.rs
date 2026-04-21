use async_trait::async_trait;
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock};
use crate::contributor::ContextContributor;

/// Task contributor — describes the current task.
/// Anchor: Tail(0) — last message, high attention at prompt end.
pub struct TaskContributor {
    task: String,
}

impl TaskContributor {
    pub fn new(task: impl Into<String>) -> Self {
        Self { task: task.into() }
    }
}

#[async_trait]
impl ContextContributor for TaskContributor {
    fn name(&self) -> &str {
        "task"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        let msg = Message::user(self.task.clone());
        vec![ContextBlock::new(vec![msg], AttentionAnchor::Tail(0))]
    }

    async fn compress(&mut self) {
        // Task is non-compressible
    }

    fn estimate_size(&self) -> usize {
        self.task.len() / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_contributor() {
        let c = TaskContributor::new("Fix the bug in the login flow");
        let blocks = c.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Tail(0)));
        assert_eq!(blocks[0].messages[0].role, vol_llm_core::message::MessageRole::User);
    }
}
