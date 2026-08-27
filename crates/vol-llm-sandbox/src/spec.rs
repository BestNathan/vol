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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum SandboxProviderConfig {
    Local {
        #[serde(default)]
        work_dir: Option<PathBuf>,
    },
    Tmp {
        #[serde(default)]
        sub_dir: Option<String>,
    },
    Ssh {
        host: String,
        user: String,
        work_dir: PathBuf,
        #[serde(default)]
        port: Option<u16>,
        #[serde(default)]
        key_path: Option<PathBuf>,
    },
}

impl SandboxProviderConfig {
    /// Get the provider kind as a string.
    pub fn kind(&self) -> &str {
        match self {
            SandboxProviderConfig::Local { .. } => "local",
            SandboxProviderConfig::Tmp { .. } => "tmp",
            SandboxProviderConfig::Ssh { .. } => "ssh",
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
            SandboxProviderConfig::Tmp { sub_dir } => Some(TmpConfig {
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
            } => Some(SshConfig {
                host: host.clone(),
                user: user.clone(),
                work_dir: work_dir.clone(),
                port: *port,
                key_path: key_path.clone(),
            }),
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
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub user: String,
    pub work_dir: PathBuf,
    pub port: Option<u16>,
    pub key_path: Option<PathBuf>,
}
