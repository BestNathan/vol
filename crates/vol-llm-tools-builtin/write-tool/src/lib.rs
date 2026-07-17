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
        "Create or overwrite a file with the specified content. Parent directories will be created if they don't exist."
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
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // Parse arguments
        let params: WriteParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse arguments: {e}")))?;

        // Resolve path through sandbox
        let file_path = context
            .resolve_path(&params.file_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to resolve path: {e}")))?;

        // Create parent directories if they don't exist
        if let Some(parent) = file_path.parent() {
            if !parent.as_os_str().is_empty() {
                context.sandbox.create_dir_all(parent).await.map_err(|e| {
                    ToolError::ExecutionFailed(format!("Failed to create directory: {e}"))
                })?;
            }
        }

        // Write file content
        context
            .sandbox
            .write_file(&file_path, params.content.as_bytes())
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {e}")))?;

        let output = format!(
            "Successfully wrote {} bytes to {}",
            params.content.len(),
            params.file_path
        );
        Ok(ToolResult::success(output))
    }
}
