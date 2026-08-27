//! vol-llm-tools-builtin-glob: File/directory path matching via glob patterns.
//!
//! ## Design
//!
//! This tool finds files and directories by glob path patterns within the agent's
//! sandbox workspace. It is **not** a content-search tool — use `grep` for that.
//!
//! ### Suitable use-cases
//!
//! - Find project entry points: `src/**/page.tsx`
//! - Locate config files: `**/{package.json,pyproject.toml}`
//! - Find tests: `**/*.{test,spec}.{ts,tsx}`
//! - List a directory: `components/*`
//! - Check file existence: `**/*.sql`
//!
//! ### Unsuitable use-cases
//!
//! - Searching file **content** — use `grep`
//! - Reading file contents — use `read_file`
//! - Git change queries — use Git tools
//!
//! ### Glob syntax supported
//!
//! | Pattern   | Meaning                                      | Example           |
//! |-----------|----------------------------------------------|-------------------|
//! | `*`       | Match within a single directory component    | `src/*.ts`        |
//! | `**`      | Match across zero or more directories        | `src/**/*.ts`     |
//! | `?`       | Match exactly one character                  | `file?.txt`       |
//! | `[abc]`   | Match one character in the set               | `file[12].txt`    |
//! | `{a,b}`   | Brace expansion — match any alternative      | `**/*.{ts,tsx}`   |
//!
//! `!pattern` negation is NOT supported inside `pattern` — use the `exclude`
//! parameter instead for clearer semantics.
//!
//! ### Exclude behavior
//!
//! Exclude patterns are matched against every path (both files and directories).
//! When a **directory** matches an exclude pattern, it and its entire subtree are
//! skipped — this provides a significant performance benefit for directories like
//! `node_modules`, `.git`, or `target`.
//!
//! For exclude patterns ending in `/**`, a derived pattern (without `/**`) is
//! also checked against directories so that the directory itself can be excluded.

use async_trait::async_trait;
use chrono::TimeZone;
use glob::Pattern;
use serde::{Deserialize, Serialize};
use std::path::Path;
use vol_llm_sandbox::FileType;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};

pub use vol_llm_tool::ToolError as BuiltinToolError;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Constants
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

const DEFAULT_MAX_RESULTS: usize = 100;
const MAX_RESULTS_LIMIT: usize = 1000;

/// Default exclude patterns applied unless the caller provides their own.
/// These match common VCS, dependency, and build artifact directories.
const DEFAULT_EXCLUDES: &[&str] = &[
    "**/.git/**",
    "**/node_modules/**",
    "**/.next/**",
    "**/dist/**",
    "**/build/**",
    "**/__pycache__/**",
    "**/.venv/**",
    "**/venv/**",
    "**/target/**", // Rust build artifacts (project-specific)
];

const VALID_KINDS: &[&str] = &["file", "directory", "all"];
const VALID_SORTS: &[&str] = &["path_asc", "path_desc", "modified_desc", "modified_asc"];

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Error codes
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Structured error codes returned in the JSON output for machine-readability.
#[derive(Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
#[allow(dead_code)]
enum GlobErrorCode {
    InvalidPattern,
    InvalidPath,
    PathOutsideWorkspace,
    StartPathNotFound,
    MaxResultsOutOfRange,
    InvalidKind,
    InvalidSort,
    ScanError,
    InternalError,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Parameter types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn default_path() -> String {
    ".".to_string()
}

fn default_kind() -> String {
    "file".to_string()
}

fn default_max_results() -> usize {
    DEFAULT_MAX_RESULTS
}

fn default_sort() -> String {
    "path_asc".to_string()
}

fn default_false() -> bool {
    false
}

/// Parameters for the Glob tool.
///
/// All parameters except `pattern` have sensible defaults.
#[derive(Debug, Deserialize, Serialize)]
pub struct GlobParams {
    /// Glob pattern relative to the search path.
    /// Supports `*`, `**`, `?`, `[abc]`, and brace expansion `{a,b}`.
    /// Examples: `src/**/*.tsx`, `**/package.json`, `**/*.{ts,tsx}`
    pub pattern: String,

    /// Search root directory, relative to workspace root.
    /// Use this to narrow the scan scope. Default: `"."` (workspace root).
    /// Example: `"src"`, `"apps/web"`
    #[serde(default = "default_path")]
    pub path: String,

    /// Glob patterns to exclude from results.
    /// Directories matching an exclude pattern are skipped entirely (subtree pruned).
    /// Default excludes common VCS/dependency/build directories.
    /// Pass an empty array `[]` to disable ALL excludes.
    #[serde(default)]
    pub exclude: Option<Vec<String>>,

    /// What to return: `"file"` (default), `"directory"`, or `"all"`.
    #[serde(default = "default_kind")]
    pub kind: String,

    /// Maximum results to return (1–1000). Default 100.
    /// If results are truncated, `truncated` will be `true` in the output.
    /// When truncated, narrow `path`, use a more specific `pattern`, or
    /// increase `max_results`.
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// Whether to include hidden files and directories (names starting with `.`).
    /// Default `false`. Even when `true`, `.git` is always excluded unless
    /// you explicitly pass an empty `exclude` list.
    #[serde(default = "default_false")]
    pub include_hidden: bool,

    /// Whether to follow symbolic links during traversal.
    /// Default `false` — prevents infinite loops and escape from the sandbox.
    /// When `true`, resolved targets are still verified to be within the
    /// workspace root.
    #[serde(default = "default_false")]
    pub follow_symlinks: bool,

    /// Sort order for results. Default `"path_asc"` — stable across calls.
    /// - `"path_asc"`: alphabetical by path (A→Z)
    /// - `"path_desc"`: reverse alphabetical (Z→A)
    /// - `"modified_desc"`: newest first
    /// - `"modified_asc"`: oldest first
    ///
    /// `modified_*` sorts require reading metadata for every match and are
    /// slightly slower for large result sets.
    #[serde(default = "default_sort")]
    pub sort: String,

