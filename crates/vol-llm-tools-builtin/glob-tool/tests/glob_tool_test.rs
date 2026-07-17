use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_glob::GlobTool;

#[tokio::test]
async fn test_glob_basic() {
    // Create a temp directory with src/ containing main.rs and lib.rs
    let temp_dir = tempfile::TempDir::new().unwrap();
    let src_dir = temp_dir.path().join("src");
    std::fs::create_dir(&src_dir).unwrap();

    std::fs::write(src_dir.join("main.rs"), "fn main() {}").unwrap();
    std::fs::write(src_dir.join("lib.rs"), "pub fn lib() {}").unwrap();

    let tool = GlobTool::new();
    let args = serde_json::json!({
        "pattern": "*.rs",
        "path": src_dir.to_str().unwrap()
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify both files are found
    assert!(result.content.contains("main.rs"));
    assert!(result.content.contains("lib.rs"));
}

#[tokio::test]
async fn test_glob_no_matches() {
    // Create a temp directory with a file
    let temp_dir = tempfile::TempDir::new().unwrap();
    std::fs::write(temp_dir.path().join("test.txt"), "content").unwrap();

    let tool = GlobTool::new();
    let args = serde_json::json!({
        "pattern": "*.nonexistent",
        "path": temp_dir.path().to_str().unwrap()
    });
    let context = ToolContext::for_test();

    let result = tool.execute(&args, &context).await.unwrap();
    assert!(result.success);

    // Verify "No files matched" message
    assert!(result.content.contains("No files matched"));
}
