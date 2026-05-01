use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde::Deserialize;
use tokio::sync::{OnceCell, RwLock};

use crate::def::{SkillDef, SkillMetadata, SkillScope};
use crate::Result;

fn default_version() -> String {
    "1.0.0".to_string()
}

#[derive(Debug, Deserialize)]
struct SkillFrontmatter {
    name: String,
    #[serde(default = "default_version")]
    version: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    triggers: Vec<String>,
}

/// Discovers, loads, and caches skills from registered roots.
pub struct SkillLoader {
    roots: Vec<(SkillScope, PathBuf)>,
    skills: Arc<RwLock<HashMap<String, Arc<SkillDef>>>>,
    metadata_cache: Arc<RwLock<Vec<SkillMetadata>>>,
    discovered: OnceCell<()>,
}

impl SkillLoader {
    /// Creates a loader with no default roots (useful for tests).
    pub fn new_empty() -> Self {
        Self {
            roots: Vec::new(),
            skills: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
            discovered: OnceCell::new(),
        }
    }

    /// Creates a loader with default roots.
    pub fn new(working_dir: Option<PathBuf>) -> Self {
        let mut loader = Self {
            roots: Vec::new(),
            skills: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
            discovered: OnceCell::new(),
        };

        if let Some(home) = dirs::home_dir() {
            let user_root = home.join(".agents").join("skills");
            loader.add_root(SkillScope::User, user_root);
        }

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

                let doc = match md_frontmatter::parse::<SkillFrontmatter>(&content) {
                    Ok(doc) => doc,
                    Err(e) => {
                        tracing::warn!(path = %skill_md.display(), error = %e, "Failed to parse SKILL.md frontmatter, skipping");
                        continue;
                    }
                };

                let file_listing = scan_skill_files(&dir_path);
                let id = format!("{}:{}", scope.prefix(), doc.frontmatter.name);

                let def = SkillDef {
                    id: id.clone(),
                    name: doc.frontmatter.name.clone(),
                    version: doc.frontmatter.version,
                    description: doc.frontmatter.description,
                    scope: scope.clone(),
                    triggers: doc.frontmatter.triggers,
                    content: doc.body,
                    file_listing,
                };

                match skills_map.entry(doc.frontmatter.name) {
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(Arc::new(def));
                    }
                    std::collections::hash_map::Entry::Occupied(e) => {
                        tracing::warn!(skill = %e.key(), "Duplicate skill name, keeping existing");
                    }
                }
            }
        }

        let mut guard = self.skills.write().await;
        for (name, def) in skills_map {
            guard.insert(name, def);
        }
        drop(guard);

        self.rebuild_metadata().await;

        Ok(())
    }

    /// Ensure skills are discovered on first access.
    async fn ensure_discovered(&self) {
        self.discovered
            .get_or_init(|| async {
                let _ = self.discover_all().await;
            })
            .await;
    }

    /// List metadata for progressive disclosure.
    pub async fn list_metadata(&self) -> Vec<SkillMetadata> {
        self.ensure_discovered().await;
        self.metadata_cache.read().await.clone()
    }

    /// Get full skill by name.
    pub async fn get(&self, name: &str) -> Option<Arc<SkillDef>> {
        self.ensure_discovered().await;
        self.skills.read().await.get(name).cloned()
    }

    /// Find skills whose triggers match the query.
    pub async fn get_by_trigger(&self, query: &str) -> Vec<Arc<SkillDef>> {
        self.ensure_discovered().await;
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

    /// Register a skill directly.
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

/// Scan a skill directory for all files, returning relative paths.
fn scan_skill_files(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(root) {
        for entry in entries.flatten() {
            let path = entry.path();
            collect_files_recursive(&path, root, &mut files);
        }
    }
    files.sort();
    files
}

fn collect_files_recursive(path: &Path, root: &Path, files: &mut Vec<String>) {
    if path.is_file() {
        if let Ok(rel) = path.strip_prefix(root) {
            files.push(rel.to_string_lossy().to_string());
        }
    } else if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect_files_recursive(&entry.path(), root, files);
            }
        }
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

        let invalid_dir = skills_dir.join("invalid-skill");
        std::fs::create_dir_all(&invalid_dir).unwrap();

        let mut loader = SkillLoader::new(None);
        loader.roots.clear();
        loader.add_root(SkillScope::User, skills_dir.clone());
        loader.discover_all().await.unwrap();

        let skill = loader.get("rust-conventions").await;
        assert!(skill.is_some(), "rust-conventions skill should exist");
        assert!(skill.unwrap().content.contains("# Rust Conventions"));
    }

    #[tokio::test]
    async fn test_discover_empty_root() {
        let temp_dir = tempfile::tempdir().unwrap();
        let non_existent = temp_dir.path().join("nonexistent");
        let mut loader = SkillLoader::new(None);
        loader.roots.clear();
        loader.add_root(SkillScope::User, non_existent);
        let result = loader.discover_all().await;
        assert!(result.is_ok());
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

        let skill = loader.get("dup").await.unwrap();
        assert_eq!(skill.content, "content2");
    }

    #[tokio::test]
    async fn test_invalid_frontmatter_skipped() {
        let temp_dir = tempfile::tempdir().unwrap();
        let skills_dir = temp_dir.path().join(".agents").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        // Skill with no frontmatter — should be skipped
        let no_fm_dir = skills_dir.join("no-frontmatter");
        std::fs::create_dir_all(&no_fm_dir).unwrap();
        std::fs::write(no_fm_dir.join("SKILL.md"), "# No Frontmatter\n\nJust markdown.").unwrap();

        // Skill with invalid YAML — should be skipped
        let bad_yaml_dir = skills_dir.join("bad-yaml");
        std::fs::create_dir_all(&bad_yaml_dir).unwrap();
        let mut f = std::fs::File::create(bad_yaml_dir.join("SKILL.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: [unclosed").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "# Body").unwrap();

        let mut loader = SkillLoader::new(None);
        loader.roots.clear();
        loader.add_root(SkillScope::User, skills_dir.clone());
        loader.discover_all().await.unwrap();

        // Both should be skipped — no skills discovered
        assert!(loader.get("no-frontmatter").await.is_none());
        assert!(loader.get("bad-yaml").await.is_none());
        assert!(loader.list_metadata().await.is_empty());
    }
}
