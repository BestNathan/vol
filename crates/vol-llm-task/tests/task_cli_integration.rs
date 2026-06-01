//! Integration test: simulates an LLM agent using TaskCliTool end-to-end.
//!
//! Tests call `ExecutableTool::execute()` directly with JSON args,
//! mimicking the agent loop: JSON command string → parse → execute → ToolResult.
//! No actual LLM involved.

use std::sync::Arc;

use vol_llm_task::tools::TaskCliTool;
use vol_llm_task::{InMemoryTaskStore, TaskStatus, TaskStore};
use vol_llm_tool::{ExecutableTool, ToolContext, ToolResult};

/// Helper: execute a CLI command string through TaskCliTool.
async fn run(tool: &TaskCliTool, ctx: &ToolContext, command: &str) -> Result<ToolResult, vol_llm_tool::ToolError> {
    tool.execute(&serde_json::json!({"command": command}), ctx).await
}

/// Simulate a full agent session using CLI-style task commands.
#[tokio::test]
async fn test_full_agent_session_with_cli_tool() {
    let store = Arc::new(InMemoryTaskStore::new());
    let tool = TaskCliTool::new(store.clone());
    let ctx = ToolContext::default();

    // Turn 1: Agent checks what tasks exist
    let r = run(&tool, &ctx, "list").await.unwrap();
    println!("[T1 list] {}", r.content);
    assert!(r.success);
    assert!(r.content.contains("No tasks found"));

    // Turn 2: Agent creates a task (detailed)
    let r = run(&tool, &ctx,
        "create --name 'Implement login' --desc 'Add OAuth flow to auth module' --assignee coding-agent"
    ).await.unwrap();
    println!("[T2 create] {}", r.content);
    assert!(r.success);
    assert!(r.content.contains("Implement login"));
    assert!(r.content.contains("Task t1"));

    // Turn 3: Agent quick-creates two more tasks
    for name in &["Write tests", "Update docs"] {
        let r = run(&tool, &ctx, &format!("+task --name '{}'", name)).await.unwrap();
        println!("[T3 +task] {}", r.content);
        assert!(r.success);
        assert!(r.content.contains(name));
    }

    // Turn 4: Agent lists all pending tasks
    let r = run(&tool, &ctx, "list --status pending").await.unwrap();
    println!("[T4 list] {}", r.content);
    assert!(r.success);
    assert!(r.content.contains("Implement login"));
    assert!(r.content.contains("Write tests"));
    assert!(r.content.contains("Update docs"));

    // Turn 5: Agent checks parameters for update
    let r = run(&tool, &ctx, "scheme update").await.unwrap();
    println!("[T5 scheme] {}", r.content);
    assert!(r.success);
    assert!(r.content.contains("--status"));
    assert!(r.content.contains("--id"));

    // Turn 6: Agent updates task 1 to Running
    let r = run(&tool, &ctx, "update --id 1 --status running").await.unwrap();
    println!("[T6 update] {}", r.content);
    assert!(r.success);
    assert!(r.content.contains("Task t1 updated"));

    // Turn 7: Agent checks task 1 details
    let r = run(&tool, &ctx, "get --id 1").await.unwrap();
    println!("[T7 get] {}", r.content);
    assert!(r.success);
    assert!(r.content.contains("Running"));
    assert!(r.content.contains("Implement login"));
    assert!(r.content.contains("coding-agent"));

    // Turn 8: Agent claims first ready task (one of t2 or t3)
    let r = run(&tool, &ctx, "+claim").await.unwrap();
    println!("[T8 +claim] {}", r.content);
    assert!(r.success);
    assert!(r.content.contains("Task t"));
    assert!(r.content.contains("claimed"));
    assert!(r.content.contains("Running"));

    // Turn 9: Agent marks task 1 as done
    let r = run(&tool, &ctx, "+done --id 1").await.unwrap();
    println!("[T9 +done] {}", r.content);
    assert!(r.success);

    // Turn 10: Agent verifies task 1 is completed (JSON mode)
    let r = run(&tool, &ctx, "get --id 1 --json").await.unwrap();
    println!("[T10 get json] {}", r.content);
    assert!(r.success);
    assert!(r.content.starts_with('{'));
    let parsed: serde_json::Value = serde_json::from_str(&r.content).unwrap();
    assert_eq!(parsed["status"], "Completed");

    // Verify final store state
    let mut tasks: Vec<vol_llm_task::Task> = store.list(None).await.unwrap();
    tasks.sort_by_key(|t| t.id);
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].status, TaskStatus::Completed); // t1: +done
    // t2 and t3: one was +claimed (Running), the other still Pending
    let statuses: Vec<_> = tasks.iter().skip(1).map(|t| t.status).collect();
    assert!(statuses.contains(&TaskStatus::Running), "expected one Running task");
    assert!(statuses.contains(&TaskStatus::Pending), "expected one Pending task");
}

