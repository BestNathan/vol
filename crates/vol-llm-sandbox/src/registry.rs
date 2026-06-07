use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::{Sandbox, SandboxError, SandboxResult};
use crate::local::LocalSandbox;

/// Deserialized from `.agent/sandboxes/*.toml` files.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SandboxConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub sandbox_type: String,
    #[serde(default)]
    pub work_dir: Option<String>,
    #[serde(default)]
    pub ssh: Option<SshConfig>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SshConfig {
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    pub user: String,
    pub identity_file: String,
    #[serde(default)]
    pub passphrase: Option<String>,
    #[serde(default)]
    pub known_hosts_file: Option<String>,
    #[serde(default)]
    pub host_key: Option<String>,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_connect_timeout")]
    pub connect_timeout_secs: u64,
}

fn default_port() -> u16 { 22 }
fn default_idle_timeout() -> u64 { 300 }
fn default_connect_timeout() -> u64 { 10 }

/// Registry of named sandbox instances.
/// Always contains a built-in "local" sandbox. Additional sandboxes
/// are loaded from TOML config files.
pub struct SandboxRegistry {
    sandboxes: HashMap<String, Arc<dyn Sandbox>>,
    default_name: String,
}

impl SandboxRegistry {
    /// Load sandboxes from a config directory.
    ///
    /// Always registers a built-in `LocalSandbox` named "local".
    /// Additional sandboxes are loaded from `*.toml` files in `sandboxes_dir`.
    pub async fn load(sandboxes_dir: &Path) -> SandboxResult<Self> {
        let mut sandboxes: HashMap<String, Arc<dyn Sandbox>> = HashMap::new();

        // Always register LocalSandbox (hardcoded, no config file needed)
        let local = Arc::new(LocalSandbox::new(None)) as Arc<dyn Sandbox>;
        sandboxes.insert("local".to_string(), local);

        // Load *.toml files
        if sandboxes_dir.exists() {
            for entry in std::fs::read_dir(sandboxes_dir).map_err(SandboxError::Io)? {
                let entry = entry.map_err(SandboxError::Io)?;
                let path = entry.path();
                if path.extension().is_some_and(|ext| ext == "toml") {
                    let content = std::fs::read_to_string(&path).map_err(SandboxError::Io)?;
                    let config: SandboxConfig = toml::from_str(&content)
                        .map_err(|e| SandboxError::UnknownType(format!(
                            "failed to parse {}: {}", path.display(), e
                        )))?;

                    if config.name == "local" {
                        return Err(SandboxError::LocalOverride);
                    }
                    if sandboxes.contains_key(&config.name) {
                        return Err(SandboxError::DuplicateName(config.name.clone()));
                    }

                    match config.sandbox_type.as_str() {
                        #[cfg(feature = "ssh")]
                        "ssh" => {
                            let ssh_config = config.ssh.ok_or_else(|| {
                                SandboxError::UnknownType(
                                    "SSH sandbox requires [sandbox.ssh] section".to_string()
                                )
                            })?;
                            let sb = crate::ssh::SSHSandbox::new(
                                config.name.clone(),
                                config.work_dir.clone(),
                                ssh_config,
                            )?;
                            let sandbox: Arc<dyn Sandbox> = Arc::new(sb);
                            sandbox.start().await?;
                            sandboxes.insert(config.name.clone(), sandbox);
                        }
                        other => return Err(SandboxError::UnknownType(other.to_string())),
                    }
                }
            }
        }

        Ok(Self { sandboxes, default_name: "local".to_string() })
    }

    /// Get a sandbox by its registry name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
        self.sandboxes.get(name).cloned()
    }

    /// Get the default sandbox (always "local").
    pub fn default(&self) -> Arc<dyn Sandbox> {
        self.sandboxes.get(&self.default_name)
            .cloned()
            .expect("LocalSandbox always present")
    }

    /// Number of registered sandboxes.
    pub fn len(&self) -> usize {
        self.sandboxes.len()
    }

    /// Check if any sandboxes are registered.
    pub fn is_empty(&self) -> bool {
        self.sandboxes.is_empty()
    }

    /// Names of all registered sandboxes.
    pub fn names(&self) -> Vec<&str> {
        self.sandboxes.keys().map(|s| s.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_always_has_local() {
        let tmp = std::env::temp_dir().join("sandbox_test_empty_registry");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let registry = SandboxRegistry::load(&tmp).await.unwrap();
        assert!(registry.get("local").is_some());
        assert_eq!(registry.default().name(), "local");
        assert_eq!(registry.default().kind(), "local");
        assert_eq!(registry.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_rejects_local_override() {
        let tmp = std::env::temp_dir().join("sandbox_test_local_override2");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config = r#"
name = "local"
type = "ssh"
work_dir = "/tmp"
"#;
        std::fs::write(tmp.join("bad.toml"), config).unwrap();

        let result = SandboxRegistry::load(&tmp).await;
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_unknown_type() {
        let tmp = std::env::temp_dir().join("sandbox_test_unknown_type");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config = r#"
name = "bad"
type = "nonexistent"
"#;
        std::fs::write(tmp.join("bad.toml"), config).unwrap();

        let result = SandboxRegistry::load(&tmp).await;
        assert!(result.is_err());

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_names() {
        let tmp = std::env::temp_dir().join("sandbox_test_names");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let registry = SandboxRegistry::load(&tmp).await.unwrap();
        let names = registry.names();
        assert!(names.contains(&"local"));
        assert_eq!(names.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }
}
