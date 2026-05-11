//! MCP error types.

use thiserror::Error;

/// Errors that can occur in MCP operations.
#[derive(Debug, Error)]
pub enum McpError {
    #[error("configuration error: {0}")]
    Config(String),

    #[error("connection error: {0}")]
    Connection(String),

    #[error("protocol error: {0}")]
    Protocol(String),
}
