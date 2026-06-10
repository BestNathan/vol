//! vol-llm-tools-builtin-read: Read tool implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

/// Error type for builtin tools
/// Re-exported from vol_llm_tool for convenience
pub use vol_llm_tool::ToolError as BuiltinToolError;

/// Parameters for the Read tool
#[derive(Debug, Deserialize, Serialize)]
pub struct ReadParams {
    /// Path to the file to read
    pub file_path: String,
    /// Line offset to start reading from (0-indexed, default: 0)
    #[serde(default)]
    pub offset: usize,
    /// Maximum number of lines to read (default: 2000)
    #[serde(default = "default_limit")]
    pub limit: usize,
}

fn default_limit() -> usize {
    2000
}

/// The Read tool for reading files with line numbers
pub struct ReadTool;

impl ReadTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for ReadTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read file contents with line numbers. Supports offset to skip initial lines and limit to restrict output length."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line offset to start reading from (0-indexed)",
                    "default": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read",
                    "default": 2000
                }
            },
            "required": ["file_path"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // Parse arguments
        let params: ReadParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        // Resolve path through sandbox
        let file_path = context
            .resolve_path(&params.file_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Path resolution failed: {}", e)))?;

        // Read file contents via sandbox (read full file; offset/limit applied by line below)
        let raw = context
            .sandbox
            .read_file(&file_path, None, None)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {}", e)))?;

        let content = String::from_utf8_lossy(&raw).to_string();

        // Apply offset and limit (line-level)
        let lines: Vec<&str> = content.lines().collect();
        let start = params.offset.min(lines.len());
        let end = (start + params.limit).min(lines.len());
        let selected_lines = &lines[start..end];

        // Format with line numbers (cat -n style: "   1  | content")
        let formatted: Vec<String> = selected_lines
            .iter()
            .enumerate()
            .map(|(i, line)| {
                let line_num = start + i + 1; // 1-indexed line numbers
                format!("{:5}  |  {}", line_num, line)
            })
            .collect();

        let output = formatted.join("\n");

        Ok(ToolResult::success(output))
    }
}
