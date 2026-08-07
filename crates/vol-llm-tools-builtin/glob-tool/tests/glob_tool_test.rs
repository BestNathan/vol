//! Integration tests for the redesigned GlobTool.
//!
//! These tests use a helper that creates a `ToolContext` with a `LocalSandbox`
//! rooted at a temp directory. All glob calls use relative paths (e.g., `"."`,
//! `"src"`) as the spec requires.

use serde_json::Value;
use std::sync::Arc;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_glob::GlobTool;

/// Create a ToolContext backed by a temp directory sandbox.
/// Returns (context, temp_dir) — keep temp_dir alive for the test duration.
fn test_context() -> (ToolContext, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(temp_dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

/// Helper to create a file in the temp dir (relative to sandbox root).
fn write_file(temp_dir: &tempfile::TempDir, rel_path: &str, content: &str) {
    let full = temp_dir.path().join(rel_path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(full, content).unwrap();
}

/// Helper to create a directory in the temp dir.
fn create_dir(temp_dir: &tempfile::TempDir, rel_path: &str) {
    let full = temp_dir.path().join(rel_path);
    std::fs::create_dir_all(full).unwrap();
}

/// Execute glob and return parsed JSON output.
async fn glob(args: Value, ctx: &ToolContext) -> Value {
    let tool = GlobTool::new();
    let result = tool.execute(&args, ctx).await.unwrap();
    let json: Value = serde_json::from_str(&result.content).unwrap();
    json
}

/// Execute glob and return raw ToolResult for error-checking.
async fn glob_raw(args: Value, ctx: &ToolContext) -> vol_llm_tool::ToolResult {
    let tool = GlobTool::new();
    tool.execute(&args, ctx).await.unwrap()
}

/// Extract match paths from JSON output as sorted Vec.
fn match_paths(json: &Value) -> Vec<&str> {
    let mut paths: Vec<&str> = json["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    paths.sort();
    paths
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Basic matching
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_basic_wildcard() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "fn main() {}");
    write_file(&tmp, "src/lib.rs", "pub fn lib() {}");
    write_file(&tmp, "README.md", "# Project");

    let json = glob(serde_json::json!({"pattern": "*.rs", "path": "src"}), &ctx).await;

    let paths = match_paths(&json);
    assert_eq!(paths, vec!["src/lib.rs", "src/main.rs"]);
    assert_eq!(json["total_matched"], 2);
    assert!(!json["truncated"].as_bool().unwrap());
}

#[tokio::test]
async fn test_glob_no_matches() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "test.txt", "content");

    let json = glob(
        serde_json::json!({"pattern": "*.nonexistent", "path": "."}),
        &ctx,
    )
    .await;

    assert_eq!(json["total_matched"], 0);
    assert!(!json["truncated"].as_bool().unwrap());
    assert!(json["message"]
        .as_str()
        .unwrap()
        .contains("No matches found"));
    assert!(json["matches"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn test_glob_exact_filename() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "Cargo.toml", "[package]");
    write_file(&tmp, "Cargo.lock", "");

    let json = glob(
        serde_json::json!({"pattern": "Cargo.toml", "path": "."}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths, vec!["Cargo.toml"]);
    assert_eq!(json["total_matched"], 1);
}

#[tokio::test]
async fn test_glob_recursive_double_star() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");
    write_file(&tmp, "src/components/button.rs", "");
    write_file(&tmp, "src/components/header.rs", "");
    write_file(&tmp, "README.md", "");

    let json = glob(serde_json::json!({"pattern": "**/*.rs", "path": "."}), &ctx).await;

    let paths = match_paths(&json);
    assert_eq!(paths.len(), 3);
    assert!(paths.contains(&"src/main.rs"));
    assert!(paths.contains(&"src/components/button.rs"));
    assert!(paths.contains(&"src/components/header.rs"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Brace expansion
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_brace_expansion() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/app.tsx", "");
    write_file(&tmp, "src/app.ts", "");
    write_file(&tmp, "src/app.css", "");

    let json = glob(
        serde_json::json!({"pattern": "*.{ts,tsx}", "path": "src"}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths, vec!["src/app.ts", "src/app.tsx"]);
    assert!(!paths.contains(&"src/app.css"));
}

#[tokio::test]
async fn test_glob_brace_expansion_multiple_sets() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/foo.test.ts", "");
    write_file(&tmp, "src/foo.spec.ts", "");
    write_file(&tmp, "src/foo.test.js", "");
    write_file(&tmp, "src/bar.ts", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*.{test,spec}.ts", "path": "."}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"src/foo.test.ts"));
    assert!(paths.contains(&"src/foo.spec.ts"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Hidden files
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_hidden_files_excluded_by_default() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "visible.txt", "");
    write_file(&tmp, ".hidden.txt", "");
    write_file(&tmp, ".secret/config.yml", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*", "path": ".", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"visible.txt"));
    assert!(!paths.contains(&".hidden.txt"));
    assert!(!paths.contains(&".secret/config.yml"));
}

#[tokio::test]
async fn test_glob_hidden_files_included_when_requested() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "visible.txt", "");
    write_file(&tmp, ".hidden.txt", "");
    write_file(&tmp, ".secret/config.yml", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*", "path": ".", "include_hidden": true, "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"visible.txt"));
    assert!(paths.contains(&".hidden.txt"));
    assert!(paths.contains(&".secret/config.yml"));
}

#[tokio::test]
async fn test_glob_hidden_directories_skipped_by_default() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, ".hidden_dir/file.txt", "");
    write_file(&tmp, "visible_dir/file.txt", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*.txt", "path": ".", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"visible_dir/file.txt"));
    assert!(!paths.contains(&".hidden_dir/file.txt"));
}

#[tokio::test]
async fn test_glob_hidden_directories_included_when_requested() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, ".hidden_dir/file.txt", "");
    write_file(&tmp, "visible_dir/file.txt", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*.txt", "path": ".", "include_hidden": true, "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"visible_dir/file.txt"));
    assert!(paths.contains(&".hidden_dir/file.txt"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Kind filtering (file / directory / all)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_kind_file() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");
    create_dir(&tmp, "src/components");

    let json = glob(
        serde_json::json!({"pattern": "*", "path": "src", "kind": "file", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths, vec!["src/main.rs"]);
}

#[tokio::test]
async fn test_glob_kind_directory() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");
    create_dir(&tmp, "src/components");

    let json = glob(
        serde_json::json!({"pattern": "*", "path": "src", "kind": "directory", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths, vec!["src/components"]);
}

#[tokio::test]
async fn test_glob_kind_all() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");
    create_dir(&tmp, "src/components");

    let json = glob(
        serde_json::json!({"pattern": "*", "path": "src", "kind": "all", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"src/main.rs"));
    assert!(paths.contains(&"src/components"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Max results truncation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_max_results_truncation() {
    let (ctx, tmp) = test_context();
    for i in 0..10 {
        write_file(&tmp, &format!("file_{:02}.txt", i), "");
    }

    let json = glob(
        serde_json::json!({"pattern": "*.txt", "path": ".", "max_results": 3, "exclude": []}),
        &ctx,
    )
    .await;

    assert_eq!(json["total_matched"], 10);
    assert!(json["truncated"].as_bool().unwrap());
    assert_eq!(json["matches"].as_array().unwrap().len(), 3);
    assert!(json["message"].as_str().unwrap().contains("truncated"));
}

#[tokio::test]
async fn test_glob_max_results_no_truncation_when_under_limit() {
    let (ctx, tmp) = test_context();
    for i in 0..5 {
        write_file(&tmp, &format!("file_{:02}.txt", i), "");
    }

    let json = glob(
        serde_json::json!({"pattern": "*.txt", "path": ".", "max_results": 100, "exclude": []}),
        &ctx,
    )
    .await;

    assert_eq!(json["total_matched"], 5);
    assert!(!json["truncated"].as_bool().unwrap());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Sort orders
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_sort_path_asc() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "zebra.rs", "");
    write_file(&tmp, "alpha.rs", "");
    write_file(&tmp, "mango.rs", "");

    let json = glob(
        serde_json::json!({"pattern": "*.rs", "path": ".", "sort": "path_asc", "exclude": []}),
        &ctx,
    )
    .await;

    let paths: Vec<&str> = json["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["alpha.rs", "mango.rs", "zebra.rs"]);
}

#[tokio::test]
async fn test_glob_sort_path_desc() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "alpha.rs", "");
    write_file(&tmp, "mango.rs", "");
    write_file(&tmp, "zebra.rs", "");

    let json = glob(
        serde_json::json!({"pattern": "*.rs", "path": ".", "sort": "path_desc", "exclude": []}),
        &ctx,
    )
    .await;

    let paths: Vec<&str> = json["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths, vec!["zebra.rs", "mango.rs", "alpha.rs"]);
}

#[tokio::test]
async fn test_glob_sort_modified_desc() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "oldest.rs", "old");
    // Sleep a tiny bit to ensure different mtimes
    std::thread::sleep(std::time::Duration::from_millis(10));
    write_file(&tmp, "newest.rs", "new");
    std::thread::sleep(std::time::Duration::from_millis(10));
    write_file(&tmp, "middle.rs", "mid");

    // Update middle to make it newer than oldest but older than newest
    std::thread::sleep(std::time::Duration::from_millis(10));
    write_file(&tmp, "newest.rs", "newer"); // newest becomes even newer

    let json = glob(
        serde_json::json!({"pattern": "*.rs", "path": ".", "sort": "modified_desc", "exclude": []}),
        &ctx,
    )
    .await;

    let paths: Vec<&str> = json["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    // newest should be first
    assert_eq!(paths[0], "newest.rs");
    // oldest should be last (since modified_desc is newest first)
    assert_eq!(paths[2], "oldest.rs");
}

#[tokio::test]
async fn test_glob_sort_modified_asc() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "oldest.rs", "old");
    std::thread::sleep(std::time::Duration::from_millis(10));
    write_file(&tmp, "middle.rs", "mid");
    std::thread::sleep(std::time::Duration::from_millis(10));
    write_file(&tmp, "newest.rs", "new");

    let json = glob(
        serde_json::json!({"pattern": "*.rs", "path": ".", "sort": "modified_asc", "exclude": []}),
        &ctx,
    )
    .await;

    let paths: Vec<&str> = json["matches"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m["path"].as_str().unwrap())
        .collect();
    assert_eq!(paths[0], "oldest.rs");
    assert_eq!(paths[2], "newest.rs");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Exclude patterns
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_exclude_pattern() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.ts", "");
    write_file(&tmp, "src/main.test.ts", "");
    write_file(&tmp, "src/utils.ts", "");

    let json = glob(
        serde_json::json!({
            "pattern": "**/*.ts",
            "path": ".",
            "exclude": ["**/*.test.ts"],
        }),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"src/main.ts"));
    assert!(paths.contains(&"src/utils.ts"));
    assert!(!paths.contains(&"src/main.test.ts"));
}

#[tokio::test]
async fn test_glob_exclude_directory() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");
    write_file(&tmp, "node_modules/pkg/index.js", "");
    write_file(&tmp, "dist/bundle.js", "");

    let json = glob(
        serde_json::json!({
            "pattern": "**/*.rs",
            "path": ".",
            "exclude": ["**/node_modules/**"],
        }),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"src/main.rs"));
    // node_modules subtree should be skipped entirely
}

#[tokio::test]
async fn test_glob_multiple_exclude_patterns() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");
    write_file(&tmp, "src/main.test.rs", "");
    write_file(&tmp, "target/debug/output", "");
    write_file(&tmp, "dist/bundle.js", "");

    let json = glob(
        serde_json::json!({
            "pattern": "**/*",
            "path": ".",
            "exclude": ["**/*.test.rs", "**/target/**", "**/dist/**"],
        }),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"src/main.rs"));
    assert!(!paths.contains(&"src/main.test.rs"));
    assert!(!paths.contains(&"target/debug/output"));
    assert!(!paths.contains(&"dist/bundle.js"));
}

