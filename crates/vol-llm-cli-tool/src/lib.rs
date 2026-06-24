//! vol-llm-cli-tool: core abstraction for "CLI-as-Tool".
//!
//! Loads TOML configs that declare a named CLI tool backed by a Sandbox,
//! validates the first command token against a binaries whitelist,
//! interpolates `{{env.VAR}}` placeholders, and formats sandbox output
//! into a tool result. Reused by both the direct-load path
//! (`vol-llm-tools-builtin-cli-tool`) and the MCP server path
//! (`vol-mcp-servers::cli_tools`).

pub mod config;
pub mod error;
pub mod exec;
pub mod interpolate;
pub mod validate;

pub use config::CliToolConfig;
pub use error::CliToolError;
pub use exec::{CliTool, ToolOutput};
