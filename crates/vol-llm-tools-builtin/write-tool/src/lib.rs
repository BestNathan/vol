//! vol-llm-tools-builtin-write: Write tool implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

/// Error type for builtin tools
/// Re-exported from vol_llm_tool for convenience
pub use vol_llm_tool::ToolError as BuiltinToolError;

/// Parameters for the Write tool
#[derive(Debug, Deserialize, Serialize)]
pub struct WriteParams {
    /// Path to the file to write
    pub file_path: String,
    /// Content to write to the file
    pub content: String,
}

/// The Write tool for creating or overwriting files
pub struct WriteTool;

impl WriteTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for WriteTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Create or overwrite a file with the specified content. The parent directory must exist."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["file_path", "content"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // Parse arguments
        let params: WriteParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        // Check if parent directory exists
        let parent = std::path::Path::new(&params.file_path)
            .parent()
            .ok_or_else(|| ToolError::ExecutionFailed("Invalid file path".to_string()))?;

        if !parent.as_os_str().is_empty() && !tokio::fs::try_exists(parent).await.unwrap_or(false) {
            return Err(ToolError::ExecutionFailed(format!(
                "Parent directory does not exist: {}",
                parent.display()
            )));
        }

        // Write file content
        tokio::fs::write(&params.file_path, &params.content)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {}", e)))?;

        let output = format!("Successfully wrote {} bytes to {}", params.content.len(), params.file_path);
        Ok(ToolResult::success(output))
    }
}
