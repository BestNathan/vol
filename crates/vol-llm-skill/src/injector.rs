use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use vol_llm_context::{AttentionAnchor, ContextBlock, ContextContributor, ContextError};
use vol_llm_core::Message;

use crate::loader::SkillLoader;

/// Formats skill metadata for system prompt injection.
pub struct SkillInjector {
    loader: Arc<SkillLoader>,
    anchor: AttentionAnchor,
    cached_size: tokio::sync::Mutex<usize>,
    /// Shared filter: only include skills whose names are in this set.
    /// None or empty = include all discovered skills.
    /// Wrapped in Arc<RwLock> so it can be updated externally by the
    /// capability overlay system without replacing the contributor.
    pub skill_filter: Arc<RwLock<Option<Vec<String>>>>,
}

impl SkillInjector {
    pub fn new(
        loader: Arc<SkillLoader>,
        anchor: AttentionAnchor,
        skill_filter: Option<Vec<String>>,
    ) -> Self {
        Self {
            loader,
            anchor,
            cached_size: tokio::sync::Mutex::new(0),
            skill_filter: Arc::new(RwLock::new(skill_filter)),
        }
    }

    /// Create a SkillInjector that loads skills from `{working_dir}/.agents/skills`.
    ///
    /// Skills are discovered lazily on first access.
    pub async fn from_workdir(working_dir: &std::path::Path, anchor: AttentionAnchor) -> Self {
        let loader = Arc::new(crate::loader::SkillLoader::new(Some(
            working_dir.to_path_buf(),
        )));
        Self::new(loader, anchor, None)
    }

    /// Discover skills from the configured roots.
    ///
    /// This must be called before `contribute()` will return any skills when using
    /// `from_workdir()` or `new()` with directory roots.
    pub async fn discover_all(&self) -> crate::Result<()> {
        self.loader.discover_all().await
    }

    /// Format metadata as prompt string for system prompt injection.
    ///
    /// Returns empty string if no skills are available.
    pub async fn format_metadata(&self) -> String {
        let metadata = self.loader.list_metadata().await;
        if metadata.is_empty() {
            return String::new();
        }

        // Apply skill name filter if set (reads from shared filter)
        let filter_guard = self.skill_filter.read().await;
        let filtered: Vec<_> = if let Some(ref filter) = *filter_guard {
            if filter.is_empty() {
                metadata
            } else {
                let filter_set: std::collections::HashSet<&str> =
                    filter.iter().map(String::as_str).collect();
                metadata
                    .into_iter()
                    .filter(|m| filter_set.contains(m.name.as_str()))
                    .collect()
            }
        } else {
            metadata
        };
        drop(filter_guard);

        if filtered.is_empty() {
            return String::new();
        }

        let mut output = String::from("Available skills:\n");
        for m in &filtered {
            output.push_str(&format!("- {}: {}\n", m.name, m.description));
        }
        output.push_str("\nUse the `skill` tool to load any skill's full instructions.");
        output
    }

    /// Return skill names after applying the current filter.
    pub async fn skill_names(&self) -> Vec<String> {
        let metadata = self.loader.list_metadata().await;
        let filter_guard = self.skill_filter.read().await;
        match &*filter_guard {
            Some(filter) if !filter.is_empty() => {
                let set: std::collections::HashSet<&str> =
                    filter.iter().map(String::as_str).collect();
                metadata
                    .into_iter()
                    .filter(|m| set.contains(m.name.as_str()))
                    .map(|m| m.name)
                    .collect()
            }
            _ => metadata.into_iter().map(|m| m.name).collect(),
        }
    }
}

