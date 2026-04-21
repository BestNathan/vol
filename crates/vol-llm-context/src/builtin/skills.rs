use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_core::Message;

use crate::block::{AttentionAnchor, ContextBlock, estimate_tokens};
use crate::contributor::ContextContributor;
use vol_llm_skill::{SkillInjector, SkillLoader};

/// Skills contributor — injects available skill metadata into the system prompt.
///
/// Wraps a `SkillInjector` to format skill metadata as a system message.
/// Anchor: Head(20) — after rules, before other middle content.
pub struct SkillsContributor {
    injector: Arc<SkillInjector>,
    cached_content: Option<String>,
}

impl SkillsContributor {
    pub fn new(loader: Arc<SkillLoader>) -> Self {
        Self {
            injector: Arc::new(SkillInjector::new(loader)),
            cached_content: None,
        }
    }
}

#[async_trait]
impl ContextContributor for SkillsContributor {
    fn name(&self) -> &str {
        "skills"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        if let Some(ref content) = self.cached_content {
            if !content.is_empty() {
                let msg = Message::system(content.clone());
                return vec![ContextBlock::new(vec![msg], AttentionAnchor::Head(20))];
            }
        }
        vec![]
    }

    async fn compress(&mut self) {
        // Skills metadata is typically small and non-compressible.
        // We refresh the cache on each compress call (which is called before re-collect).
        let content = self.injector.format_metadata().await;
        self.cached_content = Some(content);
    }

    fn estimate_size(&self) -> usize {
        self.cached_content
            .as_ref()
            .map(|c| estimate_tokens(&Message::system(c.clone())))
            .unwrap_or(0)
    }
}

/// Pre-cached skills contributor — stores the formatted content directly.
/// Use this when you want to capture skills at a specific point in time.
pub struct CachedSkillsContributor {
    content: String,
}

impl CachedSkillsContributor {
    pub fn new(content: String) -> Self {
        Self { content }
    }
}

#[async_trait]
impl ContextContributor for CachedSkillsContributor {
    fn name(&self) -> &str {
        "skills_cached"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        if self.content.is_empty() {
            return vec![];
        }
        let msg = Message::system(self.content.clone());
        vec![ContextBlock::new(vec![msg], AttentionAnchor::Head(20))]
    }

    async fn compress(&mut self) {
        // Non-compressible
    }

    fn estimate_size(&self) -> usize {
        if self.content.is_empty() {
            0
        } else {
            estimate_tokens(&Message::system(self.content.clone()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_skill::SkillDef;

    #[tokio::test]
    async fn test_skills_contributor_empty() {
        let loader = Arc::new(SkillLoader::new(None));
        let contributor = SkillsContributor::new(loader);
        // Before compress, no cached content
        let blocks = contributor.contribute().await;
        assert_eq!(blocks.len(), 0);
    }

    #[tokio::test]
    async fn test_skills_contributor_with_skills() {
        let loader = Arc::new(SkillLoader::new(None));
        let mut skill = SkillDef::new("rust-conventions", "# Rust")
            .with_description("Rust coding conventions")
            .with_triggers(vec!["rust".to_string()]);
        skill.id = "user:rust-conventions".to_string();
        loader.register(skill).await;

        let mut contributor = SkillsContributor::new(loader);
        contributor.compress().await;
        let blocks = contributor.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert!(matches!(blocks[0].anchor, AttentionAnchor::Head(20)));
        let msg = &blocks[0].messages[0];
        assert!(msg.content.as_ref().unwrap().as_str().contains("Available skills:"));
        assert!(msg.content.as_ref().unwrap().as_str().contains("rust-conventions"));
    }

    #[tokio::test]
    async fn test_cached_skills_contributor() {
        let c = CachedSkillsContributor::new("Some skills content".to_string());
        let blocks = c.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert_eq!(blocks[0].messages[0].content.as_ref().unwrap().as_str(), "Some skills content");
    }

    #[tokio::test]
    async fn test_cached_skills_contributor_empty() {
        let c = CachedSkillsContributor::new(String::new());
        let blocks = c.contribute().await;
        assert_eq!(blocks.len(), 0);
    }
}
