use vol_llm_tool::{ExecutableTool, ToolContext, ToolError};
use vol_llm_tools_builtin_write::WriteTool;

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
    let context = ToolContext::default();

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
    tokio::fs::write(&file_path, original_content).await.unwrap();

    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": file_path.to_str().unwrap(),
        "content": new_content
    });
    let context = ToolContext::default();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify file was overwritten with new content
    let written_content = tokio::fs::read_to_string(&file_path).await.unwrap();
    assert_eq!(written_content, new_content);
}

#[tokio::test]
async fn test_write_parent_not_exist() {
    let tool = WriteTool::new();
    let args = serde_json::json!({
        "file_path": "/nonexistent/directory/file.txt",
        "content": "Some content"
    });
    let context = ToolContext::default();

    let result = tool.execute(&args, &context).await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, ToolError::ExecutionFailed(_)));

    // Verify the error message mentions parent directory
    let err_msg = err.to_string();
    assert!(err_msg.contains("Parent directory does not exist"));
}
