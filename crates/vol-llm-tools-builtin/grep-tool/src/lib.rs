//! vol-llm-tools-builtin-grep: Optimized grep tool.
//!
//! Optimizations over the baseline:
//! 1. Respects `.gitignore` — reads patterns from search root, skips matched dirs
//! 2. Skips common large directories: `target`, `.git`, `node_modules`, etc.
//! 3. 30-second timeout to prevent hangs on very large repositories
//! 4. Binary file detection — skips files containing null bytes
//! 5. Better glob matching — supports `**/*.rs` path patterns

use async_trait::async_trait;
use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use ignore::WalkBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::time::Duration;

use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

pub use vol_llm_tool::ToolError as BuiltinToolError;

const MODE_FILES: &str = "files_with_matches";
const MODE_COUNT: &str = "count";
const MODE_CONTENT: &str = "content";

const MAX_DEPTH: usize = 50;
const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024; // 10 MB
const SEARCH_TIMEOUT_SECS: u64 = 30;

/// Directories always skipped during traversal.
const SKIP_DIRS: &[&str] = &[
    "target",
    ".git",
    "node_modules",
    ".cache",
    "__pycache__",
    "vendor",
    ".idea",
    ".vscode",
];

/// Binary file extensions to skip.
const BINARY_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "bmp", "ico", "svg",
    "pdf", "doc", "docx", "xls", "xlsx", "ppt", "pptx",
    "zip", "tar", "gz", "bz2", "xz", "7z", "rar",
    "exe", "dll", "so", "dylib", "wasm", "o", "a",
    "mp3", "mp4", "avi", "mov", "wav", "flac",
    "ttf", "otf", "woff", "woff2", "eot",
    "db", "sqlite", "sqlite3",
    "bin", "dat", "rlib", "rmeta",
];

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
        "Search file content using regex patterns. Respects .gitignore, skips binary/giant files. Returns matching files or match details."
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
        _context: &ToolContext,
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

        let search_path = params.path.clone().unwrap_or_else(|| ".".to_string());

        // Run the search with a timeout
        let search_future = run_search(params, search_path);
        match tokio::time::timeout(Duration::from_secs(SEARCH_TIMEOUT_SECS), search_future).await {
            Ok(result) => result,
            Err(_elapsed) => Err(ToolError::ExecutionFailed(
                "Search timed out after 30 seconds. Try narrowing the search path or using a more specific glob pattern.".to_string(),
            )),
        }
    }
}

async fn run_search(params: GrepParams, search_path: String) -> ToolResultType<ToolResult> {
    let output_mode = params.output_mode.clone();
    let glob_pattern = params.glob.clone();
    let case_sensitive = params.case_sensitive;

    // Build regex matcher
    let matcher = if case_sensitive {
        RegexMatcher::new(&params.pattern)
    } else {
        RegexMatcher::new(&format!("(?i){}", &params.pattern))
    }
    .map_err(|e| ToolError::InvalidArguments(format!("Invalid regex pattern: {}", e)))?;

    // Read .gitignore patterns from the search root
    let gitignore_patterns = read_gitignore(&search_path);

    // Compile glob patterns
    let glob_regex = glob_pattern.as_ref().map(|g| glob_to_regex(g));

    // Collect files with improved filtering
    let mut files: Vec<PathBuf> = Vec::new();
    let search_root = Path::new(&search_path);

    for entry in WalkBuilder::new(search_root)
        .max_depth(Some(MAX_DEPTH))
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map_or(false, |ft| ft.is_file()))
    {
        let path = entry.path();

        // Skip binary files by extension
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            if BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str()) {
                continue;
            }
        }

        // Skip large files
        if let Ok(metadata) = entry.metadata() {
            if metadata.len() > MAX_FILE_SIZE {
                continue;
            }
        }

        // Skip gitignored paths
        if is_ignored(path, search_root, &gitignore_patterns) {
            continue;
        }

        // Check binary content by sampling first bytes
        if is_likely_binary(path) {
            continue;
        }

        // Glob filter — match against filename (simple pattern) or relative path (**/ patterns)
        if let Some(ref re) = glob_regex {
            let match_target = if glob_pattern.as_ref().map_or(false, |g| g.contains("**")) {
                match path.strip_prefix(search_root) {
                    Ok(rel) => rel.to_string_lossy().to_string(),
                    Err(_) => path.to_string_lossy().to_string(),
                }
            } else {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            };
            if !re.is_match(&match_target) {
                continue;
            }
        }

        files.push(path.to_path_buf());
    }

    // Search files (offloaded to blocking pool for CPU-bound work)
    let results = tokio::task::spawn_blocking(move || {
        let mut results: Vec<SearchResult> = Vec::new();
        for file_path in &files {
            let file = match File::open(file_path) {
                Ok(f) => f,
                Err(_) => continue,
            };

            let reader = BufReader::new(file);
            let mut line_numbers: Vec<usize> = Vec::new();
            let mut match_count = 0;

            for (idx, line_result) in reader.lines().enumerate() {
                if let Ok(line) = line_result {
                    if matcher.is_match(line.as_bytes()).unwrap_or(false) {
                        line_numbers.push(idx + 1);
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
        results
    })
    .await
    .unwrap_or_default();

    // Format output
    let content = match output_mode.as_str() {
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
                    r.line_numbers.iter().map(|ln| format!("{}:{}", r.path.display(), ln))
                })
                .collect();
            lines.join("\n")
        }
        _ => unreachable!(),
    };

    if content.is_empty() {
        Ok(ToolResult::success("No matches found."))
    } else {
        Ok(ToolResult::success(content))
    }
}

