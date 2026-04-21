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
