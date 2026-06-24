//! HTTP MCP server hosting one tool per `.agents/cli-tools/*.toml` config.

pub mod server;

pub use server::CliToolsMcpServer;