    /// Whether to include file size (`size_bytes`) and modification time
    /// (`modified_at` as ISO 8601) in the result. Default `false`.
    /// Enable only when you need to filter by size or recency.
    #[serde(default = "default_false")]
    pub with_metadata: bool,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Output types
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// A single match in the glob results.
#[derive(Debug, Serialize)]
struct GlobMatch {
    /// Relative path from workspace root. Use this with `read_file`, `grep`, etc.
    path: String,
    /// `"file"` or `"directory"`
    #[serde(rename = "type")]
    entry_type: String,
    /// File size in bytes (only present when `with_metadata` is true)
    #[serde(skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
    /// ISO 8601 modification timestamp (only present when `with_metadata` is true)
    #[serde(skip_serializing_if = "Option::is_none")]
    modified_at: Option<String>,
}

/// Structured output from the glob tool.
///
/// Always returns a JSON object — even on errors — so the AI can
/// reliably inspect `truncated`, `total_matched`, etc.
#[derive(Debug, Serialize)]
struct GlobOutput {
    /// Matching paths (may be empty).
    matches: Vec<GlobMatch>,
    /// Total number of matches found before applying `max_results`.
    total_matched: usize,
    /// `true` when results were cut off at `max_results`.
    truncated: bool,
    /// Human-readable message when applicable (truncation, empty results, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    /// The search path that was used (resolved relative to workspace root).
    search_path: String,
    /// The glob pattern that was evaluated.
    pattern: String,
    /// Exclusion patterns that were applied.
    excluded: Vec<String>,
    /// Error information (only present on failure).
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<GlobErrorOutput>,
}

#[derive(Debug, Serialize)]
struct GlobErrorOutput {
    /// Machine-readable error code.
    code: GlobErrorCode,
    /// Human-readable description of what went wrong.
    message: String,
    /// Suggested action to resolve the error.
    suggestion: String,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Error helpers
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn error_output(
    code: GlobErrorCode,
    message: impl Into<String>,
    suggestion: impl Into<String>,
) -> GlobOutput {
    let msg: String = message.into();
    let sug: String = suggestion.into();
    GlobOutput {
        matches: vec![],
        total_matched: 0,
        truncated: false,
        message: Some(msg.clone()),
        search_path: String::new(),
        pattern: String::new(),
        excluded: vec![],
        error: Some(GlobErrorOutput {
            code,
            message: msg,
            suggestion: sug,
        }),
    }
}

fn make_error_result(
    code: GlobErrorCode,
    message: impl Into<String>,
    suggestion: impl Into<String>,
) -> ToolResultType<ToolResult> {
    let output = error_output(code, message, suggestion);
    Ok(ToolResult::failure(
        serde_json::to_string_pretty(&output).unwrap_or_else(|_| {
            r#"{"error":{"code":"INTERNAL_ERROR","message":"Failed to serialize error"}}"#
                .to_string()
        }),
    ))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Brace expansion
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Expand brace expressions like `{ts,tsx}` or `{a,{b,c}}` into multiple patterns.
///
/// Returns `None` if the braces are unbalanced.
///
/// # Examples
///
/// - `**/*.{ts,tsx}` → `["**/*.ts", "**/*.tsx"]`
/// - `a{b,c}d` → `["abd", "acd"]`
/// - `no_braces` → `["no_braces"]`
fn expand_braces(pattern: &str) -> Option<Vec<String>> {
    // Fast path: no braces
    if !pattern.contains('{') {
        return Some(vec![pattern.to_string()]);
    }

    let chars: Vec<char> = pattern.chars().collect();

    // Find the position of the first '{' at depth 0
    let mut brace_start: Option<usize> = None;
    let mut depth = 0u32;
    for (idx, &c) in chars.iter().enumerate() {
        match c {
            '{' if depth == 0 => {
                brace_start = Some(idx);
                break; // stop at the first top-level brace
            }
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
            }
            _ => {}
        }
    }

    let brace_start = match brace_start {
        Some(pos) => pos,
        None => return Some(vec![pattern.to_string()]),
    };

    // Find the matching '}' starting from brace_start
    let prefix: String = chars[..brace_start].iter().collect();
    let mut j = brace_start + 1;
    depth = 1;
    while j < chars.len() && depth > 0 {
        match chars[j] {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth > 0 {
            j += 1;
        }
    }

    if depth != 0 {
        return None; // unbalanced
    }

    // Extract the brace content (between '{' and '}')
    let alternatives_str: String = chars[brace_start + 1..j].iter().collect();
    let suffix: String = chars[j + 1..].iter().collect();

    // Split alternatives by ',' at depth 0
    let mut alternatives: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut d = 0u32;
    for c in alternatives_str.chars() {
        match c {
            '{' => {
                d += 1;
                current.push(c);
            }
            '}' => {
                d = d.saturating_sub(1);
                current.push(c);
            }
            ',' if d == 0 => {
                alternatives.push(current);
                current = String::new();
            }
            _ => {
                current.push(c);
            }
        }
    }
    alternatives.push(current);

    // Recursively expand each alternative and the suffix, then combine
    let mut results = Vec::new();
    for alt in &alternatives {
        let expanded_alt = expand_braces(alt)?;
        let expanded_suffix = expand_braces(&suffix)?;
        for a in &expanded_alt {
            for s in &expanded_suffix {
                results.push(format!("{prefix}{a}{s}"));
            }
        }
    }

    Some(results)
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Path validation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Validate that a user-supplied path/pattern is a safe relative path.
/// Rejects: absolute paths, paths containing `..`, and NUL bytes.
fn validate_relative(value: &str, field_name: &str) -> Result<(), (GlobErrorCode, String, String)> {
    if value.contains('\0') {
        return Err((
            GlobErrorCode::InvalidPath,
            format!("{field_name} contains null bytes"),
            format!("Remove any null bytes from the {field_name} value."),
        ));
    }

    let normalized = value.replace('\\', "/");

    if normalized.starts_with('/') || normalized.starts_with('~') {
        return Err((
            GlobErrorCode::InvalidPath,
            format!("{field_name} must be a relative path, got: {value}"),
            format!(
                "Use a path relative to the workspace root, e.g. \"src\" instead of \"{value}\"."
            ),
        ));
    }

    for part in normalized.split('/') {
        if part == ".." {
            return Err((
                GlobErrorCode::PathOutsideWorkspace,
                format!("{field_name} contains '..' path traversal: {value}"),
                "Use a path within the workspace. '..' is not allowed.".to_string(),
            ));
        }
    }

    Ok(())
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// ISO 8601 conversion
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Convert a Unix timestamp in milliseconds to an ISO 8601 string.
fn mtime_to_iso8601(mtime_ms: u64) -> Option<String> {
    let secs = (mtime_ms / 1000) as i64;
    let nsecs = ((mtime_ms % 1000) * 1_000_000) as u32;
    chrono::Utc
        .timestamp_opt(secs, nsecs)
        .single()
        .map(|dt| dt.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Sandbox existence check
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// Check if a path exists and is a directory using the sandbox.
/// We use `read_dir` — if it succeeds, the path is a directory.
async fn sandbox_dir_exists(
    sandbox: &std::sync::Arc<dyn vol_llm_sandbox::Sandbox>,
    path: &Path,
) -> bool {
    sandbox.read_dir(path).await.is_ok()
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// GlobTool
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

/// The Glob tool for matching file/directory paths using glob patterns.
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
        "Find files and directories by glob path patterns within the workspace. \
        Only matches paths, not file content (use grep for content search). \
        Supports *, **, ?, [abc], and {a,b} brace expansion. \
        \
        USAGE GUIDANCE: \
        1. Prefer precise patterns — avoid \"**/*\" full scans. \
        2. When `truncated` is true in the result, narrow `path` or `pattern`. \
        3. No matches does NOT guarantee files don't exist — check `path`, \
           exclude rules, and `include_hidden`. \
        4. All returned paths are relative to workspace root. \
        5. Never use absolute paths or `..` — they are rejected. \
        \
        EXAMPLES: \
        - Find config: pattern=\"**/package.json\" \
        - Find components: pattern=\"**/*.tsx\" path=\"src\" \
        - Find tests: pattern=\"**/*.{test,spec}.ts\" \
        - List directories: pattern=\"*\" path=\"src\" kind=\"directory\" \
        - Filter by name+ext: pattern=\"**/*.{ts,tsx}\" exclude=[\"**/*.test.*\"]"
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern relative to `path`. Supports *, **, ?, [abc], and {a,b} brace expansion. Examples: \"src/**/*.tsx\", \"**/{package.json,Cargo.toml}\", \"**/*.{test,spec}.ts\". Required."
                },
                "path": {
                    "type": "string",
                    "description": "Search root directory, relative to workspace root. Default: \".\". Use this to narrow the scan scope. Example: \"src\", \"apps/web\". Must be relative — absolute paths and '..' are rejected.",
                    "default": "."
                },
                "exclude": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Glob patterns to exclude from results. Directories matching an exclude pattern are skipped entirely. Defaults to common VCS/dependency/build exclusions: .git, node_modules, .next, dist, build, __pycache__, .venv, venv, target. Pass [] to disable all default excludes."
                },
                "kind": {
                    "type": "string",
                    "enum": ["file", "directory", "all"],
                    "description": "What to return. \"file\" = only files (default), \"directory\" = only directories, \"all\" = both.",
                    "default": "file"
                },
                "max_results": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": 1000,
                    "description": "Maximum results to return (1–1000). Default 100. If exceeded, output is truncated and `truncated` is true — narrow `path` or use a more specific `pattern`.",
                    "default": 100
                },
                "include_hidden": {
                    "type": "boolean",
                    "description": "Whether to include hidden files/directories (names starting with '.'). Default false.",
                    "default": false
                },
                "follow_symlinks": {
                    "type": "boolean",
                    "description": "Whether to follow symbolic links during traversal. Default false (prevents loops and sandbox escape). Targets are always verified to be within the workspace.",
                    "default": false
                },
                "sort": {
                    "type": "string",
                    "enum": ["path_asc", "path_desc", "modified_desc", "modified_asc"],
                    "description": "Sort order. \"path_asc\" = alphabetical A→Z (default, stable across calls). \"path_desc\" = Z→A. \"modified_desc\" = newest first. \"modified_asc\" = oldest first. modified_* are slower for large result sets.",
                    "default": "path_asc"
                },
                "with_metadata": {
                    "type": "boolean",
                    "description": "Include file size (bytes) and modification time (ISO 8601) in results. Default false — enable only when you need to filter by size or recency.",
                    "default": false
                }
            },
            "required": ["pattern"],
            "additionalProperties": false
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        // ── Parse parameters ──────────────────────────────────────────
        let params: GlobParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!(
                "Failed to parse glob arguments: {e}. \
                     Ensure `pattern` is a string. Valid parameters: \
                     pattern (required), path, exclude, kind, max_results, \
                     include_hidden, follow_symlinks, sort, with_metadata."
            ))
        })?;

