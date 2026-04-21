use async_trait::async_trait;
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock};
use crate::contributor::ContextContributor;

/// Role contributor — sets the system role/identity.
/// Anchor: Head(0) — highest priority, first message.
pub struct RoleContributor {
    role: String,
}

impl RoleContributor {
    pub fn new(role: impl Into<String>) -> Self {
        Self { role: role.into() }
    }
}

#[async_trait]
impl ContextContributor for RoleContributor {
    fn name(&self) -> &str {
        "role"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        let msg = Message::system(self.role.clone());
        vec![ContextBlock::new(vec![msg], AttentionAnchor::Head(0))]
    }

    async fn compress(&mut self) {
        // Role is non-compressible
    }

    fn estimate_size(&self) -> usize {
        self.role.len() / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_role_contributor() {
        let c = RoleContributor::new("You are a coding assistant");
        let blocks = c.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Head(0)));
        assert_eq!(blocks[0].messages[0].role, vol_llm_core::message::MessageRole::System);
    }
}
