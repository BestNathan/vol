//! MCP error types.

use thiserror::Error;

/// Error type for MCP operations.
#[derive(Error, Debug)]
pub enum McpError {
    #[error("failed to parse config from {path}: {detail}")]
    ConfigParse { path: String, detail: String },

    #[error("MCP server '{0}' not found")]
    ServerNotFound(String),

    #[error("failed to connect to MCP server '{server}': {detail}")]
    ConnectionFailed { server: String, detail: String },

    #[error("MCP server '{server}' initialization timed out")]
    InitializeTimeout { server: String },

    #[error("tool call failed on server '{server}', tool '{tool}': {detail}")]
    ToolCallFailed { server: String, tool: String, detail: String },

    #[error("failed to read resource '{uri}' on server '{server}': {detail}")]
    ResourceReadFailed { server: String, uri: String, detail: String },

    #[error("failed to get prompt '{name}' on server '{server}': {detail}")]
    PromptGetFailed { server: String, name: String, detail: String },

    #[error("MCP server '{0}' is disconnected")]
    ServerDisconnected(String),

    #[error("transport error: {0}")]
    TransportError(String),
}