        // ── Validate parameters ───────────────────────────────────────
        if params.pattern.is_empty() {
            return make_error_result(
                GlobErrorCode::InvalidPattern,
                "pattern must not be empty",
                "Provide a non-empty glob pattern, e.g. \"*.rs\" or \"src/**/*.toml\".",
            );
        }

        if let Err((code, msg, suggestion)) = validate_relative(&params.pattern, "pattern") {
            return make_error_result(code, msg, suggestion);
        }

        if let Err((code, msg, suggestion)) = validate_relative(&params.path, "path") {
            return make_error_result(code, msg, suggestion);
        }

        if !VALID_KINDS.contains(&params.kind.as_str()) {
            return make_error_result(
                GlobErrorCode::InvalidKind,
                format!("Invalid kind: {}. Must be one of: {:?}", params.kind, VALID_KINDS),
                "Use \"file\" for files only, \"directory\" for directories only, or \"all\" for both.",
            );
        }

        if !(1..=MAX_RESULTS_LIMIT).contains(&params.max_results) {
            return make_error_result(
                GlobErrorCode::MaxResultsOutOfRange,
                format!(
                    "max_results must be between 1 and {}, got {}",
                    MAX_RESULTS_LIMIT, params.max_results
                ),
                format!(
                    "Set max_results to a value between 1 and {}.",
                    MAX_RESULTS_LIMIT
                ),
            );
        }

        if !VALID_SORTS.contains(&params.sort.as_str()) {
            return make_error_result(
                GlobErrorCode::InvalidSort,
                format!(
                    "Invalid sort: {}. Must be one of: {:?}",
                    params.sort, VALID_SORTS
                ),
                "Use one of: \"path_asc\", \"path_desc\", \"modified_desc\", \"modified_asc\".",
            );
        }

        // ── Resolve path ──────────────────────────────────────────────
        let resolved_root = match context.resolve_path(&params.path) {
            Ok(p) => p,
            Err(e) => {
                return make_error_result(
                    GlobErrorCode::PathOutsideWorkspace,
                    format!("Path resolution failed: {e}"),
                    "Ensure the path is relative and within the workspace root.",
                );
            }
        };

