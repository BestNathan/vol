//! Edit tool tests.

use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_edit::EditTool;

fn create_temp_file(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(content.as_bytes())
        .expect("Failed to write to temp file");
    file
}

fn sandbox_context() -> (ToolContext, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(temp_dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

fn create_temp_file_in(tmp: &tempfile::TempDir, name: &str, content: &str) -> std::path::PathBuf {
    let path = tmp.path().join(name);
    std::fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn test_edit_unique_string() {
    let tool = EditTool::new();
    let file = create_temp_file("Hello world\nThis is a test\nGoodbye universe");

    let args = json!({
        "file_path": file.path().to_str().unwrap(),
        "old_string": "world",
        "new_string": "Rust"
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await.unwrap();
    assert!(result.success);
    assert!(result
        .content
        .contains("Successfully replaced 1 occurrence(s)"));

    // Verify file content
    let content = std::fs::read_to_string(file.path()).unwrap();
    assert_eq!(content, "Hello Rust\nThis is a test\nGoodbye universe");
}

#[tokio::test]
async fn test_edit_multiple_replace_all() {
    let tool = EditTool::new();
    let file = create_temp_file("foo bar foo\nfoo baz foo");

    let args = json!({
        "file_path": file.path().to_str().unwrap(),
        "old_string": "foo",
        "new_string": "QUX",
        "replace_all": true
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await.unwrap();
    assert!(result.success);
    assert!(result
        .content
        .contains("Successfully replaced 4 occurrence(s)"));

    // Verify file content
    let content = std::fs::read_to_string(file.path()).unwrap();
    assert_eq!(content, "QUX bar QUX\nQUX baz QUX");
}

#[tokio::test]
async fn test_edit_not_unique_error() {
    let tool = EditTool::new();
    let file = create_temp_file("foo bar foo\nbaz qux");

    let args = json!({
        "file_path": file.path().to_str().unwrap(),
        "old_string": "foo",
        "new_string": "QUX"
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Found 2 occurrences"));
    assert!(err.to_string().contains("replace_all=true"));

    // Verify file content unchanged
    let content = std::fs::read_to_string(file.path()).unwrap();
    assert_eq!(content, "foo bar foo\nbaz qux");
}

#[tokio::test]
async fn test_edit_not_found_error() {
    let tool = EditTool::new();
    let file = create_temp_file("Hello world\nThis is a test");

    let args = json!({
        "file_path": file.path().to_str().unwrap(),
        "old_string": "notfound",
        "new_string": "replacement"
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not found in file"));

    // Verify file content unchanged
    let content = std::fs::read_to_string(file.path()).unwrap();
    assert_eq!(content, "Hello world\nThis is a test");
}

#[tokio::test]
async fn test_edit_in_restricted_sandbox() {
    let (ctx, tmp) = sandbox_context();
    let file_path = create_temp_file_in(&tmp, "test.txt", "alpha beta gamma");

    let tool = EditTool::new();
    let args = json!({
        "file_path": file_path.to_str().unwrap(),
        "old_string": "beta",
        "new_string": "delta"
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result
        .content
        .contains("Successfully replaced 1 occurrence"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "alpha delta gamma");
}

#[tokio::test]
async fn test_edit_replace_all_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    let file_path = create_temp_file_in(&tmp, "test.txt", "x y x z x");

    let tool = EditTool::new();
    let args = json!({
        "file_path": file_path.to_str().unwrap(),
        "old_string": "x",
        "new_string": "Q",
        "replace_all": true
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result
        .content
        .contains("Successfully replaced 3 occurrence"));

    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "Q y Q z Q");
}

#[tokio::test]
async fn test_edit_multi_occurrence_without_replace_all_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    let file_path = create_temp_file_in(&tmp, "test.txt", "dup dup unique");

    let tool = EditTool::new();
    let args = json!({
        "file_path": file_path.to_str().unwrap(),
        "old_string": "dup",
        "new_string": "new"
    });
    let result = tool.execute(&args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Found 2 occurrences"));

    // File unchanged
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, "dup dup unique");
}

#[tokio::test]
async fn test_edit_empty_old_string_rejected() {
    let (ctx, tmp) = sandbox_context();
    let file_path = create_temp_file_in(&tmp, "test.txt", "some content");

    let tool = EditTool::new();
    let args = json!({
        "file_path": file_path.to_str().unwrap(),
        "old_string": "",
        "new_string": "replacement"
    });
    let result = tool.execute(&args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("old_string cannot be empty"));
}
