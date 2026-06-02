# Grep Tool Multi-Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite grep built-in tool with a strategy pattern: `rg` CLI as primary backend, `ignore` + `grep-searcher` as Rust library fallback.

**Architecture:** `GrepTool` holds an `Option<RgCliBackend>` — if `rg` binary detected, delegates to CLI backend; otherwise falls back to `RustLibBackend`. Both implement identical output format.

**Tech Stack:** `ignore` (walk), `grep-searcher` + `grep-regex` (search), `grep-matcher` (trait), `tokio` (timeout/spawn_blocking), `regex` (glob→regex)

---

### Task 1: Update Dependencies

**Files:**
- Modify: `crates/vol-llm-tools-builtin/grep-tool/Cargo.toml`

- [ ] **Step 1: Add `ignore` and `grep-regex`, remove `walkdir`**

```bash
cd /root/nq-deribit
```

Edit `crates/vol-llm-tools-builtin/grep-tool/Cargo.toml` to replace `walkdir` with `ignore` and add `grep-regex`:

```toml
[package]
name = "vol-llm-tools-builtin-grep"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
grep-matcher = "0.1"
grep-regex = "0.1"
grep-searcher = "0.1"
ignore = "0.4"
regex = "1.10"

[dev-dependencies]
tempfile = "3.10"
tokio = { workspace = true, features = ["macros", "rt"] }
```

Note: `grep = "0.2"` and `walkdir = "2.4"` are removed. `grep-regex = "0.1"` and `ignore = "0.4"` are added.

- [ ] **Step 2: Verify `ignore` and `grep-regex` resolve**

```bash
cargo check -p vol-llm-tools-builtin-grep 2>&1 | grep "error" | head -5
```

Expected: no errors (warnings about unused deps are OK — they'll be used in later tasks).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tools-builtin/grep-tool/Cargo.toml
git commit -m "deps(grep-tool): add ignore, grep-regex; remove walkdir, grep"
```

---

### Task 2: Write Test for RgCliBackend

**Files:**
- Modify: `crates/vol-llm-tools-builtin/grep-tool/tests/grep_tool_test.rs`

- [ ] **Step 1: Add test for CLI backend with output mode mapping**

Append to `crates/vol-llm-tools-builtin/grep-tool/tests/grep_tool_test.rs`:

```rust
#[tokio::test]
async fn test_grep_content_mode_handles_empty_file() {
    let dir = tempdir().unwrap();
    fs::File::create(dir.path().join("empty.txt")).unwrap();

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "hello",
        "path": dir.path().to_str().unwrap(),
        "output_mode": "content"
    });

    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("No matches"));
}

#[tokio::test]
async fn test_grep_case_sensitive_both_modes() {
    let dir = tempdir().unwrap();
    let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
    writeln!(f1, "Hello World").unwrap();
    writeln!(f1, "hello world").unwrap();

    let tool = GrepTool::new();

    // Case-insensitive (default) - should find both
    let args = json!({
        "pattern": "hello",
        "path": dir.path().to_str().unwrap(),
        "output_mode": "count",
        "case_sensitive": false
    });
    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("2"));

    // Case-sensitive - should only find lowercase
    let args = json!({
        "pattern": "hello",
        "path": dir.path().to_str().unwrap(),
        "output_mode": "count",
        "case_sensitive": true
    });
    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("1"));
}

