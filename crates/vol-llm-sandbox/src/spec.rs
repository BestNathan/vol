use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Specification for creating a sandbox instance.
///
/// This replaces the old TOML config + bind_metadata approach. The spec contains
/// all information needed to create a sandbox instance, including provider-specific
/// configuration and metadata to bind at creation time.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxSpec {
    /// Profile name (e.g., "coding", "devbox").
    pub name: String,

    /// Provider-specific configuration (includes provider kind as tag).
    #[serde(flatten)]
    pub config: SandboxProviderConfig,

    /// Metadata to bind at creation time (replaces bind_metadata).
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

impl SandboxSpec {
    /// Get the provider kind from the config.
    pub fn provider(&self) -> &str {
        self.config.kind()
    }
}

/// Provider-specific configuration variants.
///
/// Serde internally-tagged enum: the `provider` field selects the variant,
/// and for `Ssh` the SSH-specific fields are flattened into the same TOML
/// table. For `Firecracker` and `Wasm` the provider-specific config is a
/// nested table (e.g. `[firecracker]`) — the variant field name matches
/// the provider tag, so serde places the nested content under that key.
///
/// All variants carry a shared `work_dir` field, since every sandbox
/// needs a working directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum SandboxProviderConfig {
    Local {
        #[serde(default)]
        work_dir: Option<PathBuf>,
    },
    Tmp {
        #[serde(default)]
        work_dir: Option<PathBuf>,
        #[serde(default)]
        sub_dir: Option<String>,
    },
    Ssh {
        #[serde(default)]
        work_dir: PathBuf,
        host: String,
        user: String,
        #[serde(default = "default_port")]
        port: u16,
        #[serde(default)]
        key_path: Option<PathBuf>,
        /// Path to the SSH private key (alias kept for backward-compatible TOML
        /// that used `identity_file` instead of `key_path`). New configs should
        /// prefer `key_path`.
        #[serde(default)]
        identity_file: Option<PathBuf>,
        #[serde(default)]
        passphrase: Option<String>,
        #[serde(default)]
        known_hosts_file: Option<String>,
        #[serde(default)]
        host_key: Option<String>,
        #[serde(default = "default_idle_timeout")]
        idle_timeout_secs: u64,
        #[serde(default = "default_connect_timeout")]
        connect_timeout_secs: u64,
    },
    Firecracker {
        #[serde(default)]
        work_dir: Option<PathBuf>,
        firecracker: FirecrackerConfig,
    },
    Wasm {
        #[serde(default)]
        work_dir: Option<PathBuf>,
        wasm: WasmConfig,
    },
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

impl SandboxProviderConfig {
    /// Get the provider kind as a string.
    pub fn kind(&self) -> &str {
        match self {
            SandboxProviderConfig::Local { .. } => "local",
            SandboxProviderConfig::Tmp { .. } => "tmp",
            SandboxProviderConfig::Ssh { .. } => "ssh",
            SandboxProviderConfig::Firecracker { .. } => "firecracker",
            SandboxProviderConfig::Wasm { .. } => "wasm",
        }
    }

    /// Helper to extract Local config.
    pub fn as_local(&self) -> Option<LocalConfig> {
        match self {
            SandboxProviderConfig::Local { work_dir } => Some(LocalConfig {
                work_dir: work_dir.clone(),
            }),
            _ => None,
        }
    }

    /// Helper to extract Tmp config.
    pub fn as_tmp(&self) -> Option<TmpConfig> {
        match self {
            SandboxProviderConfig::Tmp { sub_dir, .. } => Some(TmpConfig {
                sub_dir: sub_dir.clone(),
            }),
            _ => None,
        }
    }

    /// Helper to extract SSH config.
    pub fn as_ssh(&self) -> Option<SshConfig> {
        match self {
            SandboxProviderConfig::Ssh {
                host,
                user,
                work_dir,
                port,
                key_path,
                identity_file,
                passphrase,
                known_hosts_file,
                host_key,
                idle_timeout_secs,
                connect_timeout_secs,
            } => Some(SshConfig {
                host: host.clone(),
                user: user.clone(),
                work_dir: work_dir.clone(),
                port: *port,
                key_path: key_path.clone().or_else(|| identity_file.clone()),
                passphrase: passphrase.clone(),
                known_hosts_file: known_hosts_file.clone(),
                host_key: host_key.clone(),
                idle_timeout_secs: *idle_timeout_secs,
                connect_timeout_secs: *connect_timeout_secs,
            }),
            _ => None,
        }
    }

    /// Helper to extract Firecracker config.
    pub fn as_firecracker(&self) -> Option<&FirecrackerConfig> {
        match self {
            SandboxProviderConfig::Firecracker { firecracker, .. } => Some(firecracker),
            _ => None,
        }
    }

    /// Helper to extract Wasm config.
    pub fn as_wasm(&self) -> Option<&WasmConfig> {
        match self {
            SandboxProviderConfig::Wasm { wasm, .. } => Some(wasm),
            _ => None,
        }
    }
}

/// Extracted Local config for convenience.
#[derive(Debug, Clone)]
pub struct LocalConfig {
    pub work_dir: Option<PathBuf>,
}

/// Extracted Tmp config for convenience.
#[derive(Debug, Clone)]
pub struct TmpConfig {
    pub sub_dir: Option<String>,
}

