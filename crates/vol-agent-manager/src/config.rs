//! Configuration for vol-agent-manager.

use serde::{Deserialize, Serialize};

/// Top-level configuration.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ManagerConfig {
    pub server: ServerConfig,
    pub health: HealthConfig,
    pub security: SecurityConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    /// Listen address, e.g. "0.0.0.0:8080"
    pub listen_addr: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HealthConfig {
    /// How often to check heartbeats (seconds)
    pub check_interval_secs: u64,
    /// Heartbeat timeout threshold (seconds)
    pub heartbeat_timeout_secs: u64,
    /// How long to retain disconnected agent state (seconds)
    pub disconnect_retention_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    /// Optional token for WebSocket and REST auth
    pub token: Option<String>,
}

impl Default for ManagerConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                listen_addr: "0.0.0.0:8080".to_string(),
            },
            health: HealthConfig {
                check_interval_secs: 15,
                heartbeat_timeout_secs: 90,
                disconnect_retention_secs: 300,
            },
            security: SecurityConfig { token: None },
        }
    }
}

impl ManagerConfig {
    /// Load from a TOML file path.
    pub fn from_path(path: &str) -> Result<Self, anyhow::Error> {
        let content = std::fs::read_to_string(path)?;
        let config: ManagerConfig = toml::from_str(&content)?;
        Ok(config)
    }
}