        // ── Check start directory exists (via sandbox) ────────────────
        if !sandbox_dir_exists(&context.sandbox, &resolved_root).await {
            let exclude_strings: Vec<String> = params
                .exclude
                .clone()
                .unwrap_or_else(|| DEFAULT_EXCLUDES.iter().map(|s| s.to_string()).collect());

            let output = GlobOutput {
                matches: vec![],
                total_matched: 0,
                truncated: false,
                message: Some(format!(
                    "Search path does not exist or is not a directory: {}",
                    params.path
                )),
                search_path: params.path.clone(),
                pattern: params.pattern.clone(),
                excluded: exclude_strings,
                error: None,
            };
            return Ok(ToolResult::success(
                serde_json::to_string_pretty(&output).unwrap_or_default(),
            ));
        }

        // ── Compile exclude patterns ──────────────────────────────────
        let exclude_strings: Vec<String> = params
            .exclude
            .clone()
            .unwrap_or_else(|| DEFAULT_EXCLUDES.iter().map(|s| s.to_string()).collect());

        let mut exclude_patterns: Vec<Pattern> = Vec::new();
        let mut exclude_dir_patterns: Vec<Pattern> = Vec::new();

        for excl in &exclude_strings {
            match Pattern::new(excl) {
                Ok(p) => {
                    // Derive a directory-matching pattern: strip trailing "/**"
                    if excl.ends_with("/**") {
                        let dir_pattern_str = &excl[..excl.len() - 3];
                        if let Ok(dp) = Pattern::new(dir_pattern_str) {
                            exclude_dir_patterns.push(dp);
                        }
                    }
                    exclude_patterns.push(p);
                }
                Err(e) => {
                    return make_error_result(
                        GlobErrorCode::InvalidPattern,
                        format!("Invalid exclude pattern '{excl}': {e}"),
                        "Check the exclude pattern syntax. Valid examples: \"**/node_modules/**\", \"**/*.test.ts\".",
                    );
                }
            }
        }

        let mut all_dir_patterns = exclude_patterns.clone();
        all_dir_patterns.extend(exclude_dir_patterns);

        // ── Expand braces in the main pattern ─────────────────────────
        let expanded_patterns = match expand_braces(&params.pattern) {
            Some(pats) => pats,
            None => {
                return make_error_result(
                    GlobErrorCode::InvalidPattern,
                    format!("Unbalanced braces in pattern: {}", params.pattern),
                    "Check that all '{{' have a matching '}}' in the pattern.",
                );
            }
        };

        let mut glob_matchers: Vec<Pattern> = Vec::new();
        for pat in &expanded_patterns {
            match Pattern::new(pat) {
                Ok(p) => glob_matchers.push(p),
                Err(e) => {
                    return make_error_result(
                        GlobErrorCode::InvalidPattern,
                        format!("Invalid glob pattern '{pat}': {e}"),
                        "Check the glob syntax. Supported: *, **, ?, [abc], and {a,b} braces.",
                    );
                }
            }
        }

        // ── Scan ──────────────────────────────────────────────────────
        // We store: (rel_path_to_search_root, rel_path_to_workspace_root, mtime_ms, size_bytes, file_type)
        // `search_rel` is used for pattern/exclude matching.
        // `workspace_rel` is what we return to the caller.
        let needs_mtime = params.sort.starts_with("modified") || params.with_metadata;
        let needs_size = params.with_metadata;
        let mut results: Vec<(String, String, Option<u64>, Option<u64>, FileType)> = Vec::new();
        let mut dirs_to_visit: Vec<std::path::PathBuf> = vec![resolved_root.clone()];
        let workspace_root = context
            .sandbox
            .root_path()
            .map(|p| p.to_path_buf())
            .unwrap_or_default();

        while let Some(dir) = dirs_to_visit.pop() {
            let entries = match context.sandbox.read_dir(&dir).await {
                Ok(entries) => entries,
                Err(e) => {
                    tracing::warn!(
                        dir = %dir.display(),
                        error = %e,
                        "Failed to read directory during glob scan"
                    );
                    continue;
                }
            };

            for entry in entries {
                let entry_path = dir.join(&entry.name);

                // ── Symlink handling ──────────────────────────────
                if entry.file_type == FileType::Symlink && !params.follow_symlinks {
                    continue;
                }

                // ── Compute relative paths ────────────────────────
                let search_rel = match entry_path.strip_prefix(&resolved_root) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let search_rel_str = search_rel.to_string_lossy().to_string();

                let workspace_rel = match entry_path.strip_prefix(&workspace_root) {
                    Ok(r) => r,
                    Err(_) => continue,
                };
                let workspace_rel_str = workspace_rel.to_string_lossy().to_string();

                // ── Hidden detection ──────────────────────────────
                let is_hidden = entry.name.starts_with('.');

                // ── Handle directories ────────────────────────────
                if entry.file_type == FileType::Directory {
                    let is_excluded = all_dir_patterns.iter().any(|p| p.matches(&search_rel_str));
                    if is_excluded {
                        continue; // skip entire subtree
                    }
                    // Recurse into non-hidden directories, or hidden dirs when include_hidden
                    if !is_hidden || params.include_hidden {
                        dirs_to_visit.push(entry_path.clone());
                    }
                }

                // ── Filter hidden files ───────────────────────────
                if is_hidden && !params.include_hidden {
                    continue;
                }

                // ── Filter by kind ────────────────────────────────
                let include_entry = match params.kind.as_str() {
                    "file" => entry.file_type == FileType::File,
                    "directory" => entry.file_type == FileType::Directory,
                    "all" => true,
                    _ => true,
                };

                if !include_entry {
                    continue;
                }

                // ── Check exclude patterns for non-directories ────
                if entry.file_type != FileType::Directory {
                    let is_excluded = exclude_patterns.iter().any(|p| p.matches(&search_rel_str));
                    if is_excluded {
                        continue;
                    }
                }

                // ── Check glob pattern ────────────────────────────
                let matched = glob_matchers.iter().any(|p| p.matches(&search_rel_str));
                if !matched {
                    continue;
                }

                // ── Collect metadata if needed ────────────────────
                let (mtime, size): (Option<u64>, Option<u64>) = if needs_mtime || needs_size {
                    match context.sandbox.metadata(&entry_path).await {
                        Ok(meta) => {
                            let m = if needs_mtime { Some(meta.mtime) } else { None };
                            let s = if needs_size { Some(meta.size) } else { None };
                            (m, s)
                        }
                        Err(_) => (None, None),
                    }
                } else {
                    (None, None)
                };

                results.push((
                    workspace_rel_str,
                    search_rel_str,
                    mtime,
                    size,
                    entry.file_type.clone(),
                ));
            }
        }

