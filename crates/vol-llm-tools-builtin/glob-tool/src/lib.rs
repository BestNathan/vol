//! vol-llm-tools-builtin-glob: Glob tool implementation.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
        _context: &ToolContext,
    ) -> vol_llm_tool::ToolResultType<ToolResult> {
        // Parse arguments
        let params: GlobParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        // Build full pattern from search_path + pattern
        let search_path = params.path.unwrap_or_else(|| ".".to_string());
        let full_pattern = PathBuf::from(&search_path)
            .join(&params.pattern)
            .to_string_lossy()
            .to_string();

        // Execute glob using glob crate
        let mut results: Vec<(PathBuf, std::time::SystemTime)> = Vec::new();

        for entry in glob::glob(&full_pattern).map_err(|e| {
            ToolError::ExecutionFailed(format!("Glob pattern error: {}", e))
        })? {
            let path = entry.map_err(|e| {
                ToolError::ExecutionFailed(format!("Path error: {}", e))
            })?;

            // Get modification time for sorting
            if let Ok(metadata) = std::fs::metadata(&path) {
                if let Ok(mtime) = metadata.modified() {
                    results.push((path, mtime));
                }
            }
        }

        // Sort results by modification time (newest first)
        results.sort_by(|a, b| b.1.cmp(&a.1));

        // Return newline-separated file paths
        if results.is_empty() {
            Ok(ToolResult::success("No files matched the pattern.".to_string()))
        } else {
            let paths: Vec<String> = results
                .into_iter()
                .map(|(path, _)| path.to_string_lossy().to_string())
                .collect();
            Ok(ToolResult::success(paths.join("\n")))
        }
    }
}
