//! Integration tests exercising ToolRegistry execute path with real tools.
//!
//! Unlike the tool_chain tests which call tool.execute() directly, these tests
//! go through `ToolRegistry::execute()` — the same path the ReAct agent uses.

// Tests intentionally unwrap after asserting is_err()/is_ok(); the crate
// inherits the workspace's deny-level unwrap/expect lints.
#![allow(clippy::unwrap_used, clippy::expect_used)]

mod fixtures;

use serde_json::json;
use vol_llm_core::ToolCall;
use vol_llm_tool::ToolRegistry;
use vol_llm_tools_builtin::{BashTool, EditTool, ReadTool, WriteTool};

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Registry execute: write → read chain
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_registry_execute_write_and_read() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    let mut registry = ToolRegistry::new();
    registry.register(WriteTool::new());
    registry.register(ReadTool::new());

    let file_path = tmp.path().join("doc.txt").to_str().unwrap().to_string();

    // Write via registry
    let write_call = ToolCall {
        id: "call_1".into(),
        name: "write_file".into(),
        arguments: json!({"file_path": file_path, "content": "registry chain test"}).to_string(),
        r#type: "function".into(),
    };
    let result = registry.execute(&write_call, &ctx).await.unwrap();
    assert!(result.success);
    assert_eq!(result.call_id, "call_1");

    // Read via registry
    let read_call = ToolCall {
        id: "call_2".into(),
        name: "read_file".into(),
        arguments: json!({"file_path": file_path}).to_string(),
        r#type: "function".into(),
    };
    let result = registry.execute(&read_call, &ctx).await.unwrap();
    assert!(result.success);
    assert_eq!(result.call_id, "call_2");
    assert!(result.content.contains("registry chain test"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Registry execute: bash → read chain
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_registry_execute_bash_and_read() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    let mut registry = ToolRegistry::new();
    registry.register(BashTool::new());
    registry.register(ReadTool::new());

    // Write a file via bash
    let output_path = tmp.path().join("output.txt");
    let bash_call = ToolCall {
        id: "c1".into(),
        name: "bash".into(),
        arguments:
            json!({"command": format!("echo 'bash output' > {}", output_path.to_str().unwrap())})
                .to_string(),
        r#type: "function".into(),
    };
    let result = registry.execute(&bash_call, &ctx).await.unwrap();
    assert!(result.success);

    // Read it back via read_file
    let read_call = ToolCall {
        id: "c2".into(),
        name: "read_file".into(),
        arguments: json!({"file_path": output_path.to_str().unwrap()}).to_string(),
        r#type: "function".into(),
    };
    let result = registry.execute(&read_call, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("bash output"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Registry execute: write → edit → read
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_registry_execute_write_edit_read() {
    let (ctx, tmp) = fixtures::sandbox_in_tempdir();
    let mut registry = ToolRegistry::new();
    registry.register(WriteTool::new());
    registry.register(EditTool::new());
    registry.register(ReadTool::new());

    let file_path = tmp.path().join("todo.md").to_str().unwrap().to_string();

    // Write
    registry
        .execute(
            &ToolCall {
                id: "1".into(),
                name: "write_file".into(),
                arguments: json!({"file_path": file_path, "content": "TODO: finish"}).to_string(),
                r#type: "function".into(),
            },
            &ctx,
        )
        .await
        .unwrap();

    // Edit
    registry
        .execute(
            &ToolCall {
                id: "2".into(),
                name: "edit_file".into(),
                arguments:
                    json!({"file_path": file_path, "old_string": "TODO", "new_string": "DONE"})
                        .to_string(),
                r#type: "function".into(),
            },
            &ctx,
        )
        .await
        .unwrap();

    // Read and verify
    let result = registry
        .execute(
            &ToolCall {
                id: "3".into(),
                name: "read_file".into(),
                arguments: json!({"file_path": file_path}).to_string(),
                r#type: "function".into(),
            },
            &ctx,
        )
        .await
        .unwrap();
    assert!(result.content.contains("DONE: finish"));
    assert!(!result.content.contains("TODO: finish"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Registry filter → execute
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_registry_filter_then_execute() {
    let (ctx, _tmp) = fixtures::sandbox_in_tempdir();
    let mut registry = ToolRegistry::new();
    registry.register(ReadTool::new());
    registry.register(BashTool::new());

    // Filter to only allow ReadTool
    let filtered = registry.filter(Some(&["read_file"]), None);

    // Bash should not be available
    let bash_call = ToolCall {
        id: "c1".into(),
        name: "bash".into(),
        arguments: json!({"command": "echo hi"}).to_string(),
        r#type: "function".into(),
    };
    let result = filtered.execute(&bash_call, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown tool"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Registry execute: unknown tool error
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[tokio::test]
async fn test_registry_execute_unknown_tool_error() {
    let (ctx, _tmp) = fixtures::sandbox_in_tempdir();
    let registry = ToolRegistry::new();

    let call = ToolCall {
        id: "c1".into(),
        name: "nonexistent_tool_xyz".into(),
        arguments: "{}".into(),
        r#type: "function".into(),
    };
    let result = registry.execute(&call, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown tool"));
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Registry: definitions includes all registered tools
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[test]
fn test_registry_definitions_includes_all() {
    let mut registry = ToolRegistry::new();
    registry.register(ReadTool::new());
    registry.register(WriteTool::new());
    registry.register(BashTool::new());

    let defs = registry.definitions();
    assert_eq!(defs.len(), 3);
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"write_file"));
    assert!(names.contains(&"bash"));
}