#[tokio::test]
async fn test_glob_filter_recursive() {
    let dir = tempdir().unwrap();
    let sub = dir.path().join("subdir");
    fs::create_dir_all(&sub).unwrap();
    let mut f1 = fs::File::create(sub.join("nested.rs")).unwrap();
    writeln!(f1, "fn hello() {{}}").unwrap();
    let mut f2 = fs::File::create(dir.path().join("top.txt")).unwrap();
    writeln!(f2, "hello world").unwrap();

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "hello",
        "path": dir.path().to_str().unwrap(),
        "glob": "**/*.rs",
        "output_mode": "files_with_matches"
    });

    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    // Should find nested.rs (matched by **/*.rs)
    let content = &result.content;
    assert!(content.contains("nested.rs"), "expected nested.rs in: {}", content);
    // Should NOT find top.txt
    assert!(!content.contains("top.txt"));
}
```

- [ ] **Step 2: Verify new tests fail with current implementation**

```bash
cargo test -p vol-llm-tools-builtin-grep --test grep_tool_test -- --nocapture 2>&1 | tail -20
```

Expected: `test_glob_filter_recursive` and `test_grep_content_mode_handles_empty_file` should FAIL because the current `glob_match` only matches filenames and there may be edge cases.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tools-builtin/grep-tool/tests/grep_tool_test.rs
git commit -m "test(grep-tool): add recursive glob, empty file, and case-sensitivity tests"
```

---

### Task 3: Implement GrepBackend Trait and GrepTool Rewrite

**Files:**
- Modify: `crates/vol-llm-tools-builtin/grep-tool/src/lib.rs`

- [ ] **Step 1: Write minimal library files**

First, remove the old lib.rs content entirely. Write the new file at `crates/vol-llm-tools-builtin/grep-tool/src/lib.rs`:

```rust
//! vol-llm-tools-builtin-grep: Multi-backend grep tool.
//!
//! Strategy: prefers `rg` CLI when available (fast, .gitignore-aware),
//! falls back to Rust library (ignore + grep-searcher) otherwise.
//!
//! Both backends produce identical output format.

pub mod backend;
pub mod cli;
pub mod lib_impl;

use async_trait::async_trait;
use backend::GrepBackend;
use cli::RgCliBackend;
use lib_impl::RustLibBackend;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
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
struct SearchResult {
    path: PathBuf,
    match_count: usize,
    line_numbers: Vec<usize>,
}

pub struct GrepTool {
    has_rg: AtomicBool,
    rg_checked: AtomicBool,
}

impl GrepTool {
    pub fn new() -> Self {
        Self {
            has_rg: AtomicBool::new(false),
            rg_checked: AtomicBool::new(false),
        }
    }

    fn ensure_checked(&self) {
        if !self.rg_checked.load(Ordering::Acquire) {
            self.rg_checked.store(true, Ordering::Release);
            let available = RgCliBackend::is_available();
            self.has_rg.store(available, Ordering::Release);
        }
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

        self.ensure_checked();

        let search_root = PathBuf::from(params.path.clone().unwrap_or_else(|| ".".to_string()));

        let search_future = if self.has_rg.load(Ordering::Acquire) {
            RgCliBackend::search(&params, &search_root)
        } else {
            RustLibBackend::search(&params, &search_root)
        };

        match tokio::time::timeout(Duration::from_secs(SEARCH_TIMEOUT_SECS), search_future).await {
            Ok(Ok(results)) => {
                let content = format_results(&params.output_mode, &results);
                if content.is_empty() {
                    Ok(ToolResult::success("No matches found."))
                } else {
                    Ok(ToolResult::success(content))
                }
            }
            Ok(Err(e)) => Err(ToolError::ExecutionFailed(e)),
            Err(_) => Err(ToolError::ExecutionFailed(
                "Search timed out after 30 seconds. Try a narrower path or glob.".to_string(),
            )),
        }
    }
}

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
```

- [ ] **Step 2: Verify compilation (will fail — modules not yet created)**

```bash
cargo check -p vol-llm-tools-builtin-grep 2>&1 | grep "error" | head -5
```

Expected: errors about missing modules `backend`, `cli`, `lib_impl`. This is expected — we create them next.

---

### Task 4: Implement Backend Trait Module

**Files:**
- Create: `crates/vol-llm-tools-builtin/grep-tool/src/backend.rs`

- [ ] **Step 1: Write the trait definition**

```bash
mkdir -p crates/vol-llm-tools-builtin/grep-tool/src
```

Create `crates/vol-llm-tools-builtin/grep-tool/src/backend.rs`:

