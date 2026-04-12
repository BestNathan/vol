//! vol-llm-tools-builtin-bash: Bash tool implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_tool::{Tool, ToolContext, ToolResult};

/// Parameters for the Bash tool
#[derive(Debug, Deserialize, Serialize)]
pub struct BashParams {
    /// Command to execute
    pub command: String,
}

/// The Bash tool for executing shell commands
pub struct BashTool;

impl BashTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for BashTool {
    fn name(&self) -> &str {
        "bash"
    }

    fn description(&self) -> &str {
        "Execute a bash command."
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "Command to execute"
                }
            },
            "required": ["command"]
        }))
    }

    async fn execute(
        &self,
        _args: &str,
        _context: &ToolContext,
    ) -> std::result::Result<ToolResult, Box<dyn std::error::Error + Send>> {
        todo!("Bash tool implementation")
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}
