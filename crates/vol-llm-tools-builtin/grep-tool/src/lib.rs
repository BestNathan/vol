//! vol-llm-tools-builtin-grep: Grep tool implementation.

use async_trait::async_trait;
use vol_llm_tool::{Tool, ToolCall, ToolResult};

/// Parameters for the Grep tool
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GrepParams {
    /// Pattern to search for
    pub pattern: String,
    /// Optional path to search in
    pub path: Option<String>,
}

/// The Grep tool for searching text in files
pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &str {
        "grep"
    }

    fn description(&self) -> &str {
        "Search for a pattern in files."
    }

    async fn call(&self, _params: ToolCall) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
        todo!("Grep tool implementation")
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}
