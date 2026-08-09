//! Single-tool tests with varied sandbox workdir configurations.
//!
//! Covers two variants of the execution environment:
//! - sandbox root placed at a subdirectory of the temp dir (simulating an
//!   agent whose `working_dir` is a subdirectory of the sandbox root)
//! - agent context carrying an `AgentDef` (real agent execution shape)

// Tests intentionally unwrap after asserting is_err()/is_ok(); the crate
// inherits the workspace's deny-level unwrap/expect lints.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod fixtures;

use serde_json::json;
use vol_llm_tool::ExecutableTool;
use vol_llm_tools_builtin::{BashTool, ReadTool, WriteTool};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Sandbox root = subdirectory (simulating agent working_dir)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_read_file_with_sandbox_root_at_subdirectory() {
    let (ctx, tmp) = fixtures::sandbox_in_subdir("agent-workspace");
    fixtures::populate_files(&tmp, &[("agent-workspace/readme.txt", "workspace content")]);

    let tool = ReadTool::new();
    let file_path = tmp.path().join("agent-workspace").join("readme.txt");
    let args = json!({"file_path": file_path.to_str().unwrap()});
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("workspace content"));
}

#[tokio::test]
async fn test_write_file_with_sandbox_root_at_subdirectory() {
    let (ctx, tmp) = fixtures::sandbox_in_subdir("agent-workspace");
    let file_path = tmp.path().join("agent-workspace").join("new_file.txt");

    let tool = WriteTool::new();
    let args = json!({"file_path": file_path.to_str().unwrap(), "content": "new workspace file"});
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);

    let written = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(written, "new workspace file");
}

#[tokio::test]
async fn test_bash_with_sandbox_root_at_subdirectory() {
    let (ctx, _tmp) = fixtures::sandbox_in_subdir("agent-workspace");

    let tool = BashTool::new();
    let args = json!({"command": "echo 'running in workspace'"});
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("running in workspace"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Agent context (with AgentDef)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_read_file_with_agent_context() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    fixtures::populate_files(&tmp, &[("data.txt", "agent data")]);

    let sandbox = ctx.sandbox.clone();
    let ctx = fixtures::agent_context(sandbox, "test-agent", None);

    let tool = ReadTool::new();
    let file_path = tmp.path().join("data.txt");
    let args = json!({"file_path": file_path.to_str().unwrap()});
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("agent data"));
}

#[tokio::test]
async fn test_bash_with_agent_context() {
    let (ctx, _tmp) = fixtures::sandbox_in_tempdir();
    let sandbox = ctx.sandbox.clone();
    let ctx = fixtures::agent_context(sandbox, "coding-agent", Some("/workspace"));

    let tool = BashTool::new();
    let args = json!({"command": "echo 'agent execution'"});
    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("agent execution"));
}
