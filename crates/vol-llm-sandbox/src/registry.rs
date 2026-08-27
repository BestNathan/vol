use crate::local::LocalSandbox;
use crate::{Sandbox, SandboxError, SandboxResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

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
    #[serde(default)]
    pub firecracker: Option<FirecrackerConfig>,
    #[serde(default)]
    pub wasm: Option<WasmConfig>,
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

fn default_port() -> u16 {
    22
}
fn default_idle_timeout() -> u64 {
    300
}
fn default_connect_timeout() -> u64 {
    10
}

/// Configuration for a Firecracker microVM sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FirecrackerConfig {
    pub kernel_image: String,
    pub rootfs_image: String,
    #[serde(default)]
    pub rootfs_readonly: bool,
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    #[serde(default = "default_idle_timeout_fc")]
    pub idle_timeout_secs: u64,
    #[serde(default = "default_connect_timeout_fc")]
    pub connect_timeout_secs: u64,
    #[serde(default)]
    pub firecracker_binary: Option<String>,
    #[serde(default = "default_guest_ip")]
    pub guest_ip: String,
    #[serde(default = "default_guest_ssh_port")]
    pub guest_ssh_port: u16,
    pub tap_device: String,
    pub ssh_identity_file: String,
    #[serde(default)]
    pub ssh_passphrase: Option<String>,
}

/// Configuration for a Wasm sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmConfig {
    #[serde(default = "default_wasm_memory")]
    pub max_memory_bytes: u64,
    #[serde(default = "default_wasm_timeout")]
    pub max_execution_ms: u64,
    #[serde(default)]
    pub modules: Vec<WasmModuleConfig>,
}

/// A single Wasm module registered in the sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmModuleConfig {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub expose_as_tool: bool,
}

fn default_wasm_memory() -> u64 {
    134_217_728
}
fn default_wasm_timeout() -> u64 {
    30_000
}
fn default_pool_size() -> usize {
    1
}
fn default_idle_timeout_fc() -> u64 {
    300
}
fn default_connect_timeout_fc() -> u64 {
    10
}
fn default_guest_ip() -> String {
    "172.16.0.2".to_string()
}
fn default_guest_ssh_port() -> u16 {
    22
}

/// Registry of named sandbox instances loaded from TOML config files.
///
/// No built-in entries. Use [`register`] to add programmatic sandboxes
/// (e.g. a "local" [`LocalSandbox`] at the server's working directory).
/// [`default`] returns a fresh [`TmpSandbox`] as the fallback.
pub struct SandboxRegistry {
    sandboxes: HashMap<String, Arc<dyn Sandbox>>,
    #[cfg(feature = "firecracker")]
    firecracker_pools: HashMap<String, Arc<crate::firecracker::FirecrackerPool>>,
}

impl SandboxRegistry {
    /// Construct a single sandbox from a parsed config.
    pub async fn build_sandbox(config: SandboxConfig) -> SandboxResult<Arc<dyn Sandbox>> {
        let sandbox: Arc<dyn Sandbox> = match config.sandbox_type.as_str() {
            "local" => Arc::new(LocalSandbox::new(
                config.work_dir.as_ref().map(std::path::PathBuf::from),
            )),
            "tmp" => Arc::new(crate::tmp::TmpSandbox::new()),
            #[cfg(feature = "ssh")]
            "ssh" => {
                let ssh_config = config.ssh.ok_or_else(|| {
                    SandboxError::Config(format!(
                        "SSH sandbox '{}' requires [ssh] section",
                        config.name
                    ))
                })?;
                let sb = crate::ssh::SSHSandbox::new(
                    config.name.clone(),
                    config.work_dir.clone(),
                    ssh_config,
                )?;
                let sandbox: Arc<dyn Sandbox> = Arc::new(sb);
                sandbox
            }
            other => {
                return Err(SandboxError::Config(format!(
                    "unsupported sandbox type: {other}"
                )));
            }
        };
        Ok(sandbox)
    }