#[tokio::test]
async fn test_glob_empty_exclude_disables_defaults() {
    let (ctx, tmp) = test_context();
    // Create a directory that would normally be excluded by default
    write_file(&tmp, "target/some_file.txt", "");
    write_file(&tmp, "src/main.rs", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*.txt", "path": ".", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    // With empty exclude, target/ should not be excluded
    assert!(paths.contains(&"target/some_file.txt"));
}

#[tokio::test]
async fn test_glob_default_excludes_are_applied() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");
    write_file(&tmp, "target/debug/some_file", "");
    write_file(&tmp, "node_modules/pkg/index.js", "");
    write_file(&tmp, ".git/config", "");

    let json = glob(serde_json::json!({"pattern": "**/*", "path": "."}), &ctx).await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"src/main.rs"));
    // Default excludes should hide these:
    assert!(!paths.contains(&"target/debug/some_file"));
    assert!(!paths.contains(&"node_modules/pkg/index.js"));
    assert!(!paths.contains(&".git/config"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Path not found / empty directory
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_path_not_found() {
    let (ctx, _tmp) = test_context();

    let json = glob(
        serde_json::json!({"pattern": "*.rs", "path": "nonexistent_dir"}),
        &ctx,
    )
    .await;

    assert_eq!(json["total_matched"], 0);
    assert!(json["message"].as_str().unwrap().contains("does not exist"));
    assert!(json["error"].is_null() || json["error"].as_object().is_none());
}

#[tokio::test]
async fn test_glob_empty_directory() {
    let (ctx, tmp) = test_context();
    create_dir(&tmp, "empty_dir");

    let json = glob(
        serde_json::json!({"pattern": "*", "path": "empty_dir", "exclude": []}),
        &ctx,
    )
    .await;

    assert_eq!(json["total_matched"], 0);
    // Should not be an error, just empty results
    assert!(json["error"].is_null() || json["error"].as_object().is_none());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Parameter validation errors
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_empty_pattern_rejected() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(serde_json::json!({"pattern": ""}), &ctx).await;
    assert!(!result.success);
    assert!(result.content.contains("INVALID_PATTERN"));
    assert!(result.content.contains("must not be empty"));
}

#[tokio::test]
async fn test_glob_absolute_path_rejected() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(serde_json::json!({"pattern": "*.rs", "path": "/etc"}), &ctx).await;
    assert!(!result.success);
    assert!(result.content.contains("INVALID_PATH"));
    assert!(result.content.contains("must be a relative path"));
}

#[tokio::test]
async fn test_glob_parent_traversal_rejected() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(
        serde_json::json!({"pattern": "*.rs", "path": "../outside"}),
        &ctx,
    )
    .await;
    assert!(!result.success);
    assert!(result.content.contains("PATH_OUTSIDE_WORKSPACE"));
}

