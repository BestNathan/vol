//! Agent discovery, loading, and caching from .agents/agents/ directories.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{OnceCell, RwLock};

use crate::agent_def::{AgentDef, AgentFrontmatter, AgentMetadata, AgentScope, AgentDefError};

/// Discovers, loads, and caches agent definitions from user and repo scopes.
pub struct AgentLoader {
    roots: Vec<(AgentScope, PathBuf)>,
    agents: Arc<RwLock<HashMap<String, Arc<AgentDef>>>>,
    metadata_cache: Arc<RwLock<Vec<AgentMetadata>>>,
    discovered: OnceCell<()>,
}

impl AgentLoader {
    /// Creates a loader with no default roots (useful for tests).
    pub fn new_empty() -> Self {
        Self {
            roots: Vec::new(),
            agents: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
            discovered: OnceCell::new(),
        }
    }

    /// Creates a loader with user and repo roots.
    pub fn new(working_dir: Option<PathBuf>) -> Self {
        let mut loader = Self {
            roots: Vec::new(),
            agents: Arc::new(RwLock::new(HashMap::new())),
            metadata_cache: Arc::new(RwLock::new(Vec::new())),
            discovered: OnceCell::new(),
        };

        if let Some(home) = dirs::home_dir() {
            let user_root = home.join(".agents").join("agents");
            loader.add_root(AgentScope::User, user_root);
        }

        if let Some(ref wd) = working_dir {
            let repo_root = wd.join(".agents").join("agents");
            loader.add_root(AgentScope::Repo, repo_root);
        }

        loader
    }

    /// Add a discovery root.
    pub fn add_root(&mut self, scope: AgentScope, path: PathBuf) {
        self.roots.push((scope, path));
    }

    /// Discover agents from all registered roots.
    pub async fn discover_all(&self) -> Result<(), AgentDefError> {
        let mut agents_map = HashMap::new();

        for (scope, root_path) in &self.roots {
            if !root_path.exists() || !root_path.is_dir() {
                continue;
            }

            let entries = match std::fs::read_dir(root_path) {
                Ok(e) => e,
                Err(e) => {
                    tracing::warn!(path = %root_path.display(), error = %e, "Failed to read agent root");
                    continue;
                }
            };

            for entry in entries.flatten() {
                let file_path = entry.path();
                if !file_path.is_file() || file_path.extension().is_none_or(|ext| ext != "md") {
                    continue;
                }

                let content = match std::fs::read_to_string(&file_path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(path = %file_path.display(), error = %e, "Failed to read agent file, skipping");
                        continue;
                    }
                };

                let doc = match md_frontmatter::parse::<AgentFrontmatter>(&content) {
                    Ok(doc) => doc,
                    Err(e) => {
                        tracing::warn!(path = %file_path.display(), error = %e, "Failed to parse agent frontmatter, skipping");
                        continue;
                    }
                };

                let fm = &doc.frontmatter;
                let r#type = fm.resolve_type();
                let id = format!("{}:{}", scope.prefix(), fm.name);

                let def = AgentDef {
                    id,
                    name: fm.name.clone(),
                    r#type,
                    description: fm.description.clone(),
                    scope: scope.clone(),
                    tools: fm.tools.clone(),
                    disallowed_tools: fm.disallowed_tools.clone(),
                    model: fm.model.clone(),
                    max_iterations: fm.resolve_max_iterations(),
                    content: doc.body,
                };

                match agents_map.entry(doc.frontmatter.name) {
                    std::collections::hash_map::Entry::Vacant(e) => {
                        e.insert(Arc::new(def));
                    }
                    std::collections::hash_map::Entry::Occupied(e) => {
                        tracing::warn!(agent = %e.key(), "Duplicate agent name, keeping existing");
                    }
                }
            }
        }

        let mut guard = self.agents.write().await;
        for (name, def) in agents_map {
            guard.insert(name, def);
        }
        drop(guard);

        self.rebuild_metadata().await;