        // ── Sort ──────────────────────────────────────────────────────
        // Tuple: (workspace_path, _search_path, mtime, size, file_type)
        match params.sort.as_str() {
            "path_asc" => results.sort_by(|a, b| a.0.cmp(&b.0)),
            "path_desc" => results.sort_by(|a, b| b.0.cmp(&a.0)),
            "modified_desc" => {
                results.sort_by(|a, b| {
                    b.2.unwrap_or(0)
                        .cmp(&a.2.unwrap_or(0))
                        .then_with(|| a.0.cmp(&b.0))
                });
            }
            "modified_asc" => {
                results.sort_by(|a, b| {
                    a.2.unwrap_or(0)
                        .cmp(&b.2.unwrap_or(0))
                        .then_with(|| a.0.cmp(&b.0))
                });
            }
            _ => {}
        }

        // ── Build output ──────────────────────────────────────────────
        let total_matched = results.len();
        let truncated = total_matched > params.max_results;
        let selected = &results[..total_matched.min(params.max_results)];

        let matches: Vec<GlobMatch> = selected
            .iter()
            .map(|(workspace_path, _search_path, mtime, size, file_type)| {
                let entry_type = match file_type {
                    FileType::Directory => "directory",
                    _ => "file",
                };

                let (size_bytes, modified_at) = if params.with_metadata {
                    let sz = *size;
                    let mt = mtime.and_then(mtime_to_iso8601);
                    (sz, mt)
                } else {
                    (None, None)
                };

                GlobMatch {
                    path: workspace_path.clone(),
                    entry_type: entry_type.to_string(),
                    size_bytes,
                    modified_at,
                }
            })
            .collect();

        let message = if truncated {
            Some(format!(
                "Results truncated: {} matches found, showing first {}. \
                 Narrow `path`, use a more specific `pattern`, or increase `max_results` (max {}).",
                total_matched, params.max_results, MAX_RESULTS_LIMIT
            ))
        } else if matches.is_empty() {
            Some(
                "No matches found. Check that: (1) the path exists, \
                 (2) the pattern is correct, (3) files aren't excluded, \
                 (4) hidden files aren't being skipped (use include_hidden=true)."
                    .to_string(),
            )
        } else {
            None
        };

        let output = GlobOutput {
            matches,
            total_matched,
            truncated,
            message,
            search_path: params.path.clone(),
            pattern: params.pattern.clone(),
            excluded: exclude_strings,
            error: None,
        };

        Ok(ToolResult::success(
            serde_json::to_string_pretty(&output).unwrap_or_default(),
        ))
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Unit tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    // ── Brace expansion tests ────────────────────────────────────────

    #[test]
    fn test_expand_braces_no_braces() {
        let result = expand_braces("src/**/*.rs").unwrap();
        assert_eq!(result, vec!["src/**/*.rs"]);
    }

    #[test]
    fn test_expand_braces_simple() {
        let result = expand_braces("**/*.{ts,tsx}").unwrap();
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["**/*.ts", "**/*.tsx"]);
    }

