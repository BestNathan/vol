//! Server configuration via TOML.
//!
//! Loads from `~/.vol/agent-server.toml` by default, or from `--config <path>`.

use serde::Deserialize;
use std::path::PathBuf;

/// Top-level server configuration.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ServerConfig {
    #[serde(default)]
    pub server: ServerSection,
    #[serde(default)]
    pub control_plane: ControlPlaneSection,
    #[serde(default)]
    pub data_plane: DataPlaneSection,
    #[serde(default)]
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub tracing: TracingSection,
    #[serde(default)]
    pub opentelemetry: OpenTelemetrySection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default)]
    pub roles: ServerRoles,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerRoles {
    #[serde(default)]
    pub control_plane: bool,
    #[serde(default = "default_data_plane_role")]
    pub data_plane: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ControlPlaneSection {
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default = "default_client_ws_path")]
    pub client_ws_path: String,
    #[serde(default = "default_node_ws_path")]
    pub node_ws_path: String,
    #[serde(default = "default_lease_timeout_secs")]
    pub lease_timeout_secs: u64,
    #[serde(default = "default_lease_scan_secs")]
    pub lease_scan_secs: u64,
    #[serde(default)]
    pub node_ingress: std::collections::HashMap<String, String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DataPlaneSection {
    #[serde(default)]
    pub node_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub control_url: Option<String>,
    #[serde(default)]
    pub auth_token: Option<String>,
    #[serde(default = "default_heartbeat_secs")]
    pub heartbeat_secs: u64,
    #[serde(default = "default_snapshot_on_connect")]
    pub snapshot_on_connect: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSection {
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
    #[serde(default = "default_store_dir")]
    pub store_dir: String,
    #[serde(default)]
    pub task_store: Option<vol_llm_runtime::TaskStoreConfig>,
    #[serde(default)]
    pub session_store: Option<vol_llm_runtime::SessionStoreConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TracingSection {
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default = "default_format")]
    pub format: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpenTelemetrySection {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_otel_endpoint")]
    pub endpoint: String,
    #[serde(default = "default_otel_service_name")]
    pub service_name: String,
    #[serde(default = "default_otel_service_namespace")]
    pub service_namespace: String,
    #[serde(default = "default_otel_deployment_env")]
    pub deployment_environment: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,
    #[serde(default = "default_max_export_timeout_millis")]
    pub batch_max_export_timeout_millis: u64,
}

// --- Defaults ---

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3001
}

fn default_data_plane_role() -> bool {
    true
}

fn default_client_ws_path() -> String {
    "/ws".to_string()
}

fn default_node_ws_path() -> String {
    "/control/v1/ws".to_string()
}

fn default_lease_timeout_secs() -> u64 {
    90
}

fn default_lease_scan_secs() -> u64 {
    15
}

fn default_heartbeat_secs() -> u64 {
    15
}

fn default_snapshot_on_connect() -> bool {
    true
}

fn default_working_dir() -> String {
    ".".to_string()
}

fn default_store_dir() -> String {
    "~/.vol".to_string()
}

fn default_level() -> String {
    "info".to_string()
}

fn default_format() -> String {
    "text".to_string()
}

fn default_otel_endpoint() -> String {
    "http://otel-collector.observability.svc.cluster.local:4317".to_string()
}

fn default_otel_service_name() -> String {
    "agent-server".to_string()
}

fn default_otel_service_namespace() -> String {
    "vol-agent".to_string()
}

fn default_otel_deployment_env() -> String {
    "production".to_string()
}

fn default_sample_rate() -> f64 {
    1.0
}

fn default_max_export_timeout_millis() -> u64 {
    5000
}

// --- Default trait implementations ---

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            roles: ServerRoles::default(),
        }
    }
}

impl Default for ServerRoles {
    fn default() -> Self {
        Self {
            control_plane: false,
            data_plane: default_data_plane_role(),
        }
    }
}

impl Default for ControlPlaneSection {
    fn default() -> Self {
        Self {
            auth_token: None,
            client_ws_path: default_client_ws_path(),
            node_ws_path: default_node_ws_path(),
            lease_timeout_secs: default_lease_timeout_secs(),
            lease_scan_secs: default_lease_scan_secs(),
            node_ingress: std::collections::HashMap::new(),
        }
    }
}