#[tokio::test]
async fn test_glob_invalid_kind_rejected() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(
        serde_json::json!({"pattern": "*.rs", "kind": "symlink"}),
        &ctx,
    )
    .await;
    assert!(!result.success);
    assert!(result.content.contains("INVALID_KIND"));
    assert!(result.content.contains("file"));
    assert!(result.content.contains("directory"));
    assert!(result.content.contains("all"));
}

#[tokio::test]
async fn test_glob_invalid_max_results_zero() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(
        serde_json::json!({"pattern": "*.rs", "max_results": 0}),
        &ctx,
    )
    .await;
    assert!(!result.success);
    assert!(result.content.contains("MAX_RESULTS_OUT_OF_RANGE"));
}

#[tokio::test]
async fn test_glob_invalid_max_results_too_high() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(
        serde_json::json!({"pattern": "*.rs", "max_results": 1001}),
        &ctx,
    )
    .await;
    assert!(!result.success);
    assert!(result.content.contains("MAX_RESULTS_OUT_OF_RANGE"));
}

#[tokio::test]
async fn test_glob_invalid_sort_rejected() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(
        serde_json::json!({"pattern": "*.rs", "sort": "random"}),
        &ctx,
    )
    .await;
    assert!(!result.success);
    assert!(result.content.contains("INVALID_SORT"));
}