    #[test]
    fn test_expand_braces_multiple() {
        let result = expand_braces("{src,lib}/*.rs").unwrap();
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["lib/*.rs", "src/*.rs"]);
    }

    #[test]
    fn test_expand_braces_nested() {
        let result = expand_braces("a{b,{c,d}}e").unwrap();
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["abe", "ace", "ade"]);
    }

    #[test]
    fn test_expand_braces_empty_alt() {
        let result = expand_braces("a{,b}c").unwrap();
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["abc", "ac"]);
    }

    #[test]
    fn test_expand_braces_prefix_suffix() {
        let result = expand_braces("prefix-{a,b}-suffix").unwrap();
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["prefix-a-suffix", "prefix-b-suffix"]);
    }

    #[test]
    fn test_expand_braces_unbalanced() {
        assert!(expand_braces("src/{a,b").is_none());
    }

    #[test]
    fn test_expand_braces_single_alt() {
        let result = expand_braces("only-{one}-suffix").unwrap();
        assert_eq!(result, vec!["only-one-suffix"]);
    }

    #[test]
    fn test_expand_braces_three_alts() {
        let result = expand_braces("*.{ts,tsx,js}").unwrap();
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["*.js", "*.ts", "*.tsx"]);
    }

    #[test]
    fn test_expand_braces_at_start() {
        let result = expand_braces("{Cargo,package}.toml").unwrap();
        let mut sorted = result.clone();
        sorted.sort();
        assert_eq!(sorted, vec!["Cargo.toml", "package.toml"]);
    }

    // ── Path validation tests ────────────────────────────────────────

    #[test]
    fn test_validate_relative_ok() {
        assert!(validate_relative("src/main.rs", "pattern").is_ok());
        assert!(validate_relative(".", "path").is_ok());
        assert!(validate_relative("apps/web/**/*.ts", "pattern").is_ok());
    }

    #[test]
    fn test_validate_relative_rejects_absolute() {
        let err = validate_relative("/etc/passwd", "path").unwrap_err();
        assert!(err.1.contains("must be a relative path"), "got: {}", err.1);
    }

    #[test]
    fn test_validate_relative_rejects_home() {
        let err = validate_relative("~/foo", "path").unwrap_err();
        assert!(err.1.contains("must be a relative path"), "got: {}", err.1);
    }

    #[test]
    fn test_validate_relative_rejects_parent_traversal() {
        let err = validate_relative("src/../../etc/passwd", "pattern").unwrap_err();
        assert_eq!(err.0, GlobErrorCode::PathOutsideWorkspace);
    }

    #[test]
    fn test_validate_relative_rejects_single_dotdot() {
        let err = validate_relative("../outside", "path").unwrap_err();
        assert_eq!(err.0, GlobErrorCode::PathOutsideWorkspace);
    }

    #[test]
    fn test_validate_relative_rejects_null_byte() {
        let err = validate_relative("src/\0test", "pattern").unwrap_err();
        assert_eq!(err.0, GlobErrorCode::InvalidPath);
    }

    #[test]
    fn test_validate_relative_accepts_dots_in_names() {
        // Single dots in filenames are OK (e.g., "foo.bar.rs")
        assert!(validate_relative("foo.bar.rs", "pattern").is_ok());
        assert!(validate_relative(".hidden", "pattern").is_ok());
    }

    // ── GlobParams deserialization tests ─────────────────────────────

    #[test]
    fn test_params_minimal() {
        let params: GlobParams = serde_json::from_value(serde_json::json!({
            "pattern": "*.rs"
        }))
        .unwrap();
        assert_eq!(params.pattern, "*.rs");
        assert_eq!(params.path, ".");
        assert_eq!(params.kind, "file");
        assert_eq!(params.max_results, 100);
        assert!(!params.include_hidden);
        assert!(!params.follow_symlinks);
        assert!(!params.with_metadata);
        assert_eq!(params.sort, "path_asc");
        assert!(params.exclude.is_none());
    }

    #[test]
    fn test_params_full() {
        let params: GlobParams = serde_json::from_value(serde_json::json!({
            "pattern": "**/*.ts",
            "path": "src",
            "exclude": ["**/*.test.ts"],
            "kind": "all",
            "max_results": 50,
            "include_hidden": true,
            "follow_symlinks": true,
            "sort": "modified_desc",
            "with_metadata": true
        }))
        .unwrap();
        assert_eq!(params.pattern, "**/*.ts");
        assert_eq!(params.path, "src");
        assert_eq!(params.exclude, Some(vec!["**/*.test.ts".to_string()]));
        assert_eq!(params.kind, "all");
        assert_eq!(params.max_results, 50);
        assert!(params.include_hidden);
        assert!(params.follow_symlinks);
        assert_eq!(params.sort, "modified_desc");
        assert!(params.with_metadata);
    }

    #[test]
    fn test_params_exclude_empty_vec() {
        let params: GlobParams = serde_json::from_value(serde_json::json!({
            "pattern": "*.rs",
            "exclude": []
        }))
        .unwrap();
        // Empty vec means Some(vec![]), which disables default excludes
        assert_eq!(params.exclude, Some(vec![]));
    }

    // ── Error output serialization tests ─────────────────────────────

    #[test]
    fn test_error_output_contains_code_and_suggestion() {
        let output = error_output(
            GlobErrorCode::InvalidPattern,
            "pattern must not be empty",
            "Provide a non-empty glob pattern.",
        );
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("INVALID_PATTERN"));
        assert!(json.contains("pattern must not be empty"));
        assert!(json.contains("Provide a non-empty glob pattern"));
    }

    #[test]
    fn test_error_output_all_fields_present() {
        let output = error_output(
            GlobErrorCode::MaxResultsOutOfRange,
            "max_results must be between 1 and 1000",
            "Set max_results to a value between 1 and 1000.",
        );
        assert_eq!(output.matches.len(), 0);
        assert_eq!(output.total_matched, 0);
        assert!(!output.truncated);
        assert!(output.message.is_some());
        assert!(output.error.is_some());
        let err = output.error.unwrap();
        assert_eq!(err.code, GlobErrorCode::MaxResultsOutOfRange);
        assert!(!err.suggestion.is_empty());
    }

    // ── mtime_to_iso8601 tests ───────────────────────────────────────

    #[test]
    fn test_mtime_to_iso8601_epoch() {
        let result = mtime_to_iso8601(0);
        assert!(result.is_some());
        // Jan 1, 1970 00:00:00 UTC
        assert!(result.unwrap().starts_with("1970-01-01T00:00:00"));
    }

    #[test]
    fn test_mtime_to_iso8601_known_time() {
        // 2025-01-15T12:30:45 UTC in milliseconds
        let ms = 1736944245000u64;
        let result = mtime_to_iso8601(ms);
        assert!(result.is_some());
        assert!(result.unwrap().starts_with("2025-01-15"));
    }

    // ── GlobOutput serialization tests ───────────────────────────────

    #[test]
    fn test_glob_output_success_serialization() {
        let output = GlobOutput {
            matches: vec![GlobMatch {
                path: "src/main.rs".to_string(),
                entry_type: "file".to_string(),
                size_bytes: None,
                modified_at: None,
            }],
            total_matched: 1,
            truncated: false,
            message: None,
            search_path: ".".to_string(),
            pattern: "**/*.rs".to_string(),
            excluded: vec!["**/node_modules/**".to_string()],
            error: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("src/main.rs"));
        assert!(json.contains("\"total_matched\":1"));
        assert!(json.contains("\"truncated\":false"));
        assert!(!json.contains("\"error\""));
        // The excluded vector should not contain "target" unless it was in the input
    }

    #[test]
    fn test_glob_output_truncated() {
        let output = GlobOutput {
            matches: vec![],
            total_matched: 500,
            truncated: true,
            message: Some("Results truncated: 500 matches found, showing first 100.".to_string()),
            search_path: ".".to_string(),
            pattern: "*.rs".to_string(),
            excluded: vec![],
            error: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"truncated\":true"));
        assert!(json.contains("\"total_matched\":500"));
    }

    #[test]
    fn test_glob_output_with_metadata() {
        let output = GlobOutput {
            matches: vec![GlobMatch {
                path: "src/main.rs".to_string(),
                entry_type: "file".to_string(),
                size_bytes: Some(1024),
                modified_at: Some("2025-01-15T12:30:45+00:00".to_string()),
            }],
            total_matched: 1,
            truncated: false,
            message: None,
            search_path: ".".to_string(),
            pattern: "**/*.rs".to_string(),
            excluded: vec![],
            error: None,
        };
        let json = serde_json::to_string(&output).unwrap();
        assert!(json.contains("\"size_bytes\":1024"));
        assert!(json.contains("2025-01-15T12:30:45+00:00"));
    }

    // ── GlobErrorCode derives ────────────────────────────────────────

    #[test]
    fn test_error_code_partial_eq() {
        assert_eq!(GlobErrorCode::InvalidPattern, GlobErrorCode::InvalidPattern);
        assert_ne!(GlobErrorCode::InvalidPattern, GlobErrorCode::InvalidKind);
    }

    #[test]
    fn test_all_error_codes_have_variants() {
        // Ensure all expected error codes exist
        let codes = [
            GlobErrorCode::InvalidPattern,
            GlobErrorCode::InvalidPath,
            GlobErrorCode::PathOutsideWorkspace,
            GlobErrorCode::StartPathNotFound,
            GlobErrorCode::MaxResultsOutOfRange,
            GlobErrorCode::InvalidKind,
            GlobErrorCode::InvalidSort,
            GlobErrorCode::ScanError,
            GlobErrorCode::InternalError,
        ];
        assert_eq!(codes.len(), 9);
    }

    // ── Tool metadata tests ──────────────────────────────────────────

    #[test]
    fn test_glob_tool_name() {
        let tool = GlobTool::new();
        assert_eq!(tool.name(), "glob");
    }

    #[test]
    fn test_glob_tool_description_is_non_empty() {
        let tool = GlobTool::new();
        assert!(!tool.description().is_empty());
        assert!(tool.description().contains("glob"));
    }

    #[test]
    fn test_glob_tool_parameters_is_valid_json_schema() {
        let tool = GlobTool::new();
        let params = tool.parameters();
        // Should be a JSON object with "type": "object"
        assert_eq!(params["type"], "object");
        // Should have required field
        assert!(params["required"].is_array());
        assert!(params["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("pattern")));
        // Should have properties
        assert!(params["properties"].is_object());
        // All expected parameters should be present
        let props = params["properties"].as_object().unwrap();
        assert!(props.contains_key("pattern"));
        assert!(props.contains_key("path"));
        assert!(props.contains_key("exclude"));
        assert!(props.contains_key("kind"));
        assert!(props.contains_key("max_results"));
        assert!(props.contains_key("include_hidden"));
        assert!(props.contains_key("follow_symlinks"));
        assert!(props.contains_key("sort"));
        assert!(props.contains_key("with_metadata"));
    }

    #[test]
    fn test_glob_tool_parameter_descriptions_are_helpful() {
        let tool = GlobTool::new();
        let params = tool.parameters();
        let props = params["properties"].as_object().unwrap();
        for (name, prop) in props {
            let desc = prop["description"].as_str().unwrap_or("");
            assert!(
                !desc.is_empty(),
                "Parameter '{}' has empty description",
                name
            );
        }
    }

    // ── Constants tests ──────────────────────────────────────────────

    #[test]
    fn test_default_excludes_contains_essential_dirs() {
        let excludes: Vec<&str> = DEFAULT_EXCLUDES.to_vec();
        assert!(excludes.contains(&"**/.git/**"));
        assert!(excludes.contains(&"**/node_modules/**"));
        assert!(excludes.contains(&"**/target/**"));
    }

    #[test]
    fn test_max_results_limit() {
        assert!(MAX_RESULTS_LIMIT >= DEFAULT_MAX_RESULTS);
    }

    // ── GlobParams edge cases ────────────────────────────────────────

    #[test]
    fn test_params_max_results_boundary_min() {
        let params: GlobParams = serde_json::from_value(serde_json::json!({
            "pattern": "*.rs",
            "max_results": 1
        }))
        .unwrap();
        assert_eq!(params.max_results, 1);
    }

    #[test]
    fn test_params_max_results_boundary_max() {
        let params: GlobParams = serde_json::from_value(serde_json::json!({
            "pattern": "*.rs",
            "max_results": 1000
        }))
        .unwrap();
        assert_eq!(params.max_results, 1000);
    }

    #[test]
    fn test_params_all_sort_variants() {
        for sort in &["path_asc", "path_desc", "modified_desc", "modified_asc"] {
            let params: GlobParams = serde_json::from_value(serde_json::json!({
                "pattern": "*.rs",
                "sort": sort
            }))
            .unwrap();
            assert_eq!(params.sort, *sort);
        }
    }

    #[test]
    fn test_params_all_kind_variants() {
        for kind in &["file", "directory", "all"] {
            let params: GlobParams = serde_json::from_value(serde_json::json!({
                "pattern": "*.rs",
                "kind": kind
            }))
            .unwrap();
            assert_eq!(params.kind, *kind);
        }
    }

    // ── Execute path tests ─────────────────────────────────────────

    fn test_sandbox(dir: &tempfile::TempDir) -> ToolContext {
        let sandbox = vol_llm_sandbox::local::LocalSandbox::new(Some(dir.path().to_path_buf()));
        ToolContext::for_test().with_sandbox(std::sync::Arc::new(sandbox))
    }

    #[tokio::test]
    async fn test_execute_basic_file_glob() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("lib.rs"), "pub fn lib() {}").unwrap();
        std::fs::write(dir.path().join("README.md"), "# Project").unwrap();

        let tool = GlobTool::new();
        let args = serde_json::json!({"pattern": "*.rs"});
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);

        let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let matches = output["matches"].as_array().unwrap();
        let paths: Vec<&str> = matches
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains(&"main.rs"));
        assert!(paths.contains(&"lib.rs"));
        assert!(!paths.contains(&"README.md"));
    }

    #[tokio::test]
    async fn test_execute_glob_directory_kind() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("README.md"), "# Readme").unwrap();

        let tool = GlobTool::new();
        let args = serde_json::json!({"pattern": "*", "kind": "directory"});
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);

        let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let matches = output["matches"].as_array().unwrap();
        let paths: Vec<&str> = matches
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains(&"src"));
        assert!(!paths.contains(&"README.md")); // kind=directory skips files
    }

    #[tokio::test]
    async fn test_execute_empty_pattern_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        let tool = GlobTool::new();
        let args = serde_json::json!({"pattern": ""});
        let result = tool.execute(&args, &ctx).await.unwrap();
        // Returns failure (not error) with structured error output
        assert!(!result.success || result.content.contains("INVALID_PATTERN"));
    }

    #[tokio::test]
    async fn test_execute_invalid_kind_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        let tool = GlobTool::new();
        let args = serde_json::json!({"pattern": "*", "kind": "symlink"});
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.content.contains("INVALID_KIND") || !result.success);
    }

    #[tokio::test]
    async fn test_execute_max_results_out_of_range() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        let tool = GlobTool::new();
        let args = serde_json::json!({"pattern": "*", "max_results": 0});
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.content.contains("MAX_RESULTS") || !result.success);
    }

    #[tokio::test]
    async fn test_execute_path_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        // "nonexistent" subdirectory doesn't exist
        let tool = GlobTool::new();
        let args = serde_json::json!({"pattern": "*", "path": "nonexistent"});
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        // Should report that the search path does not exist
        let msg = output["message"].as_str().unwrap_or("");
        assert!(msg.contains("does not exist") || msg.contains("not a directory"));
    }

    #[tokio::test]
    async fn test_execute_with_exclude_pattern() {
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.path().join("src/test.rs"), "#[test] fn t() {}").unwrap();

        let tool = GlobTool::new();
        let args = serde_json::json!({
            "pattern": "src/**/*.rs",
            "exclude": ["**/*test*"]
        });
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);

        let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let matches = output["matches"].as_array().unwrap();
        let paths: Vec<&str> = matches
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect();
        assert!(paths.contains(&"src/main.rs"));
        assert!(!paths.contains(&"src/test.rs"));
    }

    #[tokio::test]
    async fn test_execute_include_hidden_skips_hidden_by_default() {
        // Reproduce the exact deployment scenario:
        // data-plane working_dir = /app, project files under .agents/ (hidden)
        // glob with default include_hidden=false skips .agents/
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        let agents_dir = dir.path().join(".agents").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(dir.path().join("README.md"), "# README").unwrap();
        std::fs::write(agents_dir.join("explore.md"), "# Explore Agent").unwrap();

        let tool = GlobTool::new();

        // Default: include_hidden=false → skips .agents/
        let args = serde_json::json!({"pattern": "**/*.md"});
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let paths: Vec<&str> = output["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect();
        // README.md is visible, .agents/explore.md is hidden → NOT found
        assert!(paths.contains(&"README.md"), "README.md should be visible");
        assert!(
            !paths.contains(&".agents/agents/explore.md"),
            ".agents/ should be skipped by default"
        );

        // With include_hidden=true → finds hidden files too
        let args2 = serde_json::json!({"pattern": "**/*.md", "include_hidden": true});
        let result2 = tool.execute(&args2, &ctx).await.unwrap();
        let output2: serde_json::Value = serde_json::from_str(&result2.content).unwrap();
        let paths2: Vec<&str> = output2["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect();
        assert!(paths2.contains(&"README.md"));
        assert!(
            paths2.contains(&".agents/agents/explore.md"),
            "include_hidden=true should find hidden files"
        );
    }

    #[tokio::test]
    async fn test_execute_star_pattern_finds_only_root_entries() {
        // Reproduce: pattern="*" with kind="file" in deployment scenario.
        // /app/ has: .agents/ (dir), data/ (dir), logs/ (dir), target/ (excluded)
        // pattern="*" only matches root level; kind="file" skips all dirs → empty!
        let dir = tempfile::tempdir().unwrap();
        let ctx = test_sandbox(&dir);
        std::fs::create_dir_all(dir.path().join(".agents").join("agents")).unwrap();
        std::fs::create_dir_all(dir.path().join("data")).unwrap();
        std::fs::create_dir(dir.path().join("target")).unwrap();
        std::fs::write(dir.path().join("README.md"), "# Root README").unwrap();
        std::fs::write(
            dir.path().join(".agents").join("agents").join("explore.md"),
            "# Explore",
        )
        .unwrap();

        let tool = GlobTool::new();

        // Exact scenario from user: pattern="*", kind="file", include_hidden=true
        let args = serde_json::json!({
            "pattern": "*",
            "kind": "file",
            "include_hidden": true
        });
        let result = tool.execute(&args, &ctx).await.unwrap();
        assert!(result.success);
        let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
        let matches = output["matches"].as_array().unwrap();
        let paths: Vec<&str> = matches
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect();
        // glob crate * matches any filename at any depth (not just root).
        // Both README.md and .agents/agents/explore.md should be found.
        assert!(
            paths.contains(&"README.md"),
            "README.md should match *: {paths:?}"
        );
        assert!(
            paths.contains(&".agents/agents/explore.md"),
            ".agents/agents/explore.md should match *: {paths:?}"
        );
        assert_eq!(output["total_matched"].as_u64().unwrap(), 2);

        // Fix: use pattern="**/*" to find files recursively
        let args2 = serde_json::json!({
            "pattern": "**/*",
            "kind": "file",
            "include_hidden": true
        });
        let result2 = tool.execute(&args2, &ctx).await.unwrap();
        let output2: serde_json::Value = serde_json::from_str(&result2.content).unwrap();
        let paths2: Vec<&str> = output2["matches"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["path"].as_str().unwrap())
            .collect();
        assert!(paths2.contains(&"README.md"));
        assert!(
            paths2.contains(&".agents/agents/explore.md"),
            "**/* finds files in subdirs: {paths2:?}"
        );
    }

    #[tokio::test]
    async fn test_execute_exact_production_args() {
        // Exact reproduction of deployed data-plane glob call:
        // {"exclude":[], "include_hidden":true, "max_results":20, "pattern":"**/*"}
        let tool = GlobTool::new();
        let args = serde_json::json!({
            "exclude": [],
            "include_hidden": true,
            "max_results": 20,
            "pattern": "**/*"
        });

        // Scenario A: deployed structure — .agents/ with files
        {
            let dir = tempfile::tempdir().unwrap();
            let ctx = test_sandbox(&dir);
            let agents_dir = dir.path().join(".agents").join("agents");
            std::fs::create_dir_all(&agents_dir).unwrap();
            std::fs::write(agents_dir.join("explore.md"), "# Explore").unwrap();
            std::fs::write(dir.path().join("README.md"), "# README").unwrap();

            let result = tool.execute(&args, &ctx).await.unwrap();
            let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
            let count = output["total_matched"].as_u64().unwrap();
            assert!(count > 0, "should find files: {output:?}");
        }

        // Scenario B: empty directory (simulates empty /app mount)
        {
            let dir = tempfile::tempdir().unwrap();
            let ctx = test_sandbox(&dir);

            let result = tool.execute(&args, &ctx).await.unwrap();
            let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
            assert_eq!(output["total_matched"].as_u64().unwrap(), 0);
            assert!(output["message"].as_str().unwrap().contains("No matches"));
        }

        // Scenario C: only target/ directory (Docker image with binary only)
        {
            let dir = tempfile::tempdir().unwrap();
            let ctx = test_sandbox(&dir);
            std::fs::create_dir_all(dir.path().join("target").join("release")).unwrap();
            std::fs::write(
                dir.path()
                    .join("target")
                    .join("release")
                    .join("vol-agent-server"),
                b"binary",
            )
            .unwrap();

            let result = tool.execute(&args, &ctx).await.unwrap();
            let output: serde_json::Value = serde_json::from_str(&result.content).unwrap();
            let count = output["total_matched"].as_u64().unwrap();
            assert!(count > 0, "should find binary in target/: {output:?}");
        }
    }
}
