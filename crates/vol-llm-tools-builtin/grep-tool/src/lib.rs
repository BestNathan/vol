//! vol-llm-tools-builtin-grep: Grep tool implementation.

use async_trait::async_trait;
use grep::regex::RegexMatcher;
use grep_matcher::Matcher;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use walkdir::WalkDir;

use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

/// Error type for builtin tools
/// Re-exported from vol_llm_tool for convenience
pub use vol_llm_tool::ToolError as BuiltinToolError;

/// Output mode constants
const MODE_FILES: &str = "files_with_matches";
const MODE_COUNT: &str = "count";
const MODE_CONTENT: &str = "content";

/// Parameters for the Grep tool
#[derive(Debug, Deserialize, Serialize)]
pub struct GrepParams {
    /// Regex pattern to search for
    pub pattern: String,
    /// Optional root directory to search in (default: current directory)
    #[serde(default)]
    pub path: Option<String>,
    /// Optional glob pattern filter (e.g., "*.rs")
    #[serde(default)]
    pub glob: Option<String>,
    /// Output format mode (default: "files_with_matches")
    #[serde(default = "default_output_mode")]
    pub output_mode: String,
    /// Case-sensitive search (default: false)
    #[serde(default)]
    pub case_sensitive: bool,
}

fn default_output_mode() -> String {
    MODE_FILES.to_string()
}

/// The Grep tool for searching file content using regex patterns
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

#[derive(Debug)]
struct SearchResult {
    path: PathBuf,
    match_count: usize,
    line_numbers: Vec<usize>,
}

#[async_trait]
impl ExecutableTool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }

    fn description(&self) -> &'static str {
        "Search file content using regex patterns. Returns matching files or match details based on output mode."
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
                    "description": "Root directory to search in (default: current directory)",
                    "default": "."
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
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // Parse arguments
        let params: GrepParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        // Validate output_mode
        let valid_modes = [MODE_FILES, MODE_COUNT, MODE_CONTENT];
        if !valid_modes.contains(&params.output_mode.as_str()) {
            return Err(ToolError::InvalidArguments(format!(
                "Invalid output_mode: {}. Valid modes are: {:?}",
                params.output_mode, valid_modes
            )));
        }

        let search_path = params.path.unwrap_or_else(|| ".".to_string());

        // Build regex matcher
        let matcher = if params.case_sensitive {
            RegexMatcher::new(&params.pattern)
        } else {
            RegexMatcher::new(&format!("(?i){}", &params.pattern))
        }
        .map_err(|e| {
            ToolError::InvalidArguments(format!("Invalid regex pattern: {}", e))
        })?;

        // Collect files to search with recursion depth limit
        let mut files: Vec<PathBuf> = Vec::new();
        const MAX_DEPTH: usize = 50;
        for entry in WalkDir::new(&search_path)
            .max_depth(MAX_DEPTH)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.is_file() {
                // Skip files larger than 10MB
                if let Ok(metadata) = std::fs::metadata(path) {
                    const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
                    if metadata.len() > MAX_FILE_SIZE {
                        continue;
                    }
                }

                if let Some(glob_pattern) = &params.glob {
                    if glob_match(glob_pattern, path) {
                        files.push(path.to_path_buf());
                    }
                } else {
                    files.push(path.to_path_buf());
                }
            }
        }

        // Search files
        let mut results: Vec<SearchResult> = Vec::new();
        for file_path in &files {
            let file = match File::open(file_path) {
                Ok(f) => f,
                Err(_) => continue, // Skip files we can't open
            };

            let reader = BufReader::new(file);
            let mut line_numbers: Vec<usize> = Vec::new();
            let mut match_count = 0;

            for (idx, line_result) in reader.lines().enumerate() {
                if let Ok(line) = line_result {
                    if matcher.is_match(line.as_bytes()).unwrap_or(false) {
                        line_numbers.push(idx + 1); // 1-indexed line numbers
                        match_count += 1;
                    }
                }
            }

            if match_count > 0 {
                results.push(SearchResult {
                    path: file_path.clone(),
                    match_count,
                    line_numbers,
                });
            }
        }

        // Format output based on mode
        let content = match params.output_mode.as_str() {
            MODE_FILES => {
                let paths: Vec<String> = results
                    .iter()
                    .map(|r| r.path.to_string_lossy().to_string())
                    .collect();
                paths.join("\n")
            }
            MODE_COUNT => {
                let counts: Vec<String> = results
                    .iter()
                    .map(|r| format!("{}: {}", r.path.display(), r.match_count))
                    .collect();
                counts.join("\n")
            }
            MODE_CONTENT => {
                let lines: Vec<String> = results
                    .iter()
                    .flat_map(|r| {
                        r.line_numbers
                            .iter()
                            .map(|ln| format!("{}:{}", r.path.display(), ln))
                    })
                    .collect();
                lines.join("\n")
            }
            // Already validated above, this is just for exhaustiveness
            _ => unreachable!(),
        };

        if content.is_empty() {
            Ok(ToolResult::success("No matches found."))
        } else {
            Ok(ToolResult::success(content))
        }
    }
}

/// Simple glob match helper using pattern matching
fn glob_match(pattern: &str, path: &std::path::Path) -> bool {
    // Get the file name for matching
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    // Support basic glob patterns like *.rs, *.txt, etc.
    if pattern == "*" {
        return true;
    }
    if pattern.starts_with("*.") {
        let ext = &pattern[1..];
        return name.ends_with(ext);
    }
    if pattern.ends_with("*") {
        let prefix = &pattern[..pattern.len() - 1];
        return name.starts_with(prefix);
    }
    pattern == name
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_grep_basic() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();
        writeln!(f1, "foo bar").unwrap();
        writeln!(f1, "hello again").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap(),
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test.txt"));
    }

    #[tokio::test]
    async fn test_grep_no_matches() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();
        writeln!(f1, "foo bar").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "nonexistent",
            "path": dir.path().to_str().unwrap(),
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("No matches"));
    }

    #[tokio::test]
    async fn test_grep_with_glob() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.rs")).unwrap();
        writeln!(f1, "fn main() {{ println!(\"hello\"); }}").unwrap();

        let mut f2 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f2, "hello world").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap(),
            "glob": "*.rs",
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        assert!(result.content.contains("test.rs"));
        assert!(!result.content.contains("test.txt"));
    }
}
