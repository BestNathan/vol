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
    /// Path to the uncompressed ELF kernel image (vmlinux).
    pub kernel_image: String,
    /// Path to the ext4 rootfs image.
    pub rootfs_image: String,
    /// If true, mount rootfs read-only (writes go to tmpfs overlay).
    #[serde(default)]
    pub rootfs_readonly: bool,
    /// Number of pre-warmed idle microVMs in the pool.
    #[serde(default = "default_pool_size")]
    pub pool_size: usize,
    /// Seconds before an idle VM is reclaimed.
    #[serde(default = "default_idle_timeout_fc")]
    pub idle_timeout_secs: u64,
    /// Seconds to wait for SSH to become available in the guest.
    #[serde(default = "default_connect_timeout_fc")]
    pub connect_timeout_secs: u64,
    /// Path to firecracker binary. If unset, looks up "firecracker" on PATH.
    #[serde(default)]
    pub firecracker_binary: Option<String>,
    /// IP address assigned to the guest VM for SSH access.
    #[serde(default = "default_guest_ip")]
    pub guest_ip: String,
    /// SSH port inside the guest VM.
    #[serde(default = "default_guest_ssh_port")]
    pub guest_ssh_port: u16,
    /// Host tap device name for the VM's network interface.
    pub tap_device: String,
    /// Path to an SSH keypair for guest access (the private key).
    /// The corresponding public key must be in the guest's ~/.ssh/authorized_keys.
    pub ssh_identity_file: String,
    /// Optional passphrase for the SSH identity file.
    #[serde(default)]
    pub ssh_passphrase: Option<String>,
}

/// Configuration for a Wasm sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmConfig {
    /// Max linear memory per module instance, in bytes.
    #[serde(default = "default_wasm_memory")]
    pub max_memory_bytes: u64,
    /// Per-execution timeout in milliseconds.
    #[serde(default = "default_wasm_timeout")]
    pub max_execution_ms: u64,
    /// Wasm modules to precompile and serve.
    #[serde(default)]
    pub modules: Vec<WasmModuleConfig>,
}

/// A single Wasm module registered in the sandbox.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WasmModuleConfig {
    /// Logical name — used as `program` in `CommandRequest`.
    pub name: String,
    /// Path to the `.wasm` file on disk.
    pub path: String,
    /// If true, register this module as a named agent tool.
    #[serde(default)]
    pub expose_as_tool: bool,
}

fn default_wasm_memory() -> u64 {
    134_217_728
} // 128 MB
fn default_wasm_timeout() -> u64 {
    30_000
} // 30 seconds

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

/// Registry of named sandbox instances.
/// Always contains a built-in "local" sandbox. Additional sandboxes
/// are loaded from TOML config files.
pub struct SandboxRegistry {
    sandboxes: HashMap<String, Arc<dyn Sandbox>>,
    default_name: String,
    /// Pool-based sandbox factories. acquire() creates fresh instances from these.
    #[cfg(feature = "firecracker")]
    firecracker_pools: HashMap<String, Arc<crate::firecracker::FirecrackerPool>>,
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

        #[cfg(feature = "firecracker")]
        #[allow(unused_mut)]
        let mut firecracker_pools: HashMap<
            String,
            Arc<crate::firecracker::FirecrackerPool>,
        > = HashMap::new();

