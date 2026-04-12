//! vol-llm-tools-builtin-edit: Edit tool implementation.

use async_trait::async_trait;
use vol_llm_tool::{Tool, ToolCall, ToolResult};

/// Parameters for the Edit tool
#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct EditParams {
    /// Path to the file to edit
    pub path: String,
    /// Search pattern to find
    pub search: String,
    /// Replacement text
    pub replace: String,
}

/// The Edit tool for editing files
pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

    fn description(&self) -> &str {
        "Edit a file by replacing search pattern with replacement text."
    }

    async fn call(&self, _params: ToolCall) -> Result<ToolResult, Box<dyn std::error::Error + Send + Sync>> {
        todo!("Edit tool implementation")
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}