#[tokio::test]
async fn test_glob_unbalanced_braces_rejected() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(serde_json::json!({"pattern": "{a,b"}), &ctx).await;
    assert!(!result.success);
    assert!(result.content.contains("INVALID_PATTERN"));
    assert!(result.content.contains("Unbalanced braces"));
}

#[tokio::test]
async fn test_glob_invalid_exclude_pattern_rejected() {
    let (ctx, _tmp) = test_context();
    // An unmatched `[` makes a pattern invalid in the glob crate
    let result = glob_raw(
        serde_json::json!({"pattern": "*.rs", "exclude": ["**[broken]"]}),
        &ctx,
    )
    .await;
    assert!(!result.success);
    assert!(result.content.contains("INVALID_PATTERN"));
    assert!(result.content.contains("exclude"));
}

#[tokio::test]
async fn test_glob_error_includes_suggestion() {
    let (ctx, _tmp) = test_context();
    let result = glob_raw(serde_json::json!({"pattern": ""}), &ctx).await;
    assert!(!result.success);
    let json: Value = serde_json::from_str(&result.content).unwrap();
    let suggestion = json["error"]["suggestion"].as_str().unwrap();
    assert!(!suggestion.is_empty());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// With metadata
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_with_metadata() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "fn main() {}");

    let json = glob(
        serde_json::json!({"pattern": "*.rs", "path": "src", "with_metadata": true, "exclude": []}),
        &ctx,
    )
    .await;

    let first = &json["matches"][0];
    assert_eq!(first["path"], "src/main.rs");
    assert_eq!(first["type"], "file");
    // size_bytes should be present
    assert!(first["size_bytes"].is_number());
    assert!(first["size_bytes"].as_u64().unwrap() > 0);
    // modified_at should be an ISO 8601 string
    assert!(first["modified_at"].is_string());
    assert!(first["modified_at"].as_str().unwrap().contains("T"));
}