        // Load *.toml files — individual failures are non-fatal
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
                if !path.extension().is_some_and(|ext| ext == "toml") {
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

                if config.name == "local" {
                    tracing::warn!(path = %path.display(), name = %config.name, "Sandbox name 'local' is reserved, skipping");
                    continue;
                }
                if sandboxes.contains_key(&config.name) {
                    tracing::warn!(path = %path.display(), name = %config.name, "Duplicate sandbox name, skipping");
                    continue;
                }

                match config.sandbox_type.as_str() {
                    "local" => {
                        let sandbox: Arc<dyn Sandbox> =
                            Arc::new(LocalSandbox::new(
                                config.work_dir.as_ref().map(std::path::PathBuf::from),
                            ));
                        sandboxes.insert(config.name.clone(), sandbox);
                    }
                    #[cfg(feature = "ssh")]
                    "ssh" => {
                        let ssh_config = match config.ssh {
                            Some(c) => c,
                            None => {
                                tracing::warn!(name = %config.name, "SSH sandbox requires [sandbox.ssh] section, skipping");
                                continue;
                            }
                        };
                        let sb = match crate::ssh::SSHSandbox::new(
                            config.name.clone(),
                            config.work_dir.clone(),
                            ssh_config,
                        ) {
                            Ok(s) => s,
                            Err(e) => {
                                tracing::warn!(name = %config.name, error = %e, "Failed to create SSH sandbox, skipping");
                                continue;
                            }
                        };
                        let sandbox: Arc<dyn Sandbox> = Arc::new(sb);
                        if let Err(e) = sandbox.start().await {
                            tracing::warn!(name = %config.name, error = %e, "Failed to start sandbox, skipping");
                            continue;
                        }
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
                                "Firecracker sandbox '{}' requires Linux/KVM — skipping registration",
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
                        let sandbox: Arc<dyn Sandbox> = Arc::new(sb);
                        sandboxes.insert(config.name.clone(), sandbox);
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
            default_name: "local".to_string(),
            #[cfg(feature = "firecracker")]
            firecracker_pools,
        })
    }

    /// Get a sandbox by its registry name.
    pub fn get(&self, name: &str) -> Option<Arc<dyn Sandbox>> {
        self.sandboxes.get(name).cloned()
    }

    /// Acquire a sandbox instance by name.
    ///
    /// For pool-based sandboxes (firecracker), creates a fresh instance
    /// backed by a VM from the pool. For singletons (local, ssh, wasm),
    /// returns a clone of the shared Arc.
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

    /// Get the default sandbox (always "local").
    pub fn default(&self) -> Arc<dyn Sandbox> {
        self.sandboxes
            .get(&self.default_name)
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
    async fn test_registry_skips_local_override() {
        let tmp = std::env::temp_dir().join("sandbox_test_local_override2");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config = r#"
name = "local"
type = "local"
work_dir = "/tmp"
"#;
        std::fs::write(tmp.join("local.toml"), config).unwrap();

        let result = SandboxRegistry::load(&tmp).await;
        assert!(result.is_ok());
        let registry = result.unwrap();
        assert!(registry.get("local").is_some());
        assert_eq!(registry.len(), 1);

        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[tokio::test]
    async fn test_registry_skips_unknown_type() {
        let tmp = std::env::temp_dir().join("sandbox_test_unknown_type");
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();

        let config = r#"
name = "bad"
type = "nonexistent"
"#;
        std::fs::write(tmp.join("bad.toml"), config).unwrap();

        let result = SandboxRegistry::load(&tmp).await;
        assert!(result.is_ok());
        let registry = result.unwrap();
        assert!(registry.get("local").is_some());
        assert_eq!(registry.len(), 1);

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

    #[tokio::test]
    async fn load_skips_invalid_sandbox_keeps_valid() {
        let tmp = tempfile::tempdir().unwrap();
        // valid sandbox
        std::fs::write(
            tmp.path().join("good.toml"),
            r#"
name = "good"
type = "local"
"#,
        )
        .unwrap();
        // invalid sandbox (bad TOML syntax — missing `=`)
        std::fs::write(
            tmp.path().join("bad.toml"),
            r#"name "bad""#,
        )
        .unwrap();
        // duplicate name with good — should be skipped (warn)
        std::fs::write(
            tmp.path().join("dup.toml"),
            r#"
name = "good"
type = "local"
"#,
        )
        .unwrap();

        let registry = SandboxRegistry::load(tmp.path()).await.unwrap();
        // "good" is present once, "local" is always present
        assert!(registry.get("local").is_some(), "local must always exist");
        assert!(registry.get("good").is_some(), "good must be loaded");
        // "good" is not duplicated
        assert_eq!(
            registry.names().iter().filter(|n| **n == "good").count(),
            1
        );
    }
}
