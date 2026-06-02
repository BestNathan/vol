use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Discovery scope for skills.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SkillScope {
    /// ~/.agents/skills/ — user personal skills
    User,
    /// {working_dir}/.agents/skills/ — project-specific skills
    Repo,
    /// Custom path registered by caller (e.g., plugin-packaged skills)
    Custom(PathBuf),
}

impl SkillScope {
    /// Returns the scope prefix string for skill IDs.
    pub fn prefix(&self) -> String {
        match self {
            SkillScope::User => "user".to_string(),
            SkillScope::Repo => "repo".to_string(),
            SkillScope::Custom(path) => format!("custom:{}", path.display()),
        }
    }
}

impl std::fmt::Display for SkillScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillScope::User => write!(f, "User"),
            SkillScope::Repo => write!(f, "Repo"),
            SkillScope::Custom(path) => write!(f, "Custom({})", path.display()),
        }
    }
}

/// A skill definition loaded from a SKILL.md file or registered directly.
#[derive(Debug, Clone)]
pub struct SkillDef {
    /// Unique ID: "{scope_prefix}:{name}" e.g., "user:rust-conventions"
    pub id: String,
    /// Skill name from frontmatter
    pub name: String,
    /// Version from frontmatter
    pub version: String,
    /// Description from frontmatter
    pub description: String,
    /// Discovery scope
    pub scope: SkillScope,
    /// Trigger keywords for implicit matching
    pub triggers: Vec<String>,
    /// SKILL.md markdown body (after frontmatter)
    pub content: String,
    /// Relative file paths within the skill directory
    pub file_listing: Vec<String>,
    /// Absolute path to the skill directory (for resolving file_listing paths)
    pub directory: String,
}

impl SkillDef {
    /// Create a new skill with minimal fields.
    pub fn new(name: &str, content: impl Into<String>) -> Self {
        let content_str = content.into();
        Self {
            id: format!("code:{}", name),
            name: name.to_string(),
            version: "1.0.0".to_string(),
            description: String::new(),
            scope: SkillScope::Custom(PathBuf::new()),
            triggers: Vec::new(),
            content: content_str,
            file_listing: Vec::new(),
            directory: String::new(),
        }
    }

    /// Set triggers for implicit matching.
    pub fn with_triggers(mut self, triggers: Vec<String>) -> Self {
        self.triggers = triggers;
        self
    }

    /// Set description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    /// Set file listing.
    pub fn with_file_listing(mut self, files: Vec<String>) -> Self {
        self.file_listing = files;
        self
    }
}

/// Metadata for progressive disclosure (injected into system prompt).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub scope: SkillScope,
    pub triggers: Vec<String>,
}

impl From<&SkillDef> for SkillMetadata {
    fn from(def: &SkillDef) -> Self {
        Self {
            id: def.id.clone(),
            name: def.name.clone(),
            version: def.version.clone(),
            description: def.description.clone(),
            scope: def.scope.clone(),
            triggers: def.triggers.clone(),
        }
    }
}