#[async_trait]
impl ContextContributor for SkillInjector {
    fn name(&self) -> &str {
        "skills"
    }

    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        let metadata_text = self.format_metadata().await;
        if metadata_text.is_empty() {
            // Cache 0 for empty skills
            *self.cached_size.lock().await = 0;
            // Return empty placeholder block to maintain fixed Head slot
            return Ok(vec![ContextBlock::new(vec![], self.anchor.clone())]);
        }
        let msg = Message::user(metadata_text);
        // Cache the actual estimate
        let size = vol_llm_context::estimate_tokens(&msg);
        *self.cached_size.lock().await = size;
        Ok(vec![ContextBlock::new(vec![msg], self.anchor.clone())])
    }

    async fn compress(&mut self) {
        // Skills are static prompt content — nothing to compress.
    }

    fn estimate_size(&self) -> usize {
        // Try to read cached value without blocking; fall back to 0
        self.cached_size.try_lock().map(|g| *g).unwrap_or(0)
    }

    fn clone_box(&self) -> Box<dyn ContextContributor> {
        Box::new(SkillInjector {
            loader: self.loader.clone(),
            anchor: self.anchor.clone(),
            cached_size: tokio::sync::Mutex::new(0), // fresh cache for clone
            skill_filter: self.skill_filter.clone(), // share the same filter
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::SkillDef;
    use vol_llm_context::ContextContributor;

    #[tokio::test]
    async fn test_format_metadata_empty() {
        let loader = SkillLoader::new_empty();
        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
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

        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        let output = injector.format_metadata().await;

        assert!(output.contains("Available skills:"));
        assert!(output.contains("rust-conventions"));
        assert!(output.contains("Rust coding conventions"));
        assert!(output.contains("skill"));
    }

    #[tokio::test]
    async fn test_skill_injector_contribute_empty() {
        let loader = SkillLoader::new_empty();
        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        let blocks = injector.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].messages.is_empty());
    }

    #[tokio::test]
    async fn test_skill_injector_contribute_with_skills() {
        let loader = SkillLoader::new_empty();
        let mut skill = SkillDef::new("rust-conventions", "# Rust")
            .with_description("Rust coding conventions")
            .with_triggers(vec!["rust".to_string()]);
        skill.id = "user:rust-conventions".to_string();
        loader.register(skill).await;

        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        let blocks = injector.contribute().await.unwrap();
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].messages[0]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("Available skills:"));
        assert!(blocks[0].messages[0]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("rust-conventions"));
    }

    #[tokio::test]
    async fn test_skill_injector_compress_noop() {
        let loader = SkillLoader::new(None);
        let mut injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        injector.compress().await;
        // No panic, no state change — compress is a no-op
    }

    #[tokio::test]
    async fn test_skill_injector_clone_box() {
        let loader = SkillLoader::new(None);
        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        let cloned = injector.clone_box();
        assert_eq!(cloned.name(), "skills");
    }

    #[tokio::test]
    async fn test_skill_injector_clone_contribute() {
        let loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("test-skill", "# Test")
            .with_description("A test skill")
            .with_triggers(vec!["test".to_string()]);
        skill.id = "user:test-skill".to_string();
        loader.register(skill).await;

        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        let original = injector.contribute().await.unwrap();
        let cloned = injector.clone_box();
        let cloned_result = cloned.contribute().await.unwrap();

        assert_eq!(original.len(), cloned_result.len());
        assert_eq!(
            original[0].messages[0].content.as_ref().unwrap().as_str(),
            cloned_result[0].messages[0]
                .content
                .as_ref()
                .unwrap()
                .as_str()
        );
    }

    #[tokio::test]
    async fn test_skill_injector_filter_excludes_skills() {
        let loader = SkillLoader::new_empty();
        let mut skill_a = SkillDef::new("skill-a", "# A").with_description("Skill A");
        skill_a.id = "user:skill-a".into();
        let mut skill_b = SkillDef::new("skill-b", "# B").with_description("Skill B");
        skill_b.id = "user:skill-b".into();
        loader.register(skill_a).await;
        loader.register(skill_b).await;

        // Filter to only skill-b
        let injector = SkillInjector::new(
            Arc::new(loader),
            AttentionAnchor::Head(0),
            Some(vec!["skill-b".into()]),
        );
        let output = injector.format_metadata().await;
        assert!(output.contains("skill-b"));
        assert!(!output.contains("skill-a"));
    }

    #[tokio::test]
    async fn test_skill_injector_filter_none_shows_all() {
        let loader = SkillLoader::new_empty();
        let mut skill = SkillDef::new("test-skill", "# T").with_description("A test skill");
        skill.id = "user:test-skill".into();
        loader.register(skill).await;

        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        let output = injector.format_metadata().await;
        assert!(output.contains("test-skill"));
    }

    #[tokio::test]
    async fn test_skill_names_no_filter_returns_all() {
        let loader = SkillLoader::new_empty();
        let mut skill_a = SkillDef::new("skill-a", "# A").with_description("Skill A");
        skill_a.id = "user:skill-a".into();
        let mut skill_b = SkillDef::new("skill-b", "# B").with_description("Skill B");
        skill_b.id = "user:skill-b".into();
        loader.register(skill_a).await;
        loader.register(skill_b).await;

        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
        let names = injector.skill_names().await;
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"skill-a".to_string()));
        assert!(names.contains(&"skill-b".to_string()));
    }

    #[tokio::test]
    async fn test_skill_names_with_filter_returns_subset() {
        let loader = SkillLoader::new_empty();
        let mut skill_a = SkillDef::new("skill-a", "# A").with_description("Skill A");
        skill_a.id = "user:skill-a".into();
        let mut skill_b = SkillDef::new("skill-b", "# B").with_description("Skill B");
        skill_b.id = "user:skill-b".into();
        loader.register(skill_a).await;
        loader.register(skill_b).await;

        let injector = SkillInjector::new(
            Arc::new(loader),
            AttentionAnchor::Head(0),
            Some(vec!["skill-a".into()]),
        );
        let names = injector.skill_names().await;
        assert_eq!(names.len(), 1);
        assert_eq!(names[0], "skill-a");
    }

    #[tokio::test]
    async fn test_skill_names_empty_filter_returns_all() {
        let loader = SkillLoader::new_empty();
        let mut skill = SkillDef::new("test-skill", "# T").with_description("Test");
        skill.id = "user:test-skill".into();
        loader.register(skill).await;

        // Empty filter behaves like None per struct docs: include all skills.
        let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), Some(vec![]));
        let names = injector.skill_names().await;
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"test-skill".to_string()));
    }
}
