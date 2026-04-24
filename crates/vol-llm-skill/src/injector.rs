use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_core::context_block::{AttentionAnchor, ContextBlock};
use vol_llm_core::context_contributor::ContextContributor;
use vol_llm_core::Message;

use crate::loader::SkillLoader;

/// Formats skill metadata for system prompt injection.
pub struct SkillInjector {
    loader: Arc<SkillLoader>,
}

impl SkillInjector {
    pub fn new(loader: Arc<SkillLoader>) -> Self {
        Self { loader }
    }

    /// Format metadata as prompt string for system prompt injection.
    ///
    /// Returns empty string if no skills are available.
    pub async fn format_metadata(&self) -> String {
        let metadata = self.loader.list_metadata().await;
        if metadata.is_empty() {
            return String::new();
        }

        let mut output = String::from("Available skills:\n");
        for m in &metadata {
            output.push_str(&format!("- {}: {}\n", m.name, m.description));
        }
        output.push_str("\nUse the `skill` tool to load any skill's full instructions.");
        output
    }
}

#[async_trait]
impl ContextContributor for SkillInjector {
    fn name(&self) -> &str {
        "skills"
    }

    async fn contribute(&self) -> Vec<ContextBlock> {
        let metadata_text = self.format_metadata().await;
        if metadata_text.is_empty() {
            return vec![];
        }
        let msg = Message::user(metadata_text);
        vec![ContextBlock::new(vec![msg], AttentionAnchor::Head(0))]
    }

    async fn compress(&mut self) {
        // Skills are static prompt content — nothing to compress.
    }

    fn estimate_size(&self) -> usize {
        0
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(SkillInjector {
            loader: self.loader.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::SkillDef;
    use vol_llm_core::context_contributor::ContextContributor;

    #[tokio::test]
    async fn test_format_metadata_empty() {
        let loader = SkillLoader::new(None);
        let injector = SkillInjector::new(Arc::new(loader));
        let output = injector.format_metadata().await;
        assert!(output.is_empty());
    }

    #[tokio::test]
    async fn test_format_metadata_with_skills() {
        let loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("rust-conventions", "# Rust")
            .with_description("Rust coding conventions")
            .with_triggers(vec!["rust".to_string()]);
        skill.id = "user:rust-conventions".to_string();
        loader.register(skill).await;

        let injector = SkillInjector::new(Arc::new(loader));
        let output = injector.format_metadata().await;

        assert!(output.contains("Available skills:"));
        assert!(output.contains("rust-conventions"));
        assert!(output.contains("Rust coding conventions"));
        assert!(output.contains("skill"));
    }

    #[tokio::test]
    async fn test_skill_injector_contribute_empty() {
        let loader = SkillLoader::new(None);
        let injector = SkillInjector::new(Arc::new(loader));
        let blocks = injector.contribute().await;
        assert!(blocks.is_empty());
    }

    #[tokio::test]
    async fn test_skill_injector_contribute_with_skills() {
        let loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("rust-conventions", "# Rust")
            .with_description("Rust coding conventions")
            .with_triggers(vec!["rust".to_string()]);
        skill.id = "user:rust-conventions".to_string();
        loader.register(skill).await;

        let injector = SkillInjector::new(Arc::new(loader));
        let blocks = injector.contribute().await;
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].messages[0].content.as_ref().unwrap().as_str().contains("Available skills:"));
        assert!(blocks[0].messages[0].content.as_ref().unwrap().as_str().contains("rust-conventions"));
    }

    #[tokio::test]
    async fn test_skill_injector_compress_noop() {
        let loader = SkillLoader::new(None);
        let mut injector = SkillInjector::new(Arc::new(loader));
        injector.compress().await;
        // No panic, no state change — compress is a no-op
    }

    #[tokio::test]
    async fn test_skill_injector_clone_box() {
        let loader = SkillLoader::new(None);
        let injector = SkillInjector::new(Arc::new(loader));
        let cloned = injector.clone_box();
        assert_eq!(cloned.name(), "skills");
    }
}
