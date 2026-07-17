//! vol-llm-tools-builtin-edit: Edit tool implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

/// Error type for builtin tools
/// Re-exported from vol_llm_tool for convenience
pub use vol_llm_tool::ToolError as BuiltinToolError;

/// Parameters for the Edit tool
#[derive(Debug, Deserialize, Serialize)]
pub struct EditParams {
    /// Path to the file to edit
    pub file_path: String,
    /// String to find and replace
    pub old_string: String,
    /// String to replace with
    pub new_string: String,
    /// If true, replace all occurrences; if false, error if multiple occurrences found
    #[serde(default)]
    pub replace_all: bool,
}

/// The Edit tool for replacing exact strings in files
///
/// IMPORTANT: You must read the file first to know the exact string to replace.
/// This tool performs exact string matching, not fuzzy matching.
pub struct EditTool;

impl EditTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for EditTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn description(&self) -> &'static str {
        "Replace exact string occurrences in a file. IMPORTANT: You must read the file first to know the exact string to replace. This tool performs exact string matching, not fuzzy matching."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact string to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "String to replace with"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "If true, replace all occurrences; if false (default), error if multiple occurrences found",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // Parse arguments
        let params: EditParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse arguments: {e}")))?;

        // Validate old_string is not empty
        if params.old_string.is_empty() {
            return Err(ToolError::InvalidArguments(
                "old_string cannot be empty".into(),
            ));
        }

        // Resolve path through sandbox
        let file_path = context
            .resolve_path(&params.file_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to resolve path: {e}")))?;

        // Read file contents via sandbox
        let raw = context
            .sandbox
            .read_file(&file_path, None, None)
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to read file: {e}")))?;

        let content = String::from_utf8_lossy(&raw).to_string();

        // Count occurrences of old_string
        let count = content.matches(&params.old_string).count();

        // Validate occurrences
        if count == 0 {
            return Err(ToolError::ExecutionFailed(format!(
                "String '{}' not found in file",
                params.old_string
            )));
        }

        if count > 1 && !params.replace_all {
            return Err(ToolError::ExecutionFailed(format!(
                "Found {} occurrences of '{}', but replace_all is false. Set replace_all=true to replace all occurrences.",
                count, params.old_string
            )));
        }

        // Perform replacement
        let new_content = if params.replace_all {
            content.replace(&params.old_string, &params.new_string)
        } else {
            // Single replacement - only replace first occurrence
            content.replacen(&params.old_string, &params.new_string, 1)
        };

        // Write back to file via sandbox
        context
            .sandbox
            .write_file(&file_path, new_content.as_bytes())
            .await
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to write file: {e}")))?;

        let output = format!(
            "Successfully replaced {} occurrence(s) of '{}' with '{}' in {}",
            count, params.old_string, params.new_string, params.file_path
        );
        Ok(ToolResult::success(output))
    }
}
