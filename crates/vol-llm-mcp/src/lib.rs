//! vol-llm-mcp: MCP Client protocol layer for ReAct Agent.
//!
//! Provides configuration parsing, session management, and tool discovery
//! for MCP servers configured via ~/.mcp.json and .mcp.json.

pub mod config;
pub mod error;
pub mod manager;
pub mod session;

pub use config::McpConfig;
pub use error::McpError;
pub use manager::McpManager;
pub use session::McpSession;
