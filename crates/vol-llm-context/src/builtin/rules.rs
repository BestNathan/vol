use async_trait::async_trait;
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock};
use crate::contributor::ContextContributor;

/// Rules contributor — general norms and constraints.
/// Anchor: Head(10) — after role, before other head content.
pub struct RulesContributor {
    rules: Vec<String>,
}

impl RulesContributor {
    pub fn new(rules: Vec<String>) -> Self {
        Self { rules }
    }

    pub fn add_rule(&mut self, rule: impl Into<String>) {
        self.rules.push(rule.into());
    }
}

#[async_trait]
impl ContextContributor for RulesContributor {
    fn name(&self) -> &str {
        "rules"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        if self.rules.is_empty() {
            return vec![];
        }
        let content = self.rules.join("\n");
        let msg = Message::system(content.clone());
        vec![ContextBlock::new(vec![msg], AttentionAnchor::Head(10))]
    }

    async fn compress(&mut self) {
        // Rules are non-compressible
    }

    fn estimate_size(&self) -> usize {
        self.rules.iter().map(|r| r.len() / 4).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rules_contributor() {
        let mut c = RulesContributor::new(vec![]);
        c.add_rule("Always write tests");
        c.add_rule("Use TDD approach");
        let blocks = c.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Head(10)));
        assert!(blocks[0].messages[0].content.as_ref().unwrap().as_str().contains("Always write tests"));
    }

    #[tokio::test]
    async fn test_rules_contributor_empty() {
        let c = RulesContributor::new(vec![]);
        let blocks = c.contribute().await;
        assert_eq!(blocks.len(), 0);
    }
}