impl Default for DataPlaneSection {
    fn default() -> Self {
        Self {
            node_id: None,
            name: None,
            control_url: None,
            auth_token: None,
            heartbeat_secs: default_heartbeat_secs(),
            snapshot_on_connect: default_snapshot_on_connect(),
        }
    }
}

impl Default for RuntimeSection {
    fn default() -> Self {
        Self {
            working_dir: default_working_dir(),
            store_dir: default_store_dir(),
            task_store: None,
            session_store: None,
        }
    }
}

impl Default for TracingSection {
    fn default() -> Self {
        Self {
            level: default_level(),
            format: default_format(),
        }
    }
}

impl Default for OpenTelemetrySection {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: default_otel_endpoint(),
            service_name: default_otel_service_name(),
            service_namespace: default_otel_service_namespace(),
            deployment_environment: default_otel_deployment_env(),
            sample_rate: default_sample_rate(),
            batch_max_export_timeout_millis: default_max_export_timeout_millis(),
        }
    }
}

// --- Load ---

impl ServerConfig {
    /// Load config from a TOML file path.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {path:?}: {e}"))?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config {path:?}: {e}"))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if !self.server.roles.control_plane && !self.server.roles.data_plane {
            return Err("at least one server role must be enabled".to_string());
        }

        if self.server.roles.control_plane && self.control_plane.client_ws_path == "/health" {
            return Err("control_plane.client_ws_path must not equal /health".to_string());
        }
        if self.server.roles.control_plane && self.control_plane.node_ws_path == "/health" {
            return Err("control_plane.node_ws_path must not equal /health".to_string());
        }
        if !self.server.roles.control_plane
            && self.server.roles.data_plane
            && self.control_plane.client_ws_path == "/health"
        {
            return Err("control_plane.client_ws_path must not equal /health".to_string());
        }

        if self.server.roles.control_plane
            && self.control_plane.client_ws_path == self.control_plane.node_ws_path
        {
            return Err(
                "control_plane.client_ws_path and node_ws_path must be different".to_string(),
            );
        }

        if let Some(task_store) = &self.runtime.task_store {
            task_store.validate()?;
        }
        if let Some(session_store) = &self.runtime.session_store {
            session_store.validate()?;
        }
        if !(0.0..=1.0).contains(&self.opentelemetry.sample_rate) {
            return Err(format!(
                "opentelemetry.sample_rate must be between 0.0 and 1.0, got {}",
                self.opentelemetry.sample_rate
            ));
        }
        Ok(())
    }

    /// Load from explicit path, or fall back to default path, or use pure defaults.
    pub fn load_or_default(explicit: Option<&str>) -> Result<(Self, Option<PathBuf>), String> {
        if let Some(p) = explicit {
            let path = PathBuf::from(p);
            let config = Self::load(&path)?;
            return Ok((config, Some(path)));
        }
        let default_path = default_config_path();
        if default_path.exists() {
            let config = Self::load(&default_path)?;
            return Ok((config, Some(default_path)));
        }
        Ok((ServerConfig::default(), None))
    }

    /// Expand `~` in path fields to home directory.
    pub fn expand_tilde(&mut self) {
        self.runtime.working_dir = expand_tilde_str(&self.runtime.working_dir);
        self.runtime.store_dir = expand_tilde_str(&self.runtime.store_dir);
    }
}

/// Default config path: `~/.vol/agent-server.toml`
fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(format!("{home}/.vol/agent-server.toml"))
}

