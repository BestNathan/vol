use std::sync::Arc;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::def::SkillDef;

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
}
