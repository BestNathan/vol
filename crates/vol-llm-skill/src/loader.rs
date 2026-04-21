use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

use crate::def::{SkillDef, SkillMetadata, SkillScope};
use crate::parser::{parse_skill_content, scan_skill_files};
use crate::Result;

/// Discovers, loads, and caches skills from registered roots.
pub struct SkillLoader {
    roots: Vec<(SkillScope, PathBuf)>,
    skills: Arc<RwLock<HashMap<String, Arc<SkillDef>>>>,
    metadata_cache: Arc<RwLock<Vec<SkillMetadata>>>,
}

impl SkillLoader {
    /// Creates a loader with no default roots (useful for tests).
    pub fn new_empty() -> Self {
        Self {
            roots: Vec::new(),
            skills: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Creates a loader with default roots.
    ///
    /// Default roots:
    /// - User: `~/.agents/skills/`
    /// - Repo: `{working_dir}/.agents/skills/` (if working_dir provided)
    pub fn new(working_dir: Option<PathBuf>) -> Self {
        let mut loader = Self {
            roots: Vec::new(),
            skills: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
        };

        // User root
        if let Some(home) = dirs::home_dir() {
            let user_root = home.join(".agents").join("skills");
            loader.add_root(SkillScope::User, user_root);
        }

        // Repo root
        if let Some(ref wd) = working_dir {
            let repo_root = wd.join(".agents").join("skills");
            loader.add_root(SkillScope::Repo, repo_root);
        }

        loader
    }

    /// Add a discovery root.
    pub fn add_root(&mut self, scope: SkillScope, path: PathBuf) {
        self.roots.push((scope, path));
    }

    /// Discover skills from all registered roots.
    pub async fn discover_all(&self) -> Result<()> {
        let mut skills_map = HashMap::new();

        for (scope, root_path) in &self.roots {
            if !root_path.exists() || !root_path.is_dir() {
                continue;
            }

            let entries = match std::fs::read_dir(root_path) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(path = %root_path.display(), error = %e, "Failed to read skill root");
                    continue;
                }
            };

            for entry in entries.flatten() {
                let dir_path = entry.path();
                if !dir_path.is_dir() {
                    continue;
                }

                let skill_md = dir_path.join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }

                let content = match std::fs::read_to_string(&skill_md) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), error = %e, "Failed to read SKILL.md, skipping");
                        continue;
                    }
                };

                let parsed = match parse_skill_content(&content) {
                    Ok(p) => p,
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), error = %e, "Failed to parse SKILL.md, skipping");
                        continue;
                    }
                };

                let file_listing = scan_skill_files(&dir_path);
                let id = format!("{}:{}", scope.prefix(), parsed.name);

                let def = SkillDef {
                    id: id.clone(),
                    name: parsed.name.clone(),
                    version: parsed.version,
                    description: parsed.description,
                    scope: scope.clone(),
                    triggers: parsed.triggers,
                    content: parsed.body,
                    file_listing,
                };

                // First-loaded wins: don't overwrite existing
                if !skills_map.contains_key(&parsed.name) {
                    skills_map.insert(parsed.name, Arc::new(def));
                } else {
                    tracing::warn!(skill = %parsed.name, "Duplicate skill name, keeping existing");
                }
            }
        }

        // Merge into main map (discover_all can be called multiple times)
        let mut guard = self.skills.write().await;
        for (name, def) in skills_map {
            guard.insert(name, def);
        }
        drop(guard);

        // Rebuild metadata cache
        self.rebuild_metadata().await;

        Ok(())
    }

    /// List metadata for progressive disclosure.
    pub async fn list_metadata(&self) -> Vec<SkillMetadata> {
        self.metadata_cache.read().await.clone()
    }

    /// Get full skill by name.
    pub async fn get(&self, name: &str) -> Option<Arc<SkillDef>> {
        self.skills.read().await.get(name).cloned()
    }

    /// Find skills whose triggers match the query (case-insensitive keyword match).
    pub async fn get_by_trigger(&self, query: &str) -> Vec<Arc<SkillDef>> {
        let guard = self.skills.read().await;
        let query_lower = query.to_lowercase();
        guard
            .values()
            .filter(|def| {
                def.triggers
                    .iter()
                    .any(|t| query_lower.contains(&t.to_lowercase()) || t.to_lowercase().contains(&query_lower))
            })
            .cloned()
            .collect()
    }

    /// Register a skill directly (code-registered).
    pub async fn register(&self, skill: SkillDef) {
        let name = skill.name.clone();
        self.skills.write().await.insert(name, Arc::new(skill));
        self.rebuild_metadata().await;
    }

    /// Rebuild the metadata cache from current skills.
    async fn rebuild_metadata(&self) {
        let guard = self.skills.read().await;
        let metadata: Vec<SkillMetadata> = guard.values().map(|d| d.as_ref().into()).collect();
        drop(guard);
        *self.metadata_cache.write().await = metadata;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[tokio::test]
    async fn test_discover_skills_from_temp_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skills_dir = temp_dir.path().join(".agents").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        // Create a valid skill
        let rust_dir = skills_dir.join("rust-conventions");
        std::fs::create_dir_all(&rust_dir).unwrap();
        let mut f = std::fs::File::create(rust_dir.join("SKILL.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: rust-conventions").unwrap();
        writeln!(f, "version: 1.0.0").unwrap();
        writeln!(f, "description: Rust conventions").unwrap();
        writeln!(f, "triggers: [rust]").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "# Rust Conventions").unwrap();

        // Create an invalid skill (no SKILL.md)
        let invalid_dir = skills_dir.join("invalid-skill");
        std::fs::create_dir_all(&invalid_dir).unwrap();

        // Use a loader with only our test root to avoid home dir pollution
        let mut loader = SkillLoader::new(None);
        // Replace roots with only our test dir
        loader.roots.clear();
        loader.add_root(SkillScope::User, skills_dir.clone());
        loader.discover_all().await.unwrap();

        // Check that our skill was discovered
        let skill = loader.get("rust-conventions").await;
        assert!(skill.is_some(), "rust-conventions skill should exist");
        assert!(skill.unwrap().content.contains("# Rust Conventions"));
    }

    #[tokio::test]
    async fn test_discover_empty_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let non_existent = temp_dir.path().join("nonexistent");
        // Use a loader with only our empty test root
        let mut loader = SkillLoader::new(None);
        loader.roots.clear();
        loader.add_root(SkillScope::User, non_existent);
        let result = loader.discover_all().await;
        assert!(result.is_ok());
        // After discovering from empty root, no new skills should be added
        // (home dir skills may already exist, so we check no error occurred)
        let _ = loader.list_metadata().await;
    }

    #[tokio::test]
    async fn test_get_by_trigger() {
        let loader = SkillLoader::new(None);
        let mut skill = SkillDef::new("test-skill", "some content")
            .with_triggers(vec!["rust".to_string(), "coding".to_string()]);
        skill.id = "code:test-skill".to_string();
        loader.register(skill).await;

        let results = loader.get_by_trigger("rust").await;
        assert_eq!(results.len(), 1);

        let no_results = loader.get_by_trigger("python").await;
        assert_eq!(no_results.len(), 0);
    }

    #[tokio::test]
    async fn test_register_overwrites() {
        let loader = SkillLoader::new(None);
        let mut skill1 = SkillDef::new("dup", "content1");
        skill1.id = "user:dup".to_string();
        loader.register(skill1).await;

        let mut skill2 = SkillDef::new("dup", "content2");
        skill2.id = "repo:dup".to_string();
        loader.register(skill2).await;

        // Later registration overwrites (last wins)
        let skill = loader.get("dup").await.unwrap();
        assert_eq!(skill.content, "content2");
    }
}