fn expand_tilde_str(s: &str) -> String {
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let rest = s.trim_start_matches('~').trim_start_matches('/');
        if rest.is_empty() {
            home
        } else {
            format!("{home}/{rest}")
        }
    } else {
        s.to_string()
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3001);
        assert_eq!(config.runtime.working_dir, ".");
        assert_eq!(config.runtime.store_dir, "~/.vol");
        assert_eq!(config.tracing.level, "info");
        assert_eq!(config.tracing.format, "text");
    }

    #[test]
    fn test_expand_tilde() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let result = expand_tilde_str("~/foo/bar");
        assert_eq!(result, format!("{home}/foo/bar"));
    }

    #[test]
    fn test_expand_tilde_home_only() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let result = expand_tilde_str("~");
        assert_eq!(result, home);
    }

    #[test]
    fn test_expand_no_tilde() {
        let result = expand_tilde_str("/absolute/path");
        assert_eq!(result, "/absolute/path");
    }

    #[test]
    fn test_parse_minimal_toml() {
        let toml_str = "";
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 3001);
    }

    #[test]
    fn test_parse_roles_config() {
        let toml_str = r#"
        [server.roles]
        control_plane = true
        data_plane = false

        [control_plane]
        client_ws_path = "/ws"
        node_ws_path = "/control/v1/ws"
        lease_timeout_secs = 90
        lease_scan_secs = 15
    "#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert!(config.server.roles.control_plane);
        assert!(!config.server.roles.data_plane);
        assert_eq!(config.control_plane.client_ws_path, "/ws");
        assert_eq!(config.control_plane.node_ws_path, "/control/v1/ws");
    }

    #[test]
    fn test_reject_both_roles_disabled() {
        let toml_str = r#"
        [server.roles]
        control_plane = false
        data_plane = false
    "#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("at least one server role must be enabled"));
    }

    #[test]
    fn test_reject_health_route_ws_path_collision() {
        let cases = [
            (
                r#"
                [server.roles]
                control_plane = true
                data_plane = false

                [control_plane]
                client_ws_path = "/health"
            "#,
                "control_plane.client_ws_path must not equal /health",
            ),
            (
                r#"
                [server.roles]
                control_plane = true
                data_plane = false

                [control_plane]
                node_ws_path = "/health"
            "#,
                "control_plane.node_ws_path must not equal /health",
            ),
            (
                r#"
                [server.roles]
                control_plane = false
                data_plane = true

                [control_plane]
                client_ws_path = "/health"
            "#,
                "control_plane.client_ws_path must not equal /health",
            ),
        ];

        for (toml_str, expected_err) in cases {
            let config: ServerConfig = toml::from_str(toml_str).unwrap();
            let err = config.validate().unwrap_err();
            assert_eq!(err, expected_err);
        }
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml_str = r#"
[server]
port = 8080

[tracing]
level = "debug"
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.host, "0.0.0.0"); // default preserved
        assert_eq!(config.tracing.level, "debug");
        assert_eq!(config.tracing.format, "text"); // default preserved
        assert_eq!(config.runtime.working_dir, "."); // default preserved
    }

    #[test]
    fn test_reject_equal_client_and_node_ws_paths() {
        let toml_str = r#"
        [server.roles]
        control_plane = true
        data_plane = false

        [control_plane]
        client_ws_path = "/ws"
        node_ws_path = "/ws"
    "#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        assert!(
            err.contains("client_ws_path and node_ws_path must be different"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_equal_ws_paths_allowed_when_control_plane_disabled() {
        // When control plane is not running, same paths don't matter.
        let toml_str = r#"
        [server.roles]
        control_plane = false
        data_plane = true

        [control_plane]
        client_ws_path = "/ws"
        node_ws_path = "/ws"
    "#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_parse_full_toml() {
        let toml_str = r#"
[server]
host = "127.0.0.1"
port = 9090

[runtime]
working_dir = "/app"
store_dir = "/data"

[tracing]
level = "debug"
format = "json"
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.runtime.working_dir, "/app");
        assert_eq!(config.runtime.store_dir, "/data");
        assert_eq!(config.tracing.level, "debug");
        assert_eq!(config.tracing.format, "json");
    }

    #[test]
    fn test_parse_database_task_store_config() {
        let toml_str = r#"
[runtime]
working_dir = "/app"
store_dir = "/data"

[runtime.task_store]
type = "database"
url = "sqlite:///tmp/vol-agent/tasks.db"
"#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let task_store = config.runtime.task_store.as_ref().unwrap();
        assert_eq!(
            task_store.store_type,
            vol_llm_runtime::TaskStoreType::Database
        );
        assert_eq!(
            task_store.url.as_deref(),
            Some("sqlite:///tmp/vol-agent/tasks.db")
        );
    }

    #[test]
    fn parses_database_session_store_config() {
        let toml = r#"
[runtime]
working_dir = "."
store_dir = ".vol-test"

[runtime.session_store]
type = "database"
url = "sqlite://data/sessions.db"
"#;
        let config: ServerConfig = toml::from_str(toml).unwrap();
        let session_store = config.runtime.session_store.unwrap();
        assert_eq!(
            session_store.store_type,
            vol_llm_runtime::SessionStoreType::Database
        );
        assert_eq!(
            session_store.url.as_deref(),
            Some("sqlite://data/sessions.db")
        );
    }

    #[test]
    fn validates_session_store_config() {
        let toml = r#"
[runtime.session_store]
type = "database"
"#;
        let config: ServerConfig = toml::from_str(toml).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("runtime.session_store.url is required"));
    }

    #[test]
    fn test_database_task_store_requires_url() {
        let toml_str = r#"
[runtime.task_store]
type = "database"
"#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        assert_eq!(
            err,
            "runtime.task_store.url is required when type = \"database\""
        );
    }

    #[test]
    fn test_file_task_store_rejects_url() {
        let toml_str = r#"
[runtime.task_store]
type = "file"
url = "sqlite:///tmp/tasks.db"
"#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        assert_eq!(
            err,
            "runtime.task_store.url is not valid when type = \"file\""
        );
    }

    #[test]
    fn test_database_task_store_rejects_unknown_scheme() {
        let toml_str = r#"
[runtime.task_store]
type = "database"
url = "oracle://localhost/tasks"
"#;

        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        assert_eq!(err, "unsupported task store database url scheme: oracle");
    }

    #[test]
    fn test_opentelemetry_defaults() {
        let config = ServerConfig::default();
        assert!(!config.opentelemetry.enabled);
        assert_eq!(
            config.opentelemetry.endpoint,
            "http://otel-collector.observability.svc.cluster.local:4317"
        );
        assert_eq!(config.opentelemetry.service_name, "agent-server");
        assert_eq!(config.opentelemetry.service_namespace, "vol-agent");
        assert_eq!(config.opentelemetry.deployment_environment, "production");
        assert_eq!(config.opentelemetry.sample_rate, 1.0);
        assert_eq!(config.opentelemetry.batch_max_export_timeout_millis, 5000);
    }

    #[test]
    fn test_parse_opentelemetry_toml() {
        let toml_str = r#"
[opentelemetry]
enabled = true
endpoint = "http://localhost:4317"
service_name = "test-agent"
sample_rate = 0.5
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert!(config.opentelemetry.enabled);
        assert_eq!(config.opentelemetry.endpoint, "http://localhost:4317");
        assert_eq!(config.opentelemetry.service_name, "test-agent");
        assert_eq!(config.opentelemetry.sample_rate, 0.5);
        // Defaults preserved for unset fields
        assert_eq!(config.opentelemetry.service_namespace, "vol-agent");
        assert_eq!(config.opentelemetry.deployment_environment, "production");
        assert_eq!(config.opentelemetry.batch_max_export_timeout_millis, 5000);
    }

    #[test]
    fn test_reject_invalid_sample_rate() {
        let toml_str = r#"
[opentelemetry]
sample_rate = 1.5
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        let err = config.validate().unwrap_err();
        assert!(err.contains("opentelemetry.sample_rate must be between 0.0 and 1.0"));
    }

    #[test]
    fn test_parse_node_ingress_config() {
        let toml_str = r#"
[server.roles]
control_plane = true
data_plane = false

[control_plane.node_ingress]
"dp-1" = "wss://dp.vol.bestnathan.top/ws"
"dingtalk" = "wss://dingtalk.vol.bestnathan.top/ws"
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.control_plane.node_ingress.len(), 2);
        assert_eq!(
            config.control_plane.node_ingress.get("dp-1").unwrap(),
            "wss://dp.vol.bestnathan.top/ws"
        );
        assert_eq!(
            config.control_plane.node_ingress.get("dingtalk").unwrap(),
            "wss://dingtalk.vol.bestnathan.top/ws"
        );
    }
}
