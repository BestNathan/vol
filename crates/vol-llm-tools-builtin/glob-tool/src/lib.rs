//! vol-llm-tools-builtin-glob: Glob tool implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_tool::{Tool, ToolContext, ToolResult};

/// Parameters for the Glob tool
#[derive(Debug, Deserialize, Serialize)]
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

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match"
                }
            },
            "required": ["pattern"]
        }))
    }

    async fn execute(
        &self,
        _args: &str,
        _context: &ToolContext,
    ) -> std::result::Result<ToolResult, Box<dyn std::error::Error + Send>> {
        todo!("Glob tool implementation")
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}
