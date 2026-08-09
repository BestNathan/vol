//! Integration tests for GrepTool

use serde_json::json;
use std::fs;
use std::io::Write;
use std::sync::Arc;
use tempfile::tempdir;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_grep::GrepTool;

/// Build a ToolContext whose sandbox root is a caller-owned temp dir.
/// The temp dir is passed through the sandbox, so the search root is
/// mediated by the sandbox rather than the process's filesystem root.
fn sandbox_context() -> (ToolContext, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(temp_dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

/// Create a file inside the sandbox temp dir, creating parent dirs as needed.
fn create_file_in(dir: &tempfile::TempDir, name: &str, content: &str) {
    let path = dir.path().join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(path, content).unwrap();
}

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

#[tokio::test]
async fn test_grep_case_sensitive() {
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
    assert!(result.content.contains("2")); // Both lines match

    // Case-sensitive - should only find lowercase
    let args = json!({
        "pattern": "hello",
        "path": dir.path().to_str().unwrap(),
        "output_mode": "count",
        "case_sensitive": true
    });
    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("1")); // Only one line matches
}

#[tokio::test]
async fn test_grep_count_mode() {
    let dir = tempdir().unwrap();
    let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
    writeln!(f1, "hello").unwrap();
    writeln!(f1, "hello").unwrap();
    writeln!(f1, "world").unwrap();

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "hello",
        "path": dir.path().to_str().unwrap(),
        "output_mode": "count"
    });

    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("test.txt"));
    assert!(result.content.contains("2")); // 2 matches
}

#[tokio::test]
async fn test_grep_content_mode() {
    let dir = tempdir().unwrap();
    let mut f1 = fs::File::create(dir.path().join("test.txt")).unwrap();
    writeln!(f1, "line 1").unwrap();
    writeln!(f1, "hello world").unwrap();
    writeln!(f1, "line 3").unwrap();

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "hello",
        "path": dir.path().to_str().unwrap(),
        "output_mode": "content"
    });

    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("test.txt"));
    assert!(result.content.contains(":2")); // Line 2
}

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
async fn test_grep_recursive_glob() {
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
    let content = &result.content;
    assert!(
        content.contains("nested.rs"),
        "expected nested.rs in: {}",
        content
    );
    assert!(!content.contains("top.txt"));
}

#[tokio::test]
async fn test_grep_files_with_matches_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    create_file_in(&tmp, "a.rs", "fn main() {\n    println!(\"hello\");\n}");
    create_file_in(&tmp, "b.rs", "fn test() {\n    assert!(true);\n}");
    create_file_in(&tmp, "c.txt", "hello from text file");

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "hello",
        "path": tmp.path().to_str().unwrap(),
        "output_mode": "files_with_matches"
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("a.rs"));
    assert!(result.content.contains("c.txt"));
    assert!(!result.content.contains("b.rs"));
}

#[tokio::test]
async fn test_grep_count_mode_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    create_file_in(&tmp, "test.txt", "hello\nhello\nworld\n");

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "hello",
        "path": tmp.path().to_str().unwrap(),
        "output_mode": "count"
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("test.txt"));
    assert!(result.content.contains("2"));
}

#[tokio::test]
async fn test_grep_content_mode_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    create_file_in(&tmp, "code.rs", "// line 1\nfn hello() {\n    // line 3\n}");

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "hello",
        "path": tmp.path().to_str().unwrap(),
        "output_mode": "content"
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("code.rs"));
    assert!(result.content.contains(":2")); // line 2 contains "fn hello()"
}

#[tokio::test]
async fn test_grep_glob_filter_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    create_file_in(&tmp, "lib.rs", "pub fn find() {}");
    create_file_in(&tmp, "readme.md", "# find command");

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "find",
        "path": tmp.path().to_str().unwrap(),
        "glob": "*.rs",
        "output_mode": "files_with_matches"
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("lib.rs"));
    assert!(!result.content.contains("readme.md"));
}

#[tokio::test]
async fn test_grep_no_matches_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    create_file_in(&tmp, "test.txt", "nothing here");

    let tool = GrepTool::new();
    let args = json!({
        "pattern": "nonexistent_pattern_xyz",
        "path": tmp.path().to_str().unwrap(),
        "output_mode": "files_with_matches"
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("No matches"));
}
