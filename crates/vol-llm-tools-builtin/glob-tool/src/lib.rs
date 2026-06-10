//! vol-llm-tools-builtin-glob: Glob tool implementation.

use async_trait::async_trait;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use vol_llm_sandbox::FileType;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult};

/// Error type for builtin tools
/// Re-exported from vol_llm_tool for convenience
pub use vol_llm_tool::ToolError as BuiltinToolError;

/// Parameters for the Glob tool
#[derive(Debug, Deserialize, Serialize)]
pub struct GlobParams {
    /// Glob pattern to match (e.g., "*.rs", "src/**/*.toml")
    pub pattern: String,
    /// Optional base path to search in (default: current directory)
    #[serde(default)]
    pub path: Option<String>,
}

/// The Glob tool for matching file paths using glob patterns
pub struct GlobTool;

impl GlobTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GlobTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for GlobTool {
    fn name(&self) -> &'static str {
        "glob"
    }

    fn description(&self) -> &'static str {
        "Match file paths using glob patterns. Returns matching file paths sorted by modification time (newest first)."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match (e.g., \"*.rs\", \"src/**/*.toml\")"
                },
                "path": {
                    "type": "string",
                    "description": "Optional base path to search in (default: current directory)",
                    "default": "."
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> vol_llm_tool::ToolResultType<ToolResult> {
        // Parse arguments
        let params: GlobParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let search_path = params.path.unwrap_or_else(|| ".".to_string());
        let resolved_path = context
            .resolve_path(&search_path)
            .map_err(|e| ToolError::ExecutionFailed(format!("Path resolution failed: {}", e)))?;

        // Build glob pattern matcher (matches against relative paths from search root)
        let glob_pattern = Pattern::new(&params.pattern)
            .map_err(|e| ToolError::ExecutionFailed(format!("Invalid glob pattern: {}", e)))?;

        // Walk directory tree using sandbox.read_dir()
        let mut results: Vec<(String, u64)> = Vec::new();
        let mut dirs_to_visit = vec![resolved_path.clone()];

        while let Some(dir) = dirs_to_visit.pop() {
            let entries = context.sandbox.read_dir(&dir).await.map_err(|e| {
                ToolError::ExecutionFailed(format!("Failed to read directory: {}", e))
            })?;

            for entry in entries {
                let entry_path = dir.join(&entry.name);

                if entry.file_type == FileType::Directory {
                    dirs_to_visit.push(entry_path.clone());
                }

                // Check if the relative path from search root matches the glob pattern
                if let Ok(relative) = entry_path.strip_prefix(&resolved_path) {
                    if glob_pattern.matches(&relative.to_string_lossy()) {
                        if let Ok(metadata) = context.sandbox.metadata(&entry_path).await {
                            results
                                .push((entry_path.to_string_lossy().to_string(), metadata.mtime));
                        }
                    }
                }
            }
        }

        // Sort results by modification time (newest first)
        results.sort_by(|a, b| b.1.cmp(&a.1));

        // Return newline-separated file paths
        if results.is_empty() {
            Ok(ToolResult::success(
                "No files matched the pattern.".to_string(),
            ))
        } else {
            let paths: Vec<String> = results.into_iter().map(|(p, _)| p).collect();
            Ok(ToolResult::success(paths.join("\n")))
        }
    }
}