```rust
//! GrepBackend trait — common interface for grep implementations.

use std::path::Path;

use crate::{GrepParams, SearchResult};

/// A grep backend provides a single search method.
/// Implementations may shell out to CLI tools or use Rust libraries.
#[async_trait::async_trait]
pub trait GrepBackend: Send + Sync {
    /// Check whether this backend is available in the current environment.
    fn is_available() -> bool
    where
        Self: Sized;

    /// Execute a grep search with the given parameters.
    /// Returns a Vec of SearchResult (one per matching file).
    async fn search(params: &GrepParams, root: &Path) -> Result<Vec<SearchResult>, String>;
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-tools-builtin-grep 2>&1 | grep "error" | head -5
```

Expected: error about missing `cli` module (next task).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tools-builtin-grep/src/backend.rs
git add crates/vol-llm-tools-builtin-grep/src/lib.rs
git commit -m "feat(grep-tool): add GrepBackend trait and GrepTool entry point"
```

---

### Task 5: Implement RgCliBackend (rg binary)

**Files:**
- Create: `crates/vol-llm-tools-builtin/grep-tool/src/cli.rs`

- [ ] **Step 1: Write the CLI backend**

Create `crates/vol-llm-tools-builtin/grep-tool/src/cli.rs`:

```rust
//! RgCliBackend — delegates to the `rg` (ripgrep) binary.

use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::OnceLock;
use std::time::Duration;

use crate::backend::GrepBackend;
use crate::{GrepParams, SearchResult, MODE_COUNT, MODE_CONTENT, MODE_FILES};

static RG_AVAILABLE: OnceLock<bool> = OnceLock::new();

pub struct RgCliBackend;

impl RgCliBackend {
    fn detect() -> bool {
        *RG_AVAILABLE.get_or_init(|| {
            let output = Command::new("rg")
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .output();
            output.map(|o| o.status.success()).unwrap_or(false)
        })
    }
}

#[async_trait::async_trait]
impl GrepBackend for RgCliBackend {
    fn is_available() -> bool {
        Self::detect()
    }