/// Extracted SSH config for convenience.
///
/// This is the canonical SSH configuration used by `SSHSandbox` and
/// `SSHSandboxProvider`. It consolidates all SSH-related fields that
/// were previously split across `registry::SshConfig` and the provider
/// enum variant.
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub user: String,
    pub work_dir: PathBuf,
    pub port: u16,
    pub key_path: Option<PathBuf>,
    pub passphrase: Option<String>,
    pub known_hosts_file: Option<String>,
    pub host_key: Option<String>,
    pub idle_timeout_secs: u64,
    pub connect_timeout_secs: u64,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_local_spec() {
        let toml = r#"
            name = "gh-sandbox"
            provider = "local"
            work_dir = "/tmp"
        "#;
        let spec: SandboxSpec = toml::from_str(toml).unwrap();
        assert_eq!(spec.name, "gh-sandbox");
        assert_eq!(spec.provider(), "local");
        assert_eq!(
            spec.config.as_local().unwrap().work_dir,
            Some(PathBuf::from("/tmp"))
        );
    }

    #[test]
    fn parse_ssh_spec_with_key_path() {
        let toml = r#"
            name = "ansible-prod"
            provider = "ssh"
            host = "192.168.2.106"
            port = 22
            user = "root"
            work_dir = "/opt/ansible"
            key_path = "/app/.ssh/id_ed25519"
            host_key = "SHA256:abc"
        "#;
        let spec: SandboxSpec = toml::from_str(toml).unwrap();
        assert_eq!(spec.provider(), "ssh");
        let ssh = spec.config.as_ssh().unwrap();
        assert_eq!(ssh.host, "192.168.2.106");
        assert_eq!(ssh.user, "root");
        assert_eq!(ssh.work_dir, PathBuf::from("/opt/ansible"));
        assert_eq!(ssh.port, 22);
        assert_eq!(
            ssh.key_path.as_deref(),
            Some(std::path::Path::new("/app/.ssh/id_ed25519"))
        );
        assert_eq!(ssh.host_key.as_deref(), Some("SHA256:abc"));
        assert_eq!(ssh.idle_timeout_secs, 300);
        assert_eq!(ssh.connect_timeout_secs, 10);
    }

    #[test]
    fn parse_ssh_spec_with_identity_file_alias() {
        // Legacy configs may use `identity_file` instead of `key_path`.
        let toml = r#"
            name = "legacy-ssh"
            provider = "ssh"
            host = "h"
            user = "u"
            work_dir = "/"
            identity_file = "/home/u/.ssh/id"
        "#;
        let spec: SandboxSpec = toml::from_str(toml).unwrap();
        let ssh = spec.config.as_ssh().unwrap();
        assert_eq!(
            ssh.key_path.as_deref(),
            Some(std::path::Path::new("/home/u/.ssh/id"))
        );
    }

    #[test]
    fn parse_firecracker_spec() {
        let toml = r#"
            name = "fc-sandbox"
            provider = "firecracker"
            work_dir = "/tmp/fc"

            [firecracker]
            kernel_image = "/img/kernel"
            rootfs_image = "/img/rootfs"
            tap_device = "tap0"
            ssh_identity_file = "/id"
        "#;
        let spec: SandboxSpec = toml::from_str(toml).unwrap();
        assert_eq!(spec.provider(), "firecracker");
        let fc = spec.config.as_firecracker().unwrap();
        assert_eq!(fc.kernel_image, "/img/kernel");
        assert_eq!(fc.tap_device, "tap0");
    }

    #[test]
    fn parse_wasm_spec() {
        let toml = r#"
            name = "wasm-sandbox"
            provider = "wasm"
            work_dir = "/tmp/wasm"

            [wasm]
            max_memory_bytes = 1024

            [[wasm.modules]]
            name = "m1"
            path = "/mod/m1.wasm"
        "#;
        let spec: SandboxSpec = toml::from_str(toml).unwrap();
        assert_eq!(spec.provider(), "wasm");
        let wasm = spec.config.as_wasm().unwrap();
        assert_eq!(wasm.max_memory_bytes, 1024);
        assert_eq!(wasm.modules.len(), 1);
        assert_eq!(wasm.modules[0].name, "m1");
    }

    #[test]
    fn extraction_helpers_reject_other_variants() {
        let local = SandboxProviderConfig::Local { work_dir: None };
        assert!(local.as_firecracker().is_none());
        assert!(local.as_wasm().is_none());
        assert!(local.as_ssh().is_none());
        assert!(local.as_tmp().is_none());
        assert!(local.as_local().is_some());
    }

    #[test]
    fn kind_matches_provider_tag_for_every_variant() {
        let cases: Vec<(SandboxProviderConfig, &str)> = vec![
            (SandboxProviderConfig::Local { work_dir: None }, "local"),
            (
                SandboxProviderConfig::Tmp {
                    work_dir: None,
                    sub_dir: None,
                },
                "tmp",
            ),
            (
                SandboxProviderConfig::Ssh {
                    work_dir: PathBuf::from("/"),
                    host: "h".to_string(),
                    user: "u".to_string(),
                    port: 22,
                    key_path: None,
                    identity_file: None,
                    passphrase: None,
                    known_hosts_file: None,
                    host_key: None,
                    idle_timeout_secs: 300,
                    connect_timeout_secs: 10,
                },
                "ssh",
            ),
        ];
        for (config, expected) in cases {
            assert_eq!(config.kind(), expected);
        }
    }
}
