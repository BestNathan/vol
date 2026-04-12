//! vol-llm-tools-builtin-bash: Bash tool implementation.

use async_trait::async_trait;
use vol_llm_tool::{Tool, ToolCall, ToolResult};

/// Parameters for the Bash tool
#[derive(Debug, serde::Deserialize, serde::Serialize)]
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

    async fn call(&self, _params: ToolCall) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
        todo!("Bash tool implementation")
    }
}

impl Default for BashTool {
    fn default() -> Self {
        Self::new()
    }
}
