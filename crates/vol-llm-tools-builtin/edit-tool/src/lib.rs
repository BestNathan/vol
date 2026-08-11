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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(EditTool::new().name(), "edit_file");
    }
    #[test]
    fn test_description() {
        assert!(!EditTool::new().description().is_empty());
    }
    #[test]
    fn test_parameters_is_valid() {
        let p = EditTool::new().parameters();
        assert_eq!(p["type"], "object");
        let req = p["required"].as_array().unwrap();
        assert!(req.contains(&serde_json::json!("file_path")));
        assert!(req.contains(&serde_json::json!("old_string")));
        assert!(req.contains(&serde_json::json!("new_string")));
    }
    #[test]
    fn test_default() {
        let _: EditTool = Default::default();
    }
    #[test]
    fn test_params_defaults() {
        let p: EditParams = serde_json::from_value(serde_json::json!({
            "file_path": "f.txt", "old_string": "a", "new_string": "b"
        }))
        .unwrap();
        assert!(!p.replace_all);
    }

    // ── Execute path tests ─────────────────────────────────────────

    fn test_sandbox(dir: &tempfile::TempDir) -> ToolContext {
        let sandbox = vol_llm_sandbox::local::LocalSandbox::new(Some(dir.path().to_path_buf()));
        ToolContext::for_test().with_sandbox(std::sync::Arc::new(sandbox))
    }

    #[tokio::test]
    async fn test_execute_single_replacement() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::write(dir.path().join("doc.txt"), "hello world").unwrap();

        let tool = EditTool::new();
        let args = serde_json::json!({
            "file_path": dir.path().join("doc.txt").to_str().unwrap(),
            "old_string": "world",
            "new_string": "rust"
        });
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("1 occurrence"));

        let content = std::fs::read_to_string(dir.path().join("doc.txt")).unwrap();
        assert_eq!(content, "hello rust");
    }

    #[tokio::test]
    async fn test_execute_string_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::write(dir.path().join("doc.txt"), "hello world").unwrap();

        let tool = EditTool::new();
        let args = serde_json::json!({
            "file_path": dir.path().join("doc.txt").to_str().unwrap(),
            "old_string": "nonexistent",
            "new_string": "replacement"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found in file"));
    }

    #[tokio::test]
    async fn test_execute_multiple_occurrences_no_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::write(dir.path().join("doc.txt"), "a a a").unwrap();

        let tool = EditTool::new();
        let args = serde_json::json!({
            "file_path": dir.path().join("doc.txt").to_str().unwrap(),
            "old_string": "a",
            "new_string": "b"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("3 occurrences"));
        assert!(err.contains("replace_all"));
    }

    #[tokio::test]
    async fn test_execute_replace_all() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::write(dir.path().join("doc.txt"), "a a a").unwrap();

        let tool = EditTool::new();
        let args = serde_json::json!({
            "file_path": dir.path().join("doc.txt").to_str().unwrap(),
            "old_string": "a",
            "new_string": "b",
            "replace_all": true
        });
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("3 occurrence"));

        let content = std::fs::read_to_string(dir.path().join("doc.txt")).unwrap();
        assert_eq!(content, "b b b");
    }

    #[tokio::test]
    async fn test_execute_empty_old_string_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::write(dir.path().join("doc.txt"), "content").unwrap();

        let tool = EditTool::new();
        let args = serde_json::json!({
            "file_path": dir.path().join("doc.txt").to_str().unwrap(),
            "old_string": "",
            "new_string": "x"
        });
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_execute_invalid_arguments_missing_field() {
        let tool = EditTool::new();
        // Missing required "new_string" field
        let args = serde_json::json!({
            "file_path": "f.txt",
            "old_string": "a"
        });
        let result = tool.execute(&args, &ToolContext::for_test()).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse arguments"));
    }
}
