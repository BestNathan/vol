//! Edit tool tests.

use serde_json::json;
use std::io::Write;
use tempfile::NamedTempFile;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_edit::EditTool;

fn create_temp_file(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(content.as_bytes())
        .expect("Failed to write to temp file");
    file
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
