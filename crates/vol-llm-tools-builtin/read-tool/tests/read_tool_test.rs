use vol_llm_tool::{ExecutableTool, ToolContext, ToolError};
use vol_llm_tools_builtin_read::ReadTool;

#[tokio::test]
async fn test_read_file_success() {
    // Create a temp file with 3 lines
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), "line 1\nline 2\nline 3\n").unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": temp_file.path().to_str().unwrap()
    });
    let context = ToolContext::default();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify line numbers 1, 2, 3 are present
    assert!(result.content.contains("1  |  line 1"));
    assert!(result.content.contains("2  |  line 2"));
    assert!(result.content.contains("3  |  line 3"));
}

#[tokio::test]
async fn test_read_file_with_limit() {
    // Create a temp file with 10 lines
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let content = (1..=10)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(temp_file.path(), content).unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "limit": 5
    });
    let context = ToolContext::default();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify only lines 1-5 are present
    for i in 1..=5 {
        assert!(result.content.contains(&format!("{}  |  line {}", i, i)));
    }

    // Verify lines 6-10 are NOT present
    for i in 6..=10 {
        assert!(!result.content.contains(&format!("{}  |  line {}", i, i)));
    }
}

#[tokio::test]
async fn test_read_file_with_offset() {
    // Create a temp file with 5 lines
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let content = (1..=5)
        .map(|i| format!("line {}", i))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(temp_file.path(), content).unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "offset": 2
    });
    let context = ToolContext::default();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify lines 3-5 are present (offset 2 means skip first 2 lines)
    for i in 3..=5 {
        assert!(result.content.contains(&format!("{}  |  line {}", i, i)));
    }

    // Verify lines 1-2 are NOT present
    for i in 1..=2 {
        assert!(!result.content.contains(&format!("{}  |  line {}", i, i)));
    }
}

#[tokio::test]
async fn test_read_file_not_found() {
    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": "/nonexistent/path/file.txt"
    });
    let context = ToolContext::default();

    let result = tool.execute(&args, &context).await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, ToolError::NotFound(_)));
}