/// Simulate a full session using JSON output mode.
#[tokio::test]
async fn test_json_output_mode() {
    let store = Arc::new(InMemoryTaskStore::new());
    let tool = TaskCliTool::new(store.clone());
    let ctx = ToolContext::default();

    // Create with --json flag
    let r = run(&tool, &ctx, "create --name 'JSON task' --desc 'test' --json").await.unwrap();
    assert!(r.success);
    let v: serde_json::Value = serde_json::from_str(&r.content).unwrap();
    assert_eq!(v["subject"], "JSON task");

    // List with --json flag
    let r = run(&tool, &ctx, "list --json").await.unwrap();
    let arr: Vec<serde_json::Value> = serde_json::from_str(&r.content).unwrap();
    assert_eq!(arr.len(), 1);
}

/// Test error cases across subcommands.
#[tokio::test]
async fn test_error_handling() {
    let store = Arc::new(InMemoryTaskStore::new());
    let tool = TaskCliTool::new(store);
    let ctx = ToolContext::default();

    // Missing required param
    let r = run(&tool, &ctx, "create --name 'oops'").await;
    assert!(r.is_err());

    // Invalid subcommand
    let r = run(&tool, &ctx, "frobnicate").await;
    assert!(r.is_err());

    // Get non-existent task
    let r = run(&tool, &ctx, "get --id 999").await;
    assert!(r.is_err());
    assert!(r.unwrap_err().to_string().contains("not found"));
}

/// Test tool sensitivity (HITL approval) covers all subcommand types.
#[tokio::test]
async fn test_sensitivity_rules() {
    let store = Arc::new(InMemoryTaskStore::new());
    let tool = TaskCliTool::new(store);

    // Read operations are safe
    for cmd in &["get --id 1", "list", "scheme create", "claim --id 1"] {
        let s = tool.sensitivity(&serde_json::json!({"command": cmd}));
        assert!(
            matches!(s, vol_llm_tool::ToolSensitivity::Safe),
            "expected Safe for '{cmd}', got RequiresApproval"
        );
    }

    // Mutating operations require approval
    for cmd in &[
        "update --id 1 --status completed",
        "stop --id 1",
        "+done --id 1",
        "+claim",
    ] {
        let s = tool.sensitivity(&serde_json::json!({"command": cmd}));
        assert!(
            matches!(s, vol_llm_tool::ToolSensitivity::RequiresApproval { .. }),
            "expected RequiresApproval for '{cmd}', got Safe"
        );
    }
}

/// Test that agent definition file exists and is well-formed.
#[test]
fn test_agent_definition_exists() {
    let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent().unwrap()
        .join(".agents").join("agents").join("task-cli-test.md");

    if !path.exists() {
        eprintln!("Skipping agent def check: {} not found", path.display());
        return;
    }

    let content = std::fs::read_to_string(&path).unwrap();
    assert!(content.contains("name: task-cli-test"));
    assert!(content.contains("tools:"));
    assert!(content.contains("task"));
    assert!(content.contains("---"));
}

/// Test the scheme subcommand for every known subcommand.
#[tokio::test]
async fn test_scheme_all_subcommands() {
    let store = Arc::new(InMemoryTaskStore::new());
    let tool = TaskCliTool::new(store);
    let ctx = ToolContext::default();

    for sub in &["create", "update", "get", "list", "stop", "output", "claim", "+task", "+done", "+claim"] {
        let r = run(&tool, &ctx, &format!("scheme {}", sub)).await.unwrap();
        assert!(r.success, "scheme {} failed: {}", sub, r.content);
        assert!(!r.content.is_empty(), "scheme {} returned empty", sub);
    }

    // scheme without args lists all subcommands
    let r = run(&tool, &ctx, "scheme").await.unwrap();
    assert!(r.success);
    assert!(r.content.contains("create"));
    assert!(r.content.contains("+task"));
}
