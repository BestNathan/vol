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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name() {
        assert_eq!(WriteTool::new().name(), "write_file");
    }
    #[test]
    fn test_description() {
        assert!(!WriteTool::new().description().is_empty());
    }
    #[test]
    fn test_parameters_is_valid() {
        let p = WriteTool::new().parameters();
        assert_eq!(p["type"], "object");
        let req = p["required"].as_array().unwrap();
        assert!(req.contains(&serde_json::json!("file_path")));
        assert!(req.contains(&serde_json::json!("content")));
    }
    #[test]
    fn test_default() {
        let _: WriteTool = Default::default();
    }
    #[test]
    fn test_params_deserialize() {
        let p: WriteParams = serde_json::from_value(serde_json::json!({
            "file_path": "f.txt", "content": "hello"
        }))
        .unwrap();
        assert_eq!(p.file_path, "f.txt");
        assert_eq!(p.content, "hello");
    }

    fn test_sandbox(dir: &tempfile::TempDir) -> ToolContext {
        let sandbox = vol_llm_sandbox::local::LocalSandbox::new(Some(dir.path().to_path_buf()));
        ToolContext::for_test().with_sandbox(std::sync::Arc::new(sandbox))
    }

    #[tokio::test]
    async fn test_execute_invalid_arguments() {
        let tool = WriteTool::new();
        // Missing required "content" field
        let args = serde_json::json!({"file_path": "f.txt"});
        let result = tool.execute(&args, &ToolContext::for_test()).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to parse arguments"));
    }

    #[tokio::test]
    async fn test_execute_path_traversal_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        let tool = WriteTool::new();
        // ".." escapes the sandbox root — must be rejected
        let args = serde_json::json!({"file_path": "../escape.txt", "content": "x"});
        let result = tool.execute(&args, &ctx).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to resolve path"));
    }

    #[tokio::test]
    async fn test_execute_create_dir_failure() {
        let dir = tempfile::tempdir().unwrap();
        // "blocker" is a regular file — creating a directory under it must fail
        std::fs::write(dir.path().join("blocker"), "i am a file").unwrap();
        let ctx = test_sandbox(&dir);
        let tool = WriteTool::new();
        let args = serde_json::json!({"file_path": "blocker/sub.txt", "content": "x"});
        let result = tool.execute(&args, &ctx).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to create directory"));
    }

    #[tokio::test]
    async fn test_execute_write_failure() {
        let dir = tempfile::tempdir().unwrap();
        // "sub" is an existing directory — writing to it fails with EISDIR
        std::fs::create_dir(dir.path().join("sub")).unwrap();
        let ctx = test_sandbox(&dir);
        let tool = WriteTool::new();
        let args = serde_json::json!({"file_path": "sub", "content": "x"});
        let result = tool.execute(&args, &ctx).await;
        let err = result.unwrap_err().to_string();
        assert!(err.contains("Failed to write file"));
    }
}
