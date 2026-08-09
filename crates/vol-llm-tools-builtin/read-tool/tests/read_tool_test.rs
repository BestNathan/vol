use std::sync::Arc;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError};
use vol_llm_tools_builtin_read::ReadTool;

fn sandbox_context() -> (ToolContext, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(temp_dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

#[tokio::test]
async fn test_read_file_success() {
    // Create a temp file with 3 lines
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(temp_file.path(), "line 1\nline 2\nline 3\n").unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": temp_file.path().to_str().unwrap()
    });
    let context = ToolContext::for_test();

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
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(temp_file.path(), content).unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "limit": 5
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify only lines 1-5 are present
    for i in 1..=5 {
        assert!(result.content.contains(&format!("{i}  |  line {i}")));
    }

    // Verify lines 6-10 are NOT present
    for i in 6..=10 {
        assert!(!result.content.contains(&format!("{i}  |  line {i}")));
    }
}

#[tokio::test]
async fn test_read_file_with_offset() {
    // Create a temp file with 5 lines
    let temp_file = tempfile::NamedTempFile::new().unwrap();
    let content = (1..=5)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(temp_file.path(), content).unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": temp_file.path().to_str().unwrap(),
        "offset": 2
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify lines 3-5 are present (offset 2 means skip first 2 lines)
    for i in 3..=5 {
        assert!(result.content.contains(&format!("{i}  |  line {i}")));
    }

    // Verify lines 1-2 are NOT present
    for i in 1..=2 {
        assert!(!result.content.contains(&format!("{i}  |  line {i}")));
    }
}

#[tokio::test]
async fn test_read_file_not_found() {
    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": "/nonexistent/path/file.txt"
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(matches!(err, ToolError::ExecutionFailed(_)));
}

#[tokio::test]
async fn test_read_file_in_restricted_sandbox() {
    let (ctx, tmp) = sandbox_context();

    // Write a file into the sandbox temp dir
    let test_file = tmp.path().join("hello.txt");
    std::fs::write(&test_file, "line A\nline B\nline C\n").unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": test_file.to_str().unwrap()
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("1  |  line A"));
    assert!(result.content.contains("2  |  line B"));
    assert!(result.content.contains("3  |  line C"));
}

#[tokio::test]
async fn test_read_file_offset_limit_in_sandbox() {
    let (ctx, tmp) = sandbox_context();

    let lines: Vec<String> = (1..=50).map(|i| format!("line {i}")).collect();
    let content = lines.join("\n");
    let test_file = tmp.path().join("many_lines.txt");
    std::fs::write(&test_file, &content).unwrap();

    let tool = ReadTool::new();
    // Offset 10 (skip first 10 lines), limit 5
    let args = serde_json::json!({
        "file_path": test_file.to_str().unwrap(),
        "offset": 10,
        "limit": 5
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);

    // Should show lines 11-15 (1-indexed)
    assert!(result.content.contains("11  |  line 11"));
    assert!(result.content.contains("15  |  line 15"));
    // Should NOT show lines 1-10 or 16+
    assert!(!result.content.contains("10  |  line 10"));
    assert!(!result.content.contains("16  |  line 16"));
}

#[tokio::test]
async fn test_read_file_not_found_in_sandbox() {
    let (ctx, _tmp) = sandbox_context();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": "/tmp/nonexistent_xyz.txt"
    });
    let result = tool.execute(&args, &ctx).await;
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), ToolError::ExecutionFailed(_)));
}

#[tokio::test]
async fn test_read_empty_file_in_sandbox() {
    let (ctx, tmp) = sandbox_context();

    let test_file = tmp.path().join("empty.txt");
    std::fs::write(&test_file, "").unwrap();

    let tool = ReadTool::new();
    let args = serde_json::json!({
        "file_path": test_file.to_str().unwrap()
    });
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.is_empty() || result.content == "");
}