    async fn search(params: &GrepParams, root: &Path) -> Result<Vec<SearchResult>, String> {
        let mut cmd = Command::new("rg");
        cmd.args([
            "--no-heading",
            "--with-filename",
            "--color",
            "never",
            "--max-depth",
            "50",
            "--max-filesize",
            "10M",
        ]);

        // Output mode flag
        match params.output_mode.as_str() {
            MODE_FILES => {
                cmd.arg("-l");
            }
            MODE_COUNT => {
                cmd.arg("-c");
            }
            MODE_CONTENT => {
                cmd.arg("-n");
            }
            _ => unreachable!(),
        };

        // Case sensitivity
        if params.case_sensitive {
            cmd.arg("-s");
        } else {
            cmd.arg("-i");
        }

        // Glob filter
        if let Some(ref glob) = params.glob {
            cmd.arg("-g").arg(glob);
        }

        // Pattern and path
        cmd.arg("--")
            .arg(&params.pattern)
            .arg(root);

        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let child = cmd
            .spawn()
            .map_err(|e| format!("Failed to spawn rg process: {}", e))?;

        let output = tokio::task::spawn_blocking(move || {
            child.wait_with_output()
        })
        .await
        .map_err(|e| format!("rg process join error: {}", e))?
        .map_err(|e| format!("rg process error: {}", e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("No such file") {
                return Ok(vec![]); // No matches or directory doesn't exist
            }
            return Err(format!("rg failed: {}", stderr.trim()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        parse_rg_output(&stdout, params.output_mode.as_str())
    }
}

fn parse_rg_output(stdout: &str, mode: &str) -> Result<Vec<SearchResult>, String> {
    let mut results: Vec<SearchResult> = Vec::new();
    let mut current: Option<SearchResult> = None;

    for line in stdout.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        match mode {
            MODE_FILES => {
                let path = std::path::PathBuf::from(line);
                results.push(SearchResult {
                    path,
                    match_count: 1,
                    line_numbers: Vec::new(),
                });
            }
            MODE_COUNT => {
                if let Some(colon_idx) = line.rfind(':') {
                    let path = std::path::PathBuf::from(&line[..colon_idx]);
                    let count: usize = line[colon_idx + 1..].parse().unwrap_or(0);
                    results.push(SearchResult {
                        path,
                        match_count: count,
                        line_numbers: Vec::new(),
                    });
                }
            }
            MODE_CONTENT => {
                if let Some(colon_idx) = line.find(':') {
                    let path_part = &line[..colon_idx];
                    let rest = &line[colon_idx + 1..];
                    if let Some(line_colon) = rest.find(':') {
                        let line_num: usize = rest[..line_colon].parse().unwrap_or(0);
                        let path = std::path::PathBuf::from(path_part);
                        if let Some(ref mut last) = current {
                            if last.path == path {
                                last.line_numbers.push(line_num);
                                last.match_count += 1;
                                continue;
                            }
                        }
                        let new_result = SearchResult {
                            path: path.clone(),
                            match_count: 1,
                            line_numbers: vec![line_num],
                        };
                        results.push(new_result);
                        current = Some(results.last().unwrap().clone());
                    }
                }
            }
            _ => {}
        }
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_files_mode() {
        let output = "src/main.rs\nsrc/lib.rs\n";
        let results = parse_rg_output(output, MODE_FILES).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path.to_string_lossy(), "src/main.rs");
        assert_eq!(results[1].path.to_string_lossy(), "src/lib.rs");
    }

    #[test]
    fn test_parse_count_mode() {
        let output = "src/main.rs:5\nsrc/lib.rs:12\n";
        let results = parse_rg_output(output, MODE_COUNT).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].match_count, 5);
        assert_eq!(results[1].match_count, 12);
    }

    #[test]
    fn test_parse_content_mode() {
        let output = "src/main.rs:10:hello\nsrc/main.rs:15:world\nsrc/lib.rs:3:test\n";
        let results = parse_rg_output(output, MODE_CONTENT).unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].path.to_string_lossy(), "src/main.rs");
        assert_eq!(results[0].line_numbers, vec![10, 15]);
        assert_eq!(results[1].path.to_string_lossy(), "src/lib.rs");
        assert_eq!(results[1].line_numbers, vec![3]);
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-tools-builtin-grep 2>&1 | grep "error" | head -5
```

Expected: error about missing `lib_impl` module (next task). No errors in `cli.rs`.

- [ ] **Step 3: Run CLI backend unit tests**

```bash
cargo test -p vol-llm-tools-builtin-grep --lib -- cli::tests 2>&1
```

Expected: 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tools-builtin-grep/src/cli.rs
git commit -m "feat(grep-tool): add RgCliBackend with rg binary integration"
```

---

### Task 6: Implement RustLibBackend (ignore + grep-searcher)

**Files:**
- Create: `crates/vol-llm-tools-builtin/grep-tool/src/lib_impl.rs`

- [ ] **Step 1: Write the Rust library backend**

Create `crates/vol-llm-tools-builtin/grep-tool/src/lib_impl.rs`:

```rust
//! RustLibBackend — uses `ignore` crate for .gitignore-aware walking
//! and `grep-searcher` for parallel regex search.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use grep_matcher::Matcher;
use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use ignore::WalkBuilder;

use crate::backend::GrepBackend;
use crate::{GrepParams, SearchResult};

pub struct RustLibBackend;

impl RustLibBackend {
    fn glob_to_regex(glob: &str) -> regex::Regex {
        let escaped = regex::escape(glob);
        let regex_str = escaped
            .replace(r"\*\*/", "<<<DS>>>/")
            .replace(r"\*\*", ".*")
            .replace("<<<DS>>>/", "(.*/)?")
            .replace(r"\*", "[^/]*")
            .replace(r"\?", "[^/]");
        regex::Regex::new(&format!("^{}$", regex_str))
            .unwrap_or_else(|_| regex::Regex::new(".*").unwrap())
    }
}

#[async_trait::async_trait]
impl GrepBackend for RustLibBackend {
    fn is_available() -> bool {
        true // Always available as a Rust library
    }

    async fn search(params: &GrepParams, root: &Path) -> Result<Vec<SearchResult>, String> {
        let pattern = params.pattern.clone();
        let case_sensitive = params.case_sensitive;
        let glob = params.glob.clone();
        let output_mode = params.output_mode.clone();
        let root_path = root.to_path_buf();

        tokio::task::spawn_blocking(move || {
            search_blocking(&pattern, case_sensitive, &glob, &output_mode, &root_path)
        })
        .await
        .map_err(|e| format!("Search task panicked: {}", e))?
    }
}

fn search_blocking(
    pattern: &str,
    case_sensitive: bool,
    glob: &Option<String>,
    output_mode: &str,
    root: &Path,
) -> Result<Vec<SearchResult>, String> {
    // Build regex matcher
    let regex_pattern = if case_sensitive {
        pattern.to_string()
    } else {
        format!("(?i){}", pattern)
    };
    let matcher = RegexMatcher::new(&regex_pattern)
        .map_err(|e| format!("Invalid regex pattern: {}", e))?;

    // Build walker that auto-respects .gitignore
    let mut walker = WalkBuilder::new(root);
    walker.max_depth(Some(50)).hidden(false).git_ignore(true);

    // Apply glob type filter
    if let Some(ref g) = glob {
        walker.types(
            ignore::types::TypesBuilder::new()
                .add("custom", g)
                .map_err(|e| format!("Invalid glob: {}", e))?
                .build()
                .map_err(|e| format!("Glob error: {}", e))?,
        );
    }

    let searcher = Searcher::new();
    let glob_regex = glob.as_ref().map(|g| Self::glob_to_regex(g));
    let results: Arc<std::sync::Mutex<Vec<SearchResult>>> =
        Arc::new(std::sync::Mutex::new(Vec::new()));

    for entry in walker.build().filter_map(|e| e.ok()) {
        let path = entry.path();
        let file_type = entry.file_type();

        if !file_type.is_file() {
            continue;
        }

        // Apply glob filter if set
        if let Some(ref re) = glob_regex {
            let target = if glob.as_ref().map_or(false, |g| g.contains("**")) {
                match path.strip_prefix(root) {
                    Ok(rel) => rel.to_string_lossy().to_string(),
                    Err(_) => path.to_string_lossy().to_string(),
                }
            } else {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("")
                    .to_string()
            };
            if !re.is_match(&target) {
                continue;
            }
        }

        let line_nums = Arc::new(AtomicUsize::new(0));
        let mut sink = LineCollector {
            line_numbers: Vec::new(),
            count: 0,
        };

        let _ = searcher.search_path(&matcher, path, &mut sink);

        if sink.count > 0 {
            results.lock().unwrap().push(SearchResult {
                path: path.to_path_buf(),
                match_count: sink.count,
                line_numbers: sink.line_numbers,
            });
        }
    }

    Ok(results.into_inner().unwrap())
}

/// Sink for collecting line numbers during grep-searcher search.
struct LineCollector {
    line_numbers: Vec<usize>,
    count: usize,
}

impl grep_searcher::Sink for &mut LineCollector {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &Searcher,
        mat: &grep_searcher::SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        let line_num = mat.line_number().unwrap_or(0) as usize;
        self.line_numbers.push(line_num);
        self.count += 1;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::GrepParams;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;

    fn make_params(pattern: &str, glob: Option<&str>, output_mode: &str) -> GrepParams {
        GrepParams {
            pattern: pattern.to_string(),
            path: None,
            glob: glob.map(|g| g.to_string()),
            output_mode: output_mode.to_string(),
            case_sensitive: false,
        }
    }

    #[tokio::test]
    async fn test_rustlib_basic_search() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();
        writeln!(f1, "foo bar").unwrap();
        writeln!(f1, "hello again").unwrap();

        let params = make_params("hello", None, "files_with_matches");
        let results = RustLibBackend::search(&params, dir.path()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].path.ends_with("test.txt"));
    }

    #[tokio::test]
    async fn test_rustlib_no_matches() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello world").unwrap();

        let params = make_params("nonexistent", None, "files_with_matches");
        let results = RustLibBackend::search(&params, dir.path()).await.unwrap();
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_rustlib_glob_filter() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.rs")).unwrap();
        writeln!(f1, "fn hello() {{}}").unwrap();
        let mut f2 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f2, "hello world").unwrap();

        let params = make_params("hello", Some("*.rs"), "files_with_matches");
        let results = RustLibBackend::search(&params, dir.path()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert!(results[0].path.ends_with(".rs"));
    }

    #[tokio::test]
    async fn test_rustlib_count_mode() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "hello").unwrap();
        writeln!(f1, "hello").unwrap();
        writeln!(f1, "world").unwrap();

        let params = make_params("hello", None, "count");
        let results = RustLibBackend::search(&params, dir.path()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].match_count, 2);
    }

    #[tokio::test]
    async fn test_rustlib_content_mode() {
        let dir = tempdir().unwrap();
        let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
        writeln!(f1, "line 1").unwrap();
        writeln!(f1, "hello world").unwrap();
        writeln!(f1, "line 3").unwrap();

        let params = make_params("hello", None, "content");
        let results = RustLibBackend::search(&params, dir.path()).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].line_numbers, vec![2]);
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-tools-builtin-grep 2>&1 | grep "error" | head -10
```

Expected: no errors.

- [ ] **Step 3: Run RustLibBackend unit tests**

```bash
cargo test -p vol-llm-tools-builtin-grep --lib -- lib_impl::tests 2>&1
```

Expected: 5 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tools-builtin-grep/src/lib_impl.rs
git commit -m "feat(grep-tool): add RustLibBackend using ignore + grep-searcher"
```

---

### Task 7: Verify All Integration Tests Pass

**Files:**
- Check: `crates/vol-llm-tools-builtin/grep-tool/tests/grep_tool_test.rs`

- [ ] **Step 1: Run all tests (unit + integration)**

```bash
cargo test -p vol-llm-tools-builtin-grep 2>&1 | tail -30
```

Expected: all tests pass — 3 unit tests (cli parsing) + 5 unit tests (lib_impl) + 8 integration tests.

- [ ] **Step 2: Run full workspace check**

```bash
cargo check 2>&1 | grep "error" | head -5
```

Expected: no errors from grep-tool or its dependents.

- [ ] **Step 3: Commit if any test file changes were needed**

```bash
git add crates/vol-llm-tools-builtin/grep-tool/tests/
git diff --cached --stat && git commit -m "test(grep-tool): finalize integration tests for multi-backend grep"
```

---

### Task 8: Restart Backend and Verify

**Files:**
- None (runtime verification)

- [ ] **Step 1: Rebuild and restart backend**

```bash
kill $(lsof -ti :3001) 2>/dev/null
sleep 1
ANTHROPIC_AUTH_TOKEN=sk cargo run --example jsonrpc_agent_service -p vol-llm-agent-channel &
sleep 20
lsof -i :3001 2>/dev/null | grep LISTEN
```

- [ ] **Step 2: Quick smoke test with curl/websocat**

```bash
# Verify grep tool is registered
echo '{"jsonrpc":"2.0","method":"tool.list","params":{},"id":1}' | websocat -n1 ws://localhost:3001/ws 2>/dev/null | python3 -c "import sys,json; tools=[t['name'] for t in json.load(sys.stdin)['result']['tools']]; print('grep' in tools and 'grep found!' or 'grep missing!')"
```

Expected: "grep found!"

