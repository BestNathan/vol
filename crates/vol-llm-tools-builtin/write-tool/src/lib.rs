//! vol-llm-tools-builtin-write: Write tool implementation.

use async_trait::async_trait;
use vol_llm_tool::{Tool, ToolCall, ToolResult};

/// Parameters for the Write tool
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct WriteParams {
    /// Path to the file to write
    pub path: String,
    /// Content to write to the file
    pub content: String,
}

/// The Write tool for writing files
pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file at the specified path."
    }

    async fn call(&self, _params: ToolCall) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
        todo!("Write tool implementation")
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}