        Ok(())
    }

    /// Ensure agents are discovered on first access.
    async fn ensure_discovered(&self) {
        self.discovered
            .get_or_init(|| async {
                let _ = self.discover_all().await;
            })
            .await;
    }

    /// List metadata for progressive disclosure.
    pub async fn list_metadata(&self) -> Vec<AgentMetadata> {
        self.ensure_discovered().await;
        self.metadata_cache.read().await.clone()
    }

    /// Get full agent definition by name.
    pub async fn get(&self, name: &str) -> Option<Arc<AgentDef>> {
        self.ensure_discovered().await;
        self.agents.read().await.get(name).cloned()
    }

    /// Find agents whose type matches the query.
    pub async fn get_by_type(&self, r#type: &str) -> Vec<Arc<AgentDef>> {
        self.ensure_discovered().await;
        let guard = self.agents.read().await;
        let type_lower = r#type.to_lowercase();
        guard
            .values()
            .filter(|def| def.r#type.to_lowercase() == type_lower)
            .cloned()
            .collect()
    }

    /// Register an agent definition directly.
    pub async fn register(&self, agent: AgentDef) {
        let name = agent.name.clone();
        self.agents.write().await.insert(name, Arc::new(agent));
        self.rebuild_metadata().await;
    }

    /// Rebuild the metadata cache from current agents.
    async fn rebuild_metadata(&self) {
        let guard = self.agents.read().await;
        let metadata: Vec<AgentMetadata> = guard.values().map(|d| d.as_ref().into()).collect();
        drop(guard);
        *self.metadata_cache.write().await = metadata;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn create_agent_file(dir: &std::path::Path, name: &str, r#type: &str, description: &str, content: &str) {
        let mut f = std::fs::File::create(dir.join(format!("{}.md", name))).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: {}", name).unwrap();
        writeln!(f, "type: {}", r#type).unwrap();
        writeln!(f, "description: {}", description).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "{}", content).unwrap();
    }

    #[tokio::test]
    async fn test_discover_agents_from_temp_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        create_agent_file(
            &agents_dir,
            "test-runner",
            "test-runner",
            "Run tests",
            "You are a test runner.",
        );

        let mut loader = AgentLoader::new(None);
        loader.roots.clear();
        loader.add_root(AgentScope::User, agents_dir.clone());
        loader.discover_all().await.unwrap();

        let agent = loader.get("test-runner").await;
        assert!(agent.is_some());
        let def = agent.unwrap();
        assert_eq!(def.r#type, "test-runner");
        assert!(def.content.contains("You are a test runner."));
    }

    #[tokio::test]
    async fn test_discover_empty_dir() {
        let temp_dir = tempfile::tempdir().unwrap();
        let non_existent = temp_dir.path().join("nonexistent");
        let mut loader = AgentLoader::new(None);
        loader.roots.clear();
        loader.add_root(AgentScope::User, non_existent);
        let result = loader.discover_all().await;
        assert!(result.is_ok());
        assert!(loader.list_metadata().await.is_empty());
    }

    #[tokio::test]
    async fn test_get_by_type() {
        let loader = AgentLoader::new(None);
        let def = AgentDef::new("reviewer", "You review code.")
            .with_type("code-reviewer")
            .with_description("Reviews code");
        loader.register(def).await;

        let results = loader.get_by_type("code-reviewer").await;
        assert_eq!(results.len(), 1);

        let no_results = loader.get_by_type("test-runner").await;
        assert_eq!(no_results.len(), 0);
    }

    #[tokio::test]
    async fn test_register_overwrites() {
        let loader = AgentLoader::new(None);
        let def1 = AgentDef::new("dup", "content1").with_type("type-a");
        loader.register(def1).await;

        let def2 = AgentDef::new("dup", "content2").with_type("type-b");
        loader.register(def2).await;

        let def = loader.get("dup").await.unwrap();
        assert_eq!(def.content, "content2");
    }

    #[tokio::test]
    async fn test_invalid_frontmatter_skipped() {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        std::fs::write(
            agents_dir.join("no-frontmatter.md"),
            "# No Frontmatter\n\nJust markdown.",
        ).unwrap();

        let mut f = std::fs::File::create(agents_dir.join("bad-yaml.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: [unclosed").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "# Body").unwrap();

        let mut loader = AgentLoader::new(None);
        loader.roots.clear();
        loader.add_root(AgentScope::User, agents_dir.clone());
        loader.discover_all().await.unwrap();

        assert!(loader.get("no-frontmatter").await.is_none());
        assert!(loader.get("bad-yaml").await.is_none());
    }

    #[tokio::test]
    async fn test_type_defaults_to_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let agents_dir = temp_dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();

        let mut f = std::fs::File::create(agents_dir.join("my-agent.md")).unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "name: my-agent").unwrap();
        writeln!(f, "description: An agent").unwrap();
        writeln!(f, "---").unwrap();
        writeln!(f, "You are my-agent.").unwrap();

        let mut loader = AgentLoader::new(None);
        loader.roots.clear();
        loader.add_root(AgentScope::User, agents_dir.clone());
        loader.discover_all().await.unwrap();

        let def = loader.get("my-agent").await.unwrap();
        assert_eq!(def.r#type, "my-agent");
        let by_type = loader.get_by_type("my-agent").await;
        assert_eq!(by_type.len(), 1);
    }
}
