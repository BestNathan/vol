//! MCP configuration parsing and merge logic.
//!
//! Follows the Claude Desktop .mcp.json schema:
//! ```json
//! { "mcpServers": { "name": { "type": "stdio", "command": "...", "args": [...], "env": {...} } } }
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::io::ErrorKind;
use std::path::Path;

use crate::error::McpError;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Transport configuration for an MCP server.
#[derive(Debug, Clone, PartialEq)]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        headers: Option<HashMap<String, String>>,
        env: HashMap<String, String>,
    },
}

/// Parsed server configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: McpTransport,
}

// ---------------------------------------------------------------------------
// Raw deserialization types
// ---------------------------------------------------------------------------

/// Raw deserialization of .mcp.json
#[derive(Debug, Deserialize, Clone)]
struct RawMcpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize, Clone)]
struct RawStdioConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
struct RawHttpConfig {
    url: String,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
    /// Accepted but ignored — legacy field, all HTTP uses streamable HTTP.
    #[serde(default, rename = "transport")]
    _transport: Option<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
enum RawMcpTransport {
    #[serde(rename = "stdio")]
    Stdio(RawStdioConfig),
    #[serde(rename = "http")]
    Http(RawHttpConfig),
    #[serde(rename = "streamable-http")]
    StreamableHttp(RawHttpConfig),
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Try to deserialize a single server entry. Returns None if the entry
/// lacks a valid `type` field or fails to parse.
fn try_parse_server(name: &str, value: &serde_json::Value) -> Option<McpServerConfig> {
    match serde_json::from_value::<RawMcpTransport>(value.clone()) {
        Ok(raw) => {
            let transport = match raw {
                RawMcpTransport::Stdio(cfg) => McpTransport::Stdio {
                    command: cfg.command,
                    args: cfg.args,
                    env: cfg.env,
                },
                RawMcpTransport::Http(cfg) => McpTransport::Http {
                    url: cfg.url,
                    headers: cfg.headers,
                    env: cfg.env,
                },
                RawMcpTransport::StreamableHttp(cfg) => McpTransport::Http {
                    url: cfg.url,
                    headers: cfg.headers,
                    env: cfg.env,
                },
            };
            Some(McpServerConfig {
                name: name.to_string(),
                transport,
            })
        }
        Err(e) => {
            tracing::warn!(
                "Skipping MCP server '{}' (missing or invalid 'type' field): {}",
                name,
                e
            );
            None
        }
    }
}

// ---------------------------------------------------------------------------
// McpConfig
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// File I/O and merge
// ---------------------------------------------------------------------------

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
    let config: RawMcpConfig =
        serde_json::from_str(&content).map_err(|e| McpError::ConfigParse {
            path: path.display().to_string(),
            detail: e.to_string(),
        })?;
    Ok(Some(config))
}

fn load_project_config(working_dir: Option<&Path>) -> Result<Option<RawMcpConfig>, McpError> {
    let dir = working_dir
        .map(std::path::Path::to_path_buf)
        .or_else(|| std::env::current_dir().ok());
    let Some(dir) = dir else { return Ok(None) };
    read_config(&dir.join(".mcp.json"))
}

fn load_user_config() -> Result<Option<RawMcpConfig>, McpError> {
    let Some(home) = dirs::home_dir() else {
        return Ok(None);
    };
    read_config(&home.join(".mcp.json"))
}

fn merge_configs(project: Option<RawMcpConfig>, user: Option<RawMcpConfig>) -> McpConfig {
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
        .filter_map(|(name, value)| try_parse_server(&name, &value))
        .collect();

    McpConfig { servers }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_user_config_only() {
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"test":{"type":"stdio","command":"echo","args":["hello"]}}}"#,
        )
        .unwrap();
        let merged = merge_configs(None, Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "test");
        if let McpTransport::Stdio { command, args, .. } = &merged.servers[0].transport {
            assert_eq!(command, "echo");
            assert_eq!(args, &["hello"]);
        } else {
            panic!("expected Stdio transport");
        }
    }

    #[test]
    fn test_merge_project_overrides_user() {
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"weather":{"type":"stdio","command":"npx","args":["weather-server"]},"github":{"type":"stdio","command":"npx","args":["github-server"]}}}"#
        ).unwrap();
        let project: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"weather":{"type":"stdio","command":"uv","args":["run","weather.py"]}}}"#
        ).unwrap();

        let merged = merge_configs(Some(project), Some(user));
        assert_eq!(merged.servers.len(), 2);

        // weather should use project-level config
        let weather = merged.servers.iter().find(|s| s.name == "weather").unwrap();
        if let McpTransport::Stdio { command, .. } = &weather.transport {
            assert_eq!(command, "uv");
        } else {
            panic!("expected Stdio transport for weather");
        }

        // github should use user-level config
        let github = merged.servers.iter().find(|s| s.name == "github").unwrap();
        if let McpTransport::Stdio { command, .. } = &github.transport {
            assert_eq!(command, "npx");
        } else {
            panic!("expected Stdio transport for github");
        }
    }

    #[test]
    fn test_merge_empty_user() {
        let user: RawMcpConfig = serde_json::from_str(r#"{"mcpServers":{}}"#).unwrap();
        let project: RawMcpConfig =
            serde_json::from_str(r#"{"mcpServers":{"test":{"type":"stdio","command":"echo"}}}"#)
                .unwrap();
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
    fn test_merge_skips_servers_without_type() {
        // User config has both stdio and http transport servers
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{
                "stdio-srv":{"type":"stdio","command":"echo","args":["hi"]},
                "http-srv":{"url":"http://localhost:3000","transport":"sse"}
            }}"#,
        )
        .unwrap();
        let merged = merge_configs(None, Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "stdio-srv");
    }

    #[test]
    fn test_merge_mixed_valid_and_invalid() {
        // Project has stdio server, user has both types
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{
                "my-srv":{"type":"stdio","command":"npx","args":["srv"]},
                "http-srv":{"url":"http://example.com"}
            }}"#,
        )
        .unwrap();
        let project: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"my-srv":{"type":"stdio","command":"uv","args":["run","srv.py"]}}}"#,
        )
        .unwrap();

        let merged = merge_configs(Some(project), Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "my-srv");
        if let McpTransport::Stdio { command, .. } = &merged.servers[0].transport {
            assert_eq!(command, "uv");
        } else {
            panic!("expected Stdio transport");
        }
    }

    #[test]
    fn test_parse_stdio_type() {
        let value: serde_json::Value =
            serde_json::from_str(r#"{"type":"stdio","command":"echo","args":["hello"]}"#).unwrap();
        let parsed = try_parse_server("test-srv", &value).unwrap();
        assert!(matches!(parsed.transport, McpTransport::Stdio { .. }));
        assert_eq!(parsed.name, "test-srv");
    }

    #[test]
    fn test_parse_http_type() {
        let value: serde_json::Value =
            serde_json::from_str(r#"{"type":"http","url":"http://localhost:3000/mcp"}"#).unwrap();
        let parsed = try_parse_server("http-srv", &value).unwrap();
        assert!(matches!(parsed.transport, McpTransport::Http { .. }));
        assert_eq!(parsed.name, "http-srv");
    }

    #[test]
    fn test_parse_streamable_http_type() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"type":"streamable-http","url":"https://mcp-gw.dingtalk.com/server/test"}"#,
        )
        .unwrap();
        let parsed = try_parse_server("dingtalk", &value).unwrap();
        assert!(matches!(parsed.transport, McpTransport::Http { .. }));
        assert_eq!(parsed.name, "dingtalk");
    }

    #[test]
    fn test_parse_streamable_http_with_headers() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"type":"streamable-http","url":"https://example.com/mcp","headers":{"Authorization":"Bearer token"}}"#
        ).unwrap();
        let parsed = try_parse_server("auth-srv", &value).unwrap();
        match &parsed.transport {
            McpTransport::Http { headers, url, .. } => {
                assert!(headers.is_some());
                let h = headers.as_ref().unwrap();
                assert_eq!(h.get("Authorization").unwrap(), "Bearer token");
                assert_eq!(url, "https://example.com/mcp");
            }
            _ => panic!("expected Http transport"),
        }
    }

    #[test]
    fn test_parse_http_with_headers() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"type":"http","url":"http://localhost:3000/mcp","headers":{"Authorization":"Bearer token"}}"#
        ).unwrap();
        let parsed = try_parse_server("auth-srv", &value).unwrap();
        match &parsed.transport {
            McpTransport::Http { headers, .. } => {
                assert!(headers.is_some());
                let h = headers.as_ref().unwrap();
                assert_eq!(h.get("Authorization").unwrap(), "Bearer token");
            }
            _ => panic!("expected Http transport"),
        }
    }

    #[test]
    fn test_parse_http_with_env_proxy() {
        let value: serde_json::Value = serde_json::from_str(
            r#"{"type":"http","url":"https://docs.deribit.com/mcp","env":{"HTTPS_PROXY":"http://192.168.2.98:7890"}}"#
        ).unwrap();
        let parsed = try_parse_server("deribit", &value).unwrap();
        match &parsed.transport {
            McpTransport::Http { env, .. } => {
                assert_eq!(env.get("HTTPS_PROXY").unwrap(), "http://192.168.2.98:7890");
            }
            _ => panic!("expected Http transport"),
        }
    }

    #[test]
    fn test_missing_type_is_skipped() {
        let value: serde_json::Value = serde_json::from_str(r#"{"command":"echo"}"#).unwrap();
        let result = try_parse_server("no-type-srv", &value);
        assert!(result.is_none());
    }

    #[test]
    fn test_unrecognized_type_is_skipped() {
        let value: serde_json::Value =
            serde_json::from_str(r#"{"type":"websocket","url":"ws://localhost:3000"}"#).unwrap();
        let result = try_parse_server("ws-srv", &value);
        assert!(result.is_none());
    }

    #[test]
    fn test_merge_with_mixed_transports() {
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{
                "stdio-srv":{"type":"stdio","command":"echo","args":["hi"]},
                "http-srv":{"type":"http","url":"http://localhost:3000"}
            }}"#,
        )
        .unwrap();
        let merged = merge_configs(None, Some(user));
        assert_eq!(merged.servers.len(), 2);
    }
}
