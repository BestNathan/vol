//! Provider configuration loader.
//!
//! Scans `.agents/providers/*.toml` from project and user directories.
//! Project-level configs override user-level configs per-key (by filename).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::ProviderFileConfig;

const PROVIDERS_DIR: &str = ".agents/providers";

/// Provider configuration with resolved ID.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NamedProviderConfig {
    pub id: String,
    #[serde(flatten)]
    pub config: ProviderFileConfig,
}

/// Loaded provider registry.
#[derive(Debug, Clone)]
pub struct ProviderLoader {
    providers: HashMap<String, ProviderFileConfig>,
}

impl ProviderLoader {
    /// Load configuration from project-level and user-level sources.
    ///
    /// Priority: `.agents/providers/` (project root) > `~/.agents/providers/` (user home).
    /// Per-key merge: if both files define the same provider ID, the project-level wins.
    pub fn load(working_dir: Option<&Path>) -> Self {
        let project_map = load_dir(working_dir);
        let user_map = load_user_dir();

        // Merge: user first (lower priority), then project (higher priority)
        let mut providers = user_map;
        providers.extend(project_map);

        Self { providers }
    }

    /// Get a provider by ID
    pub fn get(&self, id: &str) -> Option<&ProviderFileConfig> {
        self.providers.get(id)
    }

    /// Get all provider IDs
    pub fn ids(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Programmatically insert a provider (useful for testing).
    pub fn insert(&mut self, id: impl Into<String>, config: ProviderFileConfig) {
        self.providers.insert(id.into(), config);
    }

    /// Check if a provider exists
    pub fn contains(&self, id: &str) -> bool {
        self.providers.contains_key(id)
    }

    /// Number of loaded providers
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Convert to legacy LLMProviderConfig list (for migration compatibility)
    pub fn to_provider_configs(&self) -> Vec<NamedProviderConfig> {
        self.providers
            .iter()
            .map(|(id, config)| NamedProviderConfig {
                id: id.clone(),
                config: config.clone(),
            })
            .collect()
    }
}

impl Default for ProviderLoader {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
}

/// Load all TOML files from a directory, keyed by filename (without extension).
fn load_dir(dir: Option<&Path>) -> HashMap<String, ProviderFileConfig> {
    let mut map = HashMap::new();
    let Some(dir) = dir else { return map };

    let providers_dir = dir.join(PROVIDERS_DIR);
    if !providers_dir.is_dir() {
        return map;
    }

    if let Ok(entries) = std::fs::read_dir(&providers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string());
                let Some(id) = id else { continue };

                match std::fs::read_to_string(&path) {
                    Ok(content) => match toml::from_str::<ProviderFileConfig>(&content) {
                        Ok(config) => {
                            map.insert(id, config);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse provider config '{}': {}", path.display(), e);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Failed to read provider config '{}': {}", path.display(), e);
                    }
                }
            }
        }
    }

    map
}

/// Load user-level provider configs from ~/.agents/providers/
fn load_user_dir() -> HashMap<String, ProviderFileConfig> {
    let home = dirs::home_dir();
    load_dir(home.as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_file(dir: &Path, name: &str, content: &str) {
        std::fs::create_dir_all(dir.join(PROVIDERS_DIR)).unwrap();
        let mut file = std::fs::File::create(dir.join(PROVIDERS_DIR).join(name)).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_single_provider() {
        let dir = tempfile::tempdir().unwrap();
        write_test_file(
            dir.path(),
            "anthropic-test.toml",
            r#"
provider = "anthropic"
model = "claude-test"
api_key = "${TEST_KEY}"
base_url = "https://api.test.com"
"#,
        );

        let loader = ProviderLoader::load(Some(dir.path()));
        assert_eq!(loader.len(), 1);
        assert!(loader.contains("anthropic-test"));
        let config = loader.get("anthropic-test").unwrap();
        assert_eq!(config.model, "claude-test");
    }

    #[test]
    fn test_load_with_body_and_headers() {
        let dir = tempfile::tempdir().unwrap();
        write_test_file(
            dir.path(),
            "anthropic-full.toml",
            r#"
provider = "anthropic"
model = "claude-test"
api_key = "sk-test"
base_url = "https://api.test.com"

[body]
max_tokens = 4096
temperature = 0.5

[headers]
"anthropic-version" = "2023-06-01"
"#,
        );

        let loader = ProviderLoader::load(Some(dir.path()));
        let config = loader.get("anthropic-full").unwrap();
        assert!(config.body.is_some());
        let body = config.body.as_ref().unwrap();
        assert_eq!(body["max_tokens"], 4096);
        assert!(config.headers.is_some());
        let headers = config.headers.as_ref().unwrap();
        assert_eq!(headers["anthropic-version"], "2023-06-01");
    }

    #[test]
    fn test_project_overrides_user() {
        let user_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();

        // User config
        write_test_file(
            user_dir.path(),
            "anthropic-test.toml",
            r#"
provider = "anthropic"
model = "claude-user"
api_key = "sk-user"
base_url = "https://user.api.com"
"#,
        );

        // Project config (overrides user)
        write_test_file(
            project_dir.path(),
            "anthropic-test.toml",
            r#"
provider = "anthropic"
model = "claude-project"
api_key = "sk-project"
base_url = "https://project.api.com"
"#,
        );

        // Set HOME so user dir is found
        std::env::set_var("HOME", user_dir.path());

        let loader = ProviderLoader::load(Some(project_dir.path()));
        assert_eq!(loader.len(), 1);
        let config = loader.get("anthropic-test").unwrap();
        // Project-level should win
        assert_eq!(config.model, "claude-project");

        std::env::remove_var("HOME");
    }

    #[test]
    fn test_load_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let loader = ProviderLoader::load(Some(dir.path()));
        assert!(loader.is_empty());
    }

    #[test]
    fn test_load_nonexistent_dir() {
        let loader = ProviderLoader::load(None);
        assert!(loader.is_empty());
    }
}
