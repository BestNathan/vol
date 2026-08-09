use std::sync::Arc;

use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_write::WriteTool;

/// Create a ToolContext whose sandbox is rooted at a fresh temp directory
/// (restricted sandbox, as opposed to `ToolContext::for_test()` which roots at `/`).
fn sandbox_context() -> (ToolContext, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(temp_dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

#[tokio::test]
async fn test_write_new_file() {
    // Create a temp directory for the test
    let temp_dir = tempfile::TempDir::new().unwrap();
    let file_path = temp_dir.path().join("new_file.txt");
    let content = "Hello, World!\nThis is a test file.";

    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": file_path.to_str().unwrap(),
        "content": content
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify file was created with correct content
    let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(written_content, content);
}

#[tokio::test]
async fn test_write_overwrite_file() {
    // Create a temp directory and file for the test
    let temp_dir = tempfile::TempDir::new().unwrap();
    let file_path = temp_dir.path().join("existing_file.txt");
    let original_content = "Original content";
    let new_content = "Overwritten content";

    // Create the file with original content
    tokio::fs::write(&file_path, original_content)
        .await
        .unwrap();

    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": file_path.to_str().unwrap(),
        "content": new_content
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify file was overwritten with new content
    let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(written_content, new_content);
}

#[tokio::test]
async fn test_write_creates_parent_dirs() {
    // WriteTool creates parent directories if they don't exist
    let temp_dir = tempfile::TempDir::new().unwrap();
    let nested_path = temp_dir
        .path()
        .join("a")
        .join("b")
        .join("c")
        .join("file.txt");
    let content = "Nested content";

    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": nested_path.to_str().unwrap(),
        "content": content
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    let written_content = tokio::fs::read_to_string(&nested_path).await.unwrap();
    assert_eq!(written_content, content);
}

#[tokio::test]
async fn test_write_in_restricted_sandbox() {
    let (ctx, tmp) = sandbox_context();

    let file_path = tmp.path().join("output.txt");
    let content = "sandboxed content";

    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": file_path.to_str().unwrap(),
        "content": content
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("Successfully wrote"));

    // Verify on disk
    let written = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(written, content);
}

#[tokio::test]
async fn test_write_creates_parent_dirs_in_sandbox() {
    let (ctx, tmp) = sandbox_context();

    let nested_path = tmp.path().join("deep").join("nested").join("file.txt");
    let content = "deeply nested";

    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": nested_path.to_str().unwrap(),
        "content": content
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);

    let written = std::fs::read_to_string(&nested_path).unwrap();
    assert_eq!(written, content);
}

#[tokio::test]
async fn test_write_empty_content_in_sandbox() {
    let (ctx, tmp) = sandbox_context();

    let file_path = tmp.path().join("empty.txt");
    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": file_path.to_str().unwrap(),
        "content": ""
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("Successfully wrote 0 bytes"));

    let written = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(written, "");
}

#[tokio::test]
async fn test_write_overwrite_in_sandbox() {
    let (ctx, tmp) = sandbox_context();

    let file_path = tmp.path().join("overwrite.txt");
    std::fs::write(&file_path, "original").unwrap();

    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": file_path.to_str().unwrap(),
        "content": "replaced"
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);

    let written = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(written, "replaced");
}
