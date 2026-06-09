//! Server configuration via TOML.
//!
//! Loads from `~/.vol/agent-server.toml` by default, or from `--config <path>`.

use serde::Deserialize;
use std::path::PathBuf;

/// Top-level server configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default)]
    pub server: ServerSection,
    #[serde(default)]
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub tracing: TracingSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
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

// --- Defaults ---

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3001
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

// --- Default trait implementations ---

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
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

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            runtime: RuntimeSection::default(),
            tracing: TracingSection::default(),
        }
    }
}

// --- Load ---

impl ServerConfig {
    /// Load config from a TOML file path.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {:?}: {}", path, e))?;
        let config: Self = toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))?;
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(task_store) = &self.runtime.task_store {
            task_store.validate()?;
        }
        if let Some(session_store) = &self.runtime.session_store {
            session_store.validate()?;
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
    PathBuf::from(format!("{}/.vol/agent-server.toml", home))
}

fn expand_tilde_str(s: &str) -> String {
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let rest = s.trim_start_matches('~').trim_start_matches('/');
        if rest.is_empty() {
            home
        } else {
            format!("{}/{}", home, rest)
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
        assert_eq!(result, format!("{}/foo/bar", home));
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
        assert_eq!(task_store.store_type, vol_llm_runtime::TaskStoreType::Database);
        assert_eq!(task_store.url.as_deref(), Some("sqlite:///tmp/vol-agent/tasks.db"));
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
        assert_eq!(session_store.store_type, vol_llm_runtime::SessionStoreType::Database);
        assert_eq!(session_store.url.as_deref(), Some("sqlite://data/sessions.db"));
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
        assert_eq!(err, "runtime.task_store.url is required when type = \"database\"");
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
        assert_eq!(err, "runtime.task_store.url is not valid when type = \"file\"");
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
}
