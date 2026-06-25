use vol_llm_tool::{ExecutableTool, ToolContext};
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
    tokio::fs::write(&file_path, original_content)
        .await
        .unwrap();

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
    let context = ToolContext::default();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    let written_content = tokio::fs::read_to_string(&nested_path).await.unwrap();
    assert_eq!(written_content, content);
}