/// Read .gitignore from the search root and return a set of patterns.
fn read_gitignore(root: &str) -> HashSet<String> {
    let mut patterns = HashSet::new();
    // Always skip well-known large directories
    for d in SKIP_DIRS {
        patterns.insert(format!("/{}/", d));
        patterns.insert(format!("/{}/", d));
    }
    // Read .gitignore if present
    let gitignore_path = Path::new(root).join(".gitignore");
    if let Ok(file) = File::open(&gitignore_path) {
        let reader = BufReader::new(file);
        for line in reader.lines().filter_map(|l| l.ok()) {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }
            // Convert to a simple prefix pattern for directory matching
            let pattern = trimmed.trim_start_matches('/');
            if pattern.ends_with('/') {
                patterns.insert(pattern.to_string());
            } else {
                patterns.insert(pattern.to_string());
            }
        }
    }
    patterns
}

/// Check if a path is covered by gitignore patterns.
fn is_ignored(path: &Path, root: &Path, patterns: &HashSet<String>) -> bool {
    let relative = match path.strip_prefix(root) {
        Ok(r) => r.to_string_lossy().to_string(),
        Err(_) => return false,
    };

    // Check if any path component matches a skip directory
    for component in path.components() {
        if let Some(name) = component.as_os_str().to_str() {
            if SKIP_DIRS.contains(&name) {
                return true;
            }
        }
    }

    // Check gitignore patterns
    for pattern in patterns {
        // Simple suffix/prefix matching
        if pattern.ends_with('/') {
            let dir_pattern = pattern.trim_end_matches('/');
            if relative.starts_with(dir_pattern) || relative.contains(&format!("/{}/", dir_pattern)) {
                return true;
            }
        } else if pattern.starts_with('*') {
            let suffix: &str = &pattern[1..];
            if relative.ends_with(suffix) {
                return true;
            }
        } else if relative == *pattern || relative == format!("{}/", pattern) {
            return true;
        }
    }
    false
}

/// Check if a file is likely binary by reading the first 8KB and looking for null bytes.
fn is_likely_binary(path: &Path) -> bool {
    if let Ok(data) = std::fs::read(path) {
        let sample = if data.len() > 8192 { &data[..8192] } else { &data };
        sample.contains(&0)
    } else {
        false
    }
}

/// Convert a simple glob pattern to a regex for path matching.
/// Supports: `*.ext`, `**/*.ext`, `prefix*`, `*suffix`, `prefix*suffix`
fn glob_to_regex(pattern: &str) -> regex::Regex {
    let escaped = regex::escape(pattern);
    // Replace glob wildcards with regex equivalents (in order: **/ first, then **, then *, then ?)
    let regex_str = escaped
        .replace(r"\*\*/", "<<<STARGLOB>>>/")
        .replace(r"\*\*", ".*")
        .replace("<<<STARGLOB>>>/", ".*/")
        .replace(r"\*", "[^/]*")
        .replace(r"\?", "[^/]");
    let pattern_str = format!("^{}$", regex_str);
    regex::Regex::new(&pattern_str).unwrap_or_else(|_| {
        // Fallback: match just the filename
        let name = Path::new(pattern)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(pattern);
        regex::Regex::new(&regex::escape(name).replace(r"\*", ".*")).unwrap()
    })
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

    #[tokio::test]
    async fn test_grep_binary_skip() {
        let dir = tempdir().unwrap();
        // Create a file with null bytes
        let data = vec![0u8; 100];
        fs::write(dir.path().join("binary.bin"), &data).unwrap();
        let mut f2 = fs::File::create(dir.path().join("text.txt")).unwrap();
        writeln!(f2, "hello world").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": ".*",
            "path": dir.path().to_str().unwrap(),
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        // Binary file should be skipped
        assert!(!result.content.contains("binary.bin"));
        assert!(result.content.contains("text.txt"));
    }

    #[tokio::test]
    async fn test_skip_target_like_dir() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("target/debug")).unwrap();
        let mut f1 = fs::File::create(dir.path().join("target/debug/output.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();
        let mut f2 = fs::File::create(dir.path().join("src.txt")).unwrap();
        writeln!(f2, "hello world").unwrap();

        let tool = GrepTool::new();
        let args = json!({
            "pattern": "hello",
            "path": dir.path().to_str().unwrap(),
            "output_mode": "files_with_matches"
        });

        let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
        assert!(result.success);
        // target/ should be skipped
        assert!(!result.content.contains("output.txt"));
        assert!(result.content.contains("src.txt"));
    }

    #[test]
    fn test_glob_to_regex_basic() {
        let re = glob_to_regex("*.rs");
        assert!(re.is_match("foo.rs"));
        assert!(!re.is_match("foo.txt"));
    }

    #[test]
    fn test_glob_to_regex_recursive() {
        let re = glob_to_regex("**/*.rs");
        assert!(re.is_match("src/main.rs"));
        assert!(re.is_match("src/lib.rs"));
        assert!(!re.is_match("src/main.txt"));
    }
}
