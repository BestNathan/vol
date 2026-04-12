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
