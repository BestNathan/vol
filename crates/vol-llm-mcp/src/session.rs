//! MCP Session — manages connections to multiple MCP servers.

use rmcp::model::{CallToolRequestParams, ClientInfo, JsonObject, RawContent, Tool};
use rmcp::service::{Peer, RoleClient, RunningService, ServiceError, ServiceExt};
use rmcp::transport::TokioChildProcess;
use std::collections::HashMap;
use std::process::Stdio;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing;

use crate::config::{McpServerConfig, McpTransport};
use crate::error::McpError;

/// Sanitize a server name to only contain [a-zA-Z0-9_-].
pub fn sanitize_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut prev_underscore = false;
    for c in name.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            if c == '_' {
                if prev_underscore {
                    continue; // skip consecutive underscores
                }
                prev_underscore = true;
            } else {
                prev_underscore = false;
            }
            result.push(c);
        } else if !prev_underscore {
            result.push('_');
            prev_underscore = true;
        }
    }
    // Remove trailing underscore
    if result.ends_with('_') {
        result.pop();
    }
    if result.is_empty() {
        result = "unknown".to_string();
    }
    result
}

/// MCP tool metadata.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

/// A single server connection — holds the running service and cached tools.
pub struct ServerConnection {
    pub config: McpServerConfig,
    pub running_service: RunningService<RoleClient, ClientInfo>,
    pub tools: Vec<Tool>,
}

impl ServerConnection {
    pub fn peer(&self) -> &Peer<RoleClient> {
        self.running_service.peer()
    }

    pub async fn close(&mut self) -> Result<(), tokio::task::JoinError> {
        self.running_service.close().await?;
        Ok(())
    }
}

/// MCP Session — manages all server connections.
pub struct McpSession {
    connections: HashMap<String, ServerConnection>,
}

impl McpSession {
    /// Connect to all configured MCP servers.
    ///
    /// Servers that fail to connect are skipped with a tracing error.
    /// The initialization timeout is 10 seconds per server.
    pub async fn connect(configs: Vec<McpServerConfig>) -> Self {
        let mut connections = HashMap::new();

        for config in configs {
            match Self::connect_single(&config).await {
                Ok((service, tools)) => {
                    let sanitized = sanitize_name(&config.name);
                    connections.insert(
                        sanitized,
                        ServerConnection {
                            config,
                            running_service: service,
                            tools,
                        },
                    );
                }
                Err(e) => {
                    tracing::error!("MCP server '{}' failed to connect: {}", config.name, e);
                }
            }
        }

        Self { connections }
    }

    async fn connect_single(
        config: &McpServerConfig,
    ) -> Result<(RunningService<RoleClient, ClientInfo>, Vec<Tool>), McpError> {
        let McpTransport::Stdio { command, args, env } = &config.transport else {
            return Err(McpError::ConnectionFailed {
                server: config.name.clone(),
                detail: format!(
                    "unsupported transport type for session: {:?}",
                    config.transport
                ),
            });
        };
        let mut cmd = Command::new(command);
        cmd.args(args);
        for (key, value) in env {
            cmd.env(key, value);
        }
        cmd.stdin(Stdio::piped());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::inherit());

        let child = TokioChildProcess::new(cmd).map_err(|e: std::io::Error| {
            McpError::ConnectionFailed {
                server: config.name.clone(),
                detail: e.to_string(),
            }
        })?;

        // Build ClientInfo via default (which uses Implementation::from_build_env)
        let client_info = ClientInfo::default();

        let ct = CancellationToken::new();
        let service =
            client_info
                .serve_with_ct(child, ct)
                .await
                .map_err(|e| McpError::ConnectionFailed {
                    server: config.name.clone(),
                    detail: e.to_string(),
                })?;

        // List tools from the connected server
        let tools = service.peer().list_all_tools().await.unwrap_or_else(|e| {
            tracing::warn!("Failed to list tools for server '{}': {}", config.name, e);
            Vec::new()
        });