#[tokio::test]
async fn test_glob_without_metadata() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "src/main.rs", "");

    let json = glob(
        serde_json::json!({"pattern": "*.rs", "path": "src", "with_metadata": false, "exclude": []}),
        &ctx,
    )
    .await;

    let first = &json["matches"][0];
    // size_bytes and modified_at should NOT be present
    assert!(first.get("size_bytes").is_none());
    assert!(first.get("modified_at").is_none());
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Character class matching [?] and [abc]
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_character_class() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "file1.txt", "");
    write_file(&tmp, "file2.txt", "");
    write_file(&tmp, "file3.txt", "");
    write_file(&tmp, "fileX.txt", "");

    let json = glob(
        serde_json::json!({"pattern": "file[12].txt", "path": ".", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"file1.txt"));
    assert!(paths.contains(&"file2.txt"));
}

#[tokio::test]
async fn test_glob_single_char_wildcard() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "file1.txt", "");
    write_file(&tmp, "fileA.txt", "");
    write_file(&tmp, "file10.txt", ""); // two digits — shouldn't match ?

    let json = glob(
        serde_json::json!({"pattern": "file?.txt", "path": ".", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"file1.txt"));
    assert!(paths.contains(&"fileA.txt"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Edge cases
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_output_contains_expected_top_level_fields() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "test.rs", "");

    let json = glob(serde_json::json!({"pattern": "*.rs", "path": "."}), &ctx).await;

    // Verify all top-level fields from the spec are present
    assert!(json.get("matches").is_some(), "missing 'matches'");
    assert!(
        json.get("total_matched").is_some(),
        "missing 'total_matched'"
    );
    assert!(json.get("truncated").is_some(), "missing 'truncated'");
    assert!(json.get("search_path").is_some(), "missing 'search_path'");
    assert!(json.get("pattern").is_some(), "missing 'pattern'");
    assert!(json.get("excluded").is_some(), "missing 'excluded'");
    assert!(
        json.get("message").is_some() || json.get("message").is_none(),
        "message field should be present or null"
    );
}

#[tokio::test]
async fn test_glob_search_path_in_output() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "apps/web/page.tsx", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*.tsx", "path": "apps"}),
        &ctx,
    )
    .await;

    assert_eq!(json["search_path"], "apps");
    assert_eq!(json["pattern"], "**/*.tsx");
}

