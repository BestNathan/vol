//! Integration tests for GrepTool

use serde_json::json;
use std::fs;
use std::io::Write;
use tempfile::tempdir;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_grep::GrepTool;

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
    assert!(content.contains("nested.rs"), "expected nested.rs in: {}", content);
    assert!(!content.contains("top.txt"));
}
