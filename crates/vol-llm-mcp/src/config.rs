//! MCP configuration parsing and merge logic.
//!
//! Follows the Claude Desktop .mcp.json schema:
//! ```json
//! { "mcpServers": { "name": { "command": "...", "args": [...], "env": {...} } } }
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;

use crate::error::McpError;

/// Raw deserialization of .mcp.json
#[derive(Debug, Deserialize, Clone)]
struct RawMcpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct RawServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

/// Try to deserialize a single server entry. Returns None if the entry
/// lacks a `command` field (e.g. HTTP/SSE transport with `url` instead).
fn try_parse_server(name: &str, value: &serde_json::Value) -> Option<RawServerConfig> {
    match serde_json::from_value::<RawServerConfig>(value.clone()) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!(
                "Skipping MCP server '{}' (no command field or invalid format): {}",
                name,
                e
            );
            None
        }
    }
}

/// Parsed server configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// Parsed and merged MCP configuration.
#[derive(Debug, Clone)]
pub struct McpConfig {
    servers: Vec<McpServerConfig>,
}

impl McpConfig {
    /// Load configuration from project-level and user-level sources.
    ///
    /// Priority: `.mcp.json` (project root) > `~/.mcp.json` (user home).
    /// Per-key merge: if both files define the same server name, the project-level wins.
    pub fn load(working_dir: Option<&std::path::Path>) -> Result<Self, McpError> {
        let project_config = load_project_config(working_dir)?;
        let user_config = load_user_config()?;
        let merged = merge_configs(project_config, user_config);
        Ok(merged)
    }

    /// Return all server configurations.
    pub fn servers(&self) -> &[McpServerConfig] {
        &self.servers
    }
}

/// Read and parse a `.mcp.json` file, returning `None` if it doesn't exist.
fn read_config(path: &Path) -> Result<Option<RawMcpConfig>, McpError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(McpError::ConfigParse {
                path: path.display().to_string(),
                detail: e.to_string(),
            })
        }
    };
    let config: RawMcpConfig = serde_json::from_str(&content).map_err(|e| McpError::ConfigParse {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    Ok(Some(config))
}

fn load_project_config(working_dir: Option<&Path>) -> Result<Option<RawMcpConfig>, McpError> {
    let dir = working_dir.map(|p| p.to_path_buf()).or_else(|| std::env::current_dir().ok());
    let Some(dir) = dir else { return Ok(None) };
    read_config(&dir.join(".mcp.json"))
}

fn load_user_config() -> Result<Option<RawMcpConfig>, McpError> {
    let Some(home) = dirs::home_dir() else { return Ok(None) };
    read_config(&home.join(".mcp.json"))
}

fn merge_configs(
    project: Option<RawMcpConfig>,
    user: Option<RawMcpConfig>,
) -> McpConfig {
    let mut merged: HashMap<String, serde_json::Value> = HashMap::new();

    // User-level first (lower priority)
    if let Some(user_cfg) = user {
        merged.extend(user_cfg.mcp_servers);
    }

    // Project-level overrides (higher priority)
    if let Some(project_cfg) = project {
        for (name, server) in project_cfg.mcp_servers {
            merged.insert(name, server);
        }
    }

    let servers = merged
        .into_iter()
        .filter_map(|(name, value)| {
            try_parse_server(&name, &value).map(|raw| McpServerConfig {
                name,
                command: raw.command,
                args: raw.args,
                env: raw.env,
            })
        })
        .collect();

    McpConfig { servers }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_user_config_only() {
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"test":{"command":"echo","args":["hello"]}}}"#
        ).unwrap();
        let merged = merge_configs(None, Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "test");
        assert_eq!(merged.servers[0].command, "echo");
    }

    #[test]
    fn test_merge_project_overrides_user() {
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"weather":{"command":"npx","args":["weather-server"]},"github":{"command":"npx","args":["github-server"]}}}"#
        ).unwrap();
        let project: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"weather":{"command":"uv","args":["run","weather.py"]}}}"#
        ).unwrap();

        let merged = merge_configs(Some(project), Some(user));
        assert_eq!(merged.servers.len(), 2);

        // weather should use project-level config
        let weather = merged.servers.iter().find(|s| s.name == "weather").unwrap();
        assert_eq!(weather.command, "uv");

        // github should use user-level config
        let github = merged.servers.iter().find(|s| s.name == "github").unwrap();
        assert_eq!(github.command, "npx");
    }

    #[test]
    fn test_merge_empty_user() {
        let user: RawMcpConfig = serde_json::from_str(r#"{"mcpServers":{}}"#).unwrap();
        let project: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"test":{"command":"echo"}}}"#
        ).unwrap();
        let merged = merge_configs(Some(project), Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "test");
    }

    #[test]
    fn test_merge_no_config() {
        let merged = merge_configs(None, None);
        assert!(merged.servers.is_empty());
    }

    #[test]
    fn test_merge_skips_servers_without_command() {
        // User config has both stdio and http transport servers
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{
                "stdio-srv":{"command":"echo","args":["hi"]},
                "http-srv":{"url":"http://localhost:3000","transport":"sse"}
            }}"#
        ).unwrap();
        let merged = merge_configs(None, Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "stdio-srv");
    }

    #[test]
    fn test_merge_mixed_valid_and_invalid() {
        // Project has stdio server, user has both types
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{
                "my-srv":{"command":"npx","args":["srv"]},
                "http-srv":{"url":"http://example.com"}
            }}"#
        ).unwrap();
        let project: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"my-srv":{"command":"uv","args":["run","srv.py"]}}}"#
        ).unwrap();

        let merged = merge_configs(Some(project), Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "my-srv");
        assert_eq!(merged.servers[0].command, "uv");
    }
}
