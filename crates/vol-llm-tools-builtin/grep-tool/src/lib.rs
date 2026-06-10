//! vol-llm-tools-builtin-grep: Multi-backend grep tool.
//!
//! Strategy: prefers `rg` CLI when available (fast, .gitignore-aware),
//! falls back to Rust library (ignore + grep-searcher) otherwise.
//!
//! Both backends produce identical output format.

pub mod backend;
pub mod cli;
pub mod lib_impl;

use crate::backend::GrepBackend;
use crate::cli::RgCliBackend;
use crate::lib_impl::RustLibBackend;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::time::Duration;

use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

pub use vol_llm_tool::ToolError as BuiltinToolError;

const MODE_FILES: &str = "files_with_matches";
const MODE_COUNT: &str = "count";
const MODE_CONTENT: &str = "content";
const SEARCH_TIMEOUT_SECS: u64 = 30;

#[derive(Debug, Deserialize, Serialize)]
pub struct GrepParams {
    pub pattern: String,
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub glob: Option<String>,
    #[serde(default = "default_output_mode")]
    pub output_mode: String,
    #[serde(default)]
    pub case_sensitive: bool,
}

fn default_output_mode() -> String {
    MODE_FILES.to_string()
}

/// Shared output type for both backends.
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub path: PathBuf,
    pub match_count: usize,
    pub line_numbers: Vec<usize>,
}

pub struct GrepTool;

impl GrepTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GrepTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ExecutableTool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file content using regex patterns. Uses ripgrep when available, falls back to Rust library search. Respects .gitignore, skips binary files."
    }

    fn parameters(&self) -> serde_json::Value {
        json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Regex pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "Root directory to search in (default: current directory)"
                },
                "glob": {
                    "type": "string",
                    "description": "File pattern filter (e.g., \"*.rs\", \"**/*.toml\")"
                },
                "output_mode": {
                    "type": "string",
                    "enum": ["files_with_matches", "count", "content"],
                    "description": "Output format (default: files_with_matches)"
                },
                "case_sensitive": {
                    "type": "boolean",
                    "description": "Case-sensitive search (default: false)"
                }
            },
            "required": ["pattern"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: GrepParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let valid_modes = [MODE_FILES, MODE_COUNT, MODE_CONTENT];
        if !valid_modes.contains(&params.output_mode.as_str()) {
            return Err(ToolError::InvalidArguments(format!(
                "Invalid output_mode: {}. Valid modes are: {:?}",
                params.output_mode, valid_modes
            )));
        }

        let search_root = PathBuf::from(params.path.clone().unwrap_or_else(|| ".".to_string()));

        // Try rg via sandbox first, fall back to Rust library
        let results = match tokio::time::timeout(
            Duration::from_secs(SEARCH_TIMEOUT_SECS),
            RgCliBackend::search(&params, &search_root, &*context.sandbox),
        )
        .await
        {
            Ok(Ok(results)) => results,
            Ok(Err(_rg_err)) => tokio::time::timeout(
                Duration::from_secs(SEARCH_TIMEOUT_SECS),
                RustLibBackend::search(&params, &search_root, &*context.sandbox),
            )
            .await
            .map_err(|_| {
                ToolError::ExecutionFailed(
                    "Search timed out after 30 seconds. Try a narrower path or glob.".to_string(),
                )
            })?
            .map_err(ToolError::ExecutionFailed)?,
            Err(_) => {
                // rg timed out or was unavailable, use library fallback
                tokio::time::timeout(
                    Duration::from_secs(SEARCH_TIMEOUT_SECS),
                    RustLibBackend::search(&params, &search_root, &*context.sandbox),
                )
                .await
                .map_err(|_| {
                    ToolError::ExecutionFailed(
                        "Search timed out after 30 seconds. Try a narrower path or glob."
                            .to_string(),
                    )
                })?
                .map_err(ToolError::ExecutionFailed)?
            }
        };

        let content = format_results(&params.output_mode, &results);
        if content.is_empty() {
            Ok(ToolResult::success("No matches found."))
        } else {
            Ok(ToolResult::success(content))
        }
    }
}

/// Format search results consistently for both backends.
fn format_results(output_mode: &str, results: &[SearchResult]) -> String {
    match output_mode {
        MODE_FILES => results
            .iter()
            .map(|r| r.path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n"),
        MODE_COUNT => results
            .iter()
            .map(|r| format!("{}: {}", r.path.display(), r.match_count))
            .collect::<Vec<_>>()
            .join("\n"),
        MODE_CONTENT => results
            .iter()
            .flat_map(|r| {
                r.line_numbers
                    .iter()
                    .map(|ln| format!("{}:{}", r.path.display(), ln))
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}
