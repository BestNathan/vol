//! vol-llm-tools-builtin-read: Read tool implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_tool::{Tool, ToolCall, ToolResult};

/// Error type for builtin tools
#[derive(Debug, thiserror::Error)]
pub enum BuiltinToolError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Tool execution error: {0}")]
    Execution(String),
}

/// Parameters for the Read tool
#[derive(Debug, Deserialize, Serialize)]
pub struct ReadParams {
    /// Path to the file to read
    pub path: String,
}

/// The Read tool for reading files
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file at the specified path."
    }

    async fn call(&self, _params: ToolCall) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
        todo!("Read tool implementation")
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}
