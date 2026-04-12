//! vol-llm-tools-builtin-glob: Glob tool implementation.

use async_trait::async_trait;
use vol_llm_tool::{Tool, ToolCall, ToolResult};

/// Parameters for the Glob tool
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct GlobParams {
    /// Glob pattern to match
    pub pattern: String,
}

/// The Glob tool for finding files matching a pattern
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &str {
        "glob"
    }

    fn description(&self) -> &str {
        "Find files matching a glob pattern."
    }

    async fn call(&self, _params: ToolCall) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
        todo!("Glob tool implementation")
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}