#[tokio::test]
async fn test_glob_excluded_list_in_output() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "test.rs", "");

    let json = glob(
        serde_json::json!({
            "pattern": "*.rs",
            "exclude": ["**/custom/**", "**/*.test.rs"]
        }),
        &ctx,
    )
    .await;

    let excluded: Vec<&str> = json["excluded"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    assert!(excluded.contains(&"**/custom/**"));
    assert!(excluded.contains(&"**/*.test.rs"));
}

#[tokio::test]
async fn test_glob_max_results_default_is_100() {
    let (_ctx, _tmp) = test_context();
    // Verify default max_results is 100 via the parameter schema
    let tool = GlobTool::new();
    let params = tool.parameters();
    let max_results = &params["properties"]["max_results"];
    assert_eq!(max_results["default"], 100);
}

#[tokio::test]
async fn test_glob_sort_default_is_path_asc() {
    let tool = GlobTool::new();
    let params = tool.parameters();
    let sort = &params["properties"]["sort"];
    assert_eq!(sort["default"], "path_asc");
}

#[tokio::test]
async fn test_glob_include_hidden_default_false() {
    let tool = GlobTool::new();
    let params = tool.parameters();
    let ih = &params["properties"]["include_hidden"];
    assert_eq!(ih["default"], false);
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Deep nesting
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_deeply_nested_files() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "a/b/c/d/e/deep.rs", "");
    write_file(&tmp, "a/shallow.rs", "");

    let json = glob(
        serde_json::json!({"pattern": "**/*.rs", "path": ".", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert_eq!(paths.len(), 2);
    assert!(paths.contains(&"a/b/c/d/e/deep.rs"));
    assert!(paths.contains(&"a/shallow.rs"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Dotfiles in non-hidden directories
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_dotfile_in_visible_dir_excluded() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "config/.env", "");
    write_file(&tmp, "config/settings.toml", "");

    let json = glob(
        serde_json::json!({"pattern": "config/*", "path": ".", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"config/settings.toml"));
    assert!(!paths.contains(&"config/.env"));
}

#[tokio::test]
async fn test_glob_dotfile_in_visible_dir_included_with_flag() {
    let (ctx, tmp) = test_context();
    write_file(&tmp, "config/.env", "");
    write_file(&tmp, "config/settings.toml", "");

    let json = glob(
        serde_json::json!({"pattern": "config/*", "path": ".", "include_hidden": true, "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"config/settings.toml"));
    assert!(paths.contains(&"config/.env"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Mixed file types in same pattern
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_glob_named_file_and_directory() {
    let (ctx, tmp) = test_context();
    // Both a file and a directory named "docs"
    write_file(&tmp, "docs", "file named docs");
    create_dir(&tmp, "docs_dir");

    // When kind is "file", only the file should match
    let json = glob(
        serde_json::json!({"pattern": "doc*", "path": ".", "kind": "file", "exclude": []}),
        &ctx,
    )
    .await;

    let paths = match_paths(&json);
    assert!(paths.contains(&"docs"), "Should find file named 'docs'");
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Tool trait implementation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_glob_tool_implements_default() {
    let _tool = GlobTool::default();
    let _tool = GlobTool::new();
}

#[test]
fn test_glob_tool_name_is_glob() {
    let tool = GlobTool::new();
    assert_eq!(tool.name(), "glob");
}
