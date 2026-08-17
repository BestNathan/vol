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
#[derive(Debug, Clone, Default)]
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
        self.providers
            .keys()
            .map(std::string::String::as_str)
            .collect()
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
                    .map(std::string::ToString::to_string);
                let Some(id) = id else { continue };

                match std::fs::read_to_string(&path) {
                    Ok(content) => match toml::from_str::<ProviderFileConfig>(&content) {
                        Ok(config) => {
                            map.insert(id, config);
                        }
                        Err(e) => {
                            tracing::warn!(
                                "Failed to parse provider config '{}': {}",
                                path.display(),
                                e
                            );
                        }
                    },
                    Err(e) => {
                        tracing::warn!(
                            "Failed to read provider config '{}': {}",
                            path.display(),
                            e
                        );
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

    /// Serializes tests that read or mutate `$HOME` (`load_user_dir` / HOME
    /// mutation) so parallel test execution cannot observe each other's env.
    /// Poisoning is recovered from so a panicking test does not cascade.
    static HOME_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn home_lock() -> std::sync::MutexGuard<'static, ()> {
        HOME_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn write_test_file(dir: &Path, name: &str, content: &str) {
        std::fs::create_dir_all(dir.join(PROVIDERS_DIR)).unwrap();
        let mut file = std::fs::File::create(dir.join(PROVIDERS_DIR).join(name)).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_single_provider() {
        let _guard = home_lock();
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
        let _guard = home_lock();
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
        let _guard = home_lock();
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
        let _guard = home_lock();
        let dir = tempfile::tempdir().unwrap();
        let loader = ProviderLoader::load(Some(dir.path()));
        assert!(loader.is_empty());
    }

    #[test]
    fn test_load_nonexistent_dir() {
        let _guard = home_lock();
        let loader = ProviderLoader::load(None);
        assert!(loader.is_empty());
    }

    #[test]
    fn test_insert_get_ids_and_to_provider_configs() {
        let mut loader = ProviderLoader::default();
        assert!(loader.is_empty());

        loader.insert(
            "provider-a",
            ProviderFileConfig {
                provider: vol_llm_core::LLMProvider::Anthropic,
                model: "claude-test".to_string(),
                api_key: crate::secret::Secret::literal("sk-a"),
                base_url: "https://a.test".to_string(),
                body: None,
                headers: None,
            },
        );
        loader.insert(
            "provider-b",
            ProviderFileConfig {
                provider: vol_llm_core::LLMProvider::OpenAI,
                model: "gpt-4o".to_string(),
                api_key: crate::secret::Secret::literal("sk-b"),
                base_url: "https://b.test".to_string(),
                body: None,
                headers: None,
            },
        );

        assert!(!loader.is_empty());
        assert_eq!(loader.len(), 2);
        assert!(loader.contains("provider-a"));
        assert!(loader.contains("provider-b"));
        assert!(!loader.contains("missing"));

        let mut ids = loader.ids();
        ids.sort();
        assert_eq!(ids, vec!["provider-a", "provider-b"]);

        assert_eq!(loader.get("provider-a").unwrap().model, "claude-test");
        assert!(loader.get("missing").is_none());

        let configs = loader.to_provider_configs();
        assert_eq!(configs.len(), 2);
        let mut by_id: HashMap<_, _> = configs
            .into_iter()
            .map(|named| (named.id, named.config))
            .collect();
        let b = by_id.remove("provider-b").unwrap();
        assert_eq!(b.model, "gpt-4o");
        assert_eq!(
            by_id.remove("provider-a").unwrap().base_url,
            "https://a.test"
        );
    }

    #[test]
    fn test_invalid_toml_file_is_skipped() {
        let _guard = home_lock();
        let dir = tempfile::tempdir().unwrap();
        write_test_file(
            dir.path(),
            "broken.toml",
            "provider = \"anthropic\"\nmodel = [unclosed\n",
        );

        let loader = ProviderLoader::load(Some(dir.path()));
        assert!(
            !loader.contains("broken"),
            "invalid TOML must be skipped with a warning"
        );
    }

    #[test]
    fn test_unreadable_file_is_skipped() {
        let _guard = home_lock();
        // A directory named like a TOML file cannot be read as a file.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(PROVIDERS_DIR)).unwrap();
        std::fs::create_dir(dir.path().join(PROVIDERS_DIR).join("broken.toml")).unwrap();

        let loader = ProviderLoader::load(Some(dir.path()));
        assert!(
            !loader.contains("broken"),
            "unreadable entry must be skipped"
        );
    }

    #[test]
    fn test_non_toml_files_are_ignored() {
        let _guard = home_lock();
        let dir = tempfile::tempdir().unwrap();
        write_test_file(dir.path(), "notes.txt", "provider = \"anthropic\"");
        write_test_file(dir.path(), "README.md", "# docs");

        let loader = ProviderLoader::load(Some(dir.path()));
        assert!(!loader.contains("notes"), "non-.toml files must be ignored");
        assert!(!loader.contains("README"));
    }
}