    /// Load sandboxes from a config directory.
    ///
    /// Reads `*.toml` files from `sandboxes_dir`. Individual failures are
    /// logged and skipped. No built-in entries — use [`register`] afterwards
    /// to add programmatic sandboxes.
    pub async fn load(sandboxes_dir: &Path) -> SandboxResult<Self> {
        let mut sandboxes: HashMap<String, Arc<dyn Sandbox>> = HashMap::new();

        #[cfg(feature = "firecracker")]
        #[allow(unused_mut)]
        let mut firecracker_pools: HashMap<
            String,
            Arc<crate::firecracker::FirecrackerPool>,
        > = HashMap::new();

        if sandboxes_dir.exists() {
            for entry in std::fs::read_dir(sandboxes_dir).map_err(SandboxError::Io)? {
                let entry = match entry {
                    Ok(e) => e,
                    Err(e) => {
                        tracing::warn!("Failed to read sandbox directory entry: {}", e);
                        continue;
                    }
                };
                let path = entry.path();
                if path.extension().is_none_or(|ext| ext != "toml") {
                    continue;
                }
                let content = match std::fs::read_to_string(&path) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(path = %path.display(), error = %e, "Failed to read sandbox config, skipping");
                        continue;
                    }
                };
                let config: SandboxConfig = match toml::from_str(&content) {
                    Ok(c) => c,
                    Err(e) => {
                        tracing::warn!(path = %path.display(), error = %e, "Failed to parse sandbox config, skipping");
                        continue;
                    }
                };
                if sandboxes.contains_key(&config.name) {
                    tracing::warn!(name = %config.name, "Duplicate sandbox name, skipping");
                    continue;
                }
                match config.sandbox_type.as_str() {
                    "local" | "ssh" | "tmp" => {
                        let sandbox = match Self::build_sandbox(config.clone()).await {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(path = %path.display(), error = %e, "Failed to build sandbox, skipping");
                                continue;
                            }
                        };
                        sandboxes.insert(config.name.clone(), sandbox);
                    }
                    #[cfg(feature = "firecracker")]
                    "firecracker" => {
                        let fc_config = match config.firecracker {
                            Some(c) => c,
                            None => {
                                tracing::warn!(name = %config.name, "Firecracker sandbox requires [sandbox.firecracker] section, skipping");
                                continue;
                            }
                        };
                        #[cfg(target_os = "linux")]
                        {
                            let pool = crate::firecracker::FirecrackerPool::new(
                                fc_config.clone(),
                                tokio::runtime::Handle::current(),
                            );
                            let sandbox: Arc<dyn Sandbox> =
                                Arc::new(crate::firecracker::FirecrackerSandbox::new(
                                    config.name.clone(),
                                    std::path::PathBuf::from(
                                        config.work_dir.as_deref().unwrap_or("/tmp/fc-sandbox"),
                                    ),
                                    pool.clone(),
                                ));
                            sandboxes.insert(config.name.clone(), sandbox);
                            firecracker_pools.insert(config.name.clone(), pool);
                        }
                        #[cfg(not(target_os = "linux"))]
                        {
                            let _ = fc_config;
                            tracing::warn!(
                                "Firecracker requires Linux/KVM, skipping '{}'",
                                config.name
                            );
                        }
                    }
                    #[cfg(feature = "wasm")]
                    "wasm" => {
                        let wasm_config = match config.wasm {
                            Some(c) => c,
                            None => {
                                tracing::warn!(name = %config.name, "Wasm sandbox requires [wasm] section, skipping");
                                continue;
                            }
                        };
                        let sb = match crate::wasm::WasmSandbox::new(
                            config.name.clone(),
                            std::path::PathBuf::from(
                                config.work_dir.as_deref().unwrap_or("/tmp/wasm-sandbox"),
                            ),
                            wasm_config,
                        ) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(name = %config.name, error = %e, "Failed to create Wasm sandbox, skipping");
                                continue;
                            }
                        };
                        sandboxes.insert(config.name.clone(), Arc::new(sb) as Arc<dyn Sandbox>);
                    }
                    other => {
                        tracing::warn!(name = %config.name, sandbox_type = %other, "Unknown sandbox type, skipping");
                        continue;
                    }
                }
            }
        }

        Ok(Self {
            sandboxes,
            #[cfg(feature = "firecracker")]
            firecracker_pools,
        })
    }

    /// Register a sandbox instance by name.
    pub fn register(&mut self, name: &str, sandbox: Arc<dyn Sandbox>) {
        self.sandboxes.insert(name.to_string(), sandbox);
    }

    /// Get a sandbox by name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
        self.sandboxes.get(name).cloned()
    }

    /// Acquire a sandbox instance by name (pure lookup).
    ///
    /// For pool-based sandboxes (firecracker), creates a fresh instance.
    /// For all others, returns a clone of the shared Arc.
    pub fn acquire(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
        #[cfg(feature = "firecracker")]
        {
            if let Some(pool) = self.firecracker_pools.get(name) {
                let work_dir = self
                    .sandboxes
                    .get(name)
                    .map(|sb| sb.root_path().to_path_buf())
                    .unwrap_or_else(|| std::path::PathBuf::from("/tmp/fc-sandbox"));
                return Some(Arc::new(crate::firecracker::FirecrackerSandbox::new(
                    name.to_string(),
                    work_dir,
                    pool.clone(),
                )));
            }
        }
        self.sandboxes.get(name).cloned()
    }

    /// The default fallback sandbox — a fresh [`TmpSandbox`] with a random subdir.
    pub fn default(&self) -> Arc<dyn Sandbox> {
        Arc::new(crate::tmp::TmpSandbox::new())
    }

    /// Number of registered sandboxes.
    pub fn len(&self) -> usize {
        self.sandboxes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sandboxes.is_empty()
    }

    pub fn names(&self) -> Vec<&str> {
        self.sandboxes
            .keys()
            .map(std::string::String::as_str)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_empty_default_is_tmp() {
        let tmp = std::env::temp_dir().join("sandbox_test_empty");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let registry = SandboxRegistry::load(&tmp).await.unwrap();
        assert!(registry.is_empty());
        // Default is always a fresh TmpSandbox
        let default = registry.default();
        assert_eq!(default.kind(), "tmp");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_register_and_acquire() {
        let tmp = std::env::temp_dir().join("sandbox_test_register");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let mut registry = SandboxRegistry::load(&tmp).await.unwrap();
        registry.register("local", Arc::new(LocalSandbox::new(Some(tmp.join("work")))));

        let acquired = registry.acquire("local").unwrap();
        assert_eq!(acquired.kind(), "local");
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_load_toml() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("dev.toml"),
            r#"
name = "dev"
type = "tmp"
"#,
        )
        .unwrap();

        let registry = SandboxRegistry::load(tmp.path()).await.unwrap();
        assert!(registry.get("dev").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[tokio::test]
    async fn test_registry_skips_invalid() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("bad.toml"), "not valid {{{").unwrap();

        let registry = SandboxRegistry::load(tmp.path()).await.unwrap();
        assert!(registry.is_empty());
    }
}