        Ok((service, tools))
    }

    /// List tools for a specific server.
    pub fn list_tools(&self, server: &str) -> Result<Vec<McpToolInfo>, McpError> {
        let conn = self
            .connections
            .get(server)
            .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;

        Ok(conn
            .tools
            .iter()
            .map(|t| McpToolInfo {
                name: t.name.to_string(),
                description: t.description.as_ref().map(std::string::ToString::to_string),
                input_schema: Some(t.schema_as_json_value()),
            })
            .collect())
    }

    /// List all tools from all servers.
    /// Returns (sanitized_server_name, tool_info) pairs.
    pub fn list_all_tools(&self) -> Vec<(String, McpToolInfo)> {
        let mut result = Vec::new();
        for (server, conn) in &self.connections {
            for tool in &conn.tools {
                result.push((
                    server.clone(),
                    McpToolInfo {
                        name: tool.name.to_string(),
                        description: tool
                            .description
                            .as_ref()
                            .map(std::string::ToString::to_string),
                        input_schema: Some(tool.schema_as_json_value()),
                    },
                ));
            }
        }
        result
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String, McpError> {
        let conn = self
            .connections
            .get(server)
            .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;

        let arguments = match args {
            serde_json::Value::Object(obj) => Some(JsonObject::from_iter(obj)),
            _ => Some(JsonObject::new()),
        };

        let params = match arguments {
            Some(args) => CallToolRequestParams::new(tool_name.to_string()).with_arguments(args),
            None => CallToolRequestParams::new(tool_name.to_string()),
        };

        let result = conn
            .peer()
            .call_tool(params)
            .await
            .map_err(|e: ServiceError| McpError::ToolCallFailed {
                server: server.to_string(),
                tool: tool_name.to_string(),
                detail: e.to_string(),
            })?;

        Ok(Self::format_tool_result(&result))
    }

    /// Format CallToolResult into a string.
    fn format_tool_result(result: &rmcp::model::CallToolResult) -> String {
        if result.is_error == Some(true) {
            let text = result
                .content
                .iter()
                .filter_map(|c| match &c.raw {
                    RawContent::Text(text_block) => Some(text_block.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            return format!(
                "MCP tool error: {}",
                if text.is_empty() {
                    "unknown error"
                } else {
                    &text
                }
            );
        }

        result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                RawContent::Text(text_block) => Some(text_block.text.clone()),
                RawContent::Image(_) => Some("[image content]".to_string()),
                RawContent::Resource(resource_block) => {
                    // Use uri since ResourceContents doesn't have a name field
                    let uri = match &resource_block.resource {
                        rmcp::model::ResourceContents::TextResourceContents { uri, .. } => uri,
                        rmcp::model::ResourceContents::BlobResourceContents { uri, .. } => uri,
                    };
                    Some(format!("[resource: {uri}]"))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Disconnect all server connections.
    pub async fn disconnect(&mut self) {
        for (name, conn) in &mut self.connections {
            if let Err(e) = conn.close().await {
                tracing::warn!("Error closing MCP server '{}': {}", name, e);
            }
        }
        self.connections.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name_already_clean() {
        assert_eq!(sanitize_name("weather"), "weather");
        assert_eq!(sanitize_name("my-server"), "my-server");
        assert_eq!(sanitize_name("my_server"), "my_server");
    }

    #[test]
    fn test_sanitize_name_replaces_special() {
        assert_eq!(sanitize_name("my server"), "my_server");
        assert_eq!(sanitize_name("my/server"), "my_server");
        assert_eq!(sanitize_name("my.server"), "my_server");
    }

    #[test]
    fn test_sanitize_name_consecutive_underscores_merged() {
        assert_eq!(sanitize_name("my__server"), "my_server");
        assert_eq!(sanitize_name("my   server"), "my_server");
    }

    #[test]
    fn test_sanitize_name_trailing_underscore_removed() {
        assert_eq!(sanitize_name("server!"), "server");
    }

    #[test]
    fn test_sanitize_name_empty_becomes_unknown() {
        assert_eq!(sanitize_name("unknown"), "unknown");
        assert_eq!(sanitize_name("!!!"), "unknown");
    }
}
