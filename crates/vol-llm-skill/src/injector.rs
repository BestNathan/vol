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
