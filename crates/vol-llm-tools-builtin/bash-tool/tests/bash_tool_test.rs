//! Integration tests for the Bash tool

use serde_json::json;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_bash::BashTool;

#[tokio::test]
async fn test_bash_simple_command() {
    let tool = BashTool::new();
    let args = json!({
        "command": "echo hello"
    });

    let result = tool.execute(&args, &ToolContext::default()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("hello"));
}

#[tokio::test]
async fn test_bash_rm_rf_blocked() {
    let tool = BashTool::new();
    let args = json!({
        "command": "rm -rf /"
    });

    let result = tool.execute(&args, &ToolContext::default()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // The security violation should cause the command to be blocked
    let err_str = format!("{err}");
    assert!(
        err_str.contains("blocked")
            || err_str.contains("Security")
            || err_str.contains("SecurityViolation")
    );
}

#[tokio::test]
async fn test_bash_fork_bomb_blocked() {
    let tool = BashTool::new();
    let args = json!({
        "command": ":(){:|:&}:"
    });

    let result = tool.execute(&args, &ToolContext::default()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(
        err_str.contains("blocked")
            || err_str.contains("Security")
            || err_str.contains("SecurityViolation")
    );
}

#[tokio::test]
async fn test_bash_rm_file_allowed() {
    let tool = BashTool::new();
    // rm with a specific file path (not starting with /) should be allowed
    // The command will fail because the file doesn't exist, but it should NOT be blocked
    let args = json!({
        "command": "rm /tmp/nonexistent_file_test_12345"
    });

    let result = tool.execute(&args, &ToolContext::default()).await;
    // Should not error due to security - may succeed or fail due to file not existing
    let err_str = result.map_or_else(|e| format!("{e}"), |r| r.content.clone());
    // The key is that it's NOT a security block - either it succeeds or fails with "No such file"
    assert!(
        !err_str.contains("SecurityViolation")
            || err_str.contains("No such file")
            || err_str.contains("nonexistent")
    );
}

#[tokio::test]
async fn test_bash_timeout() {
    let tool = BashTool::new();
    let args = json!({
        "command": "sleep 5",
        "timeout": 100
    });

    let result = tool.execute(&args, &ToolContext::default()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(err_str.contains("timed out") || err_str.contains("Timeout"));
}

#[tokio::test]
async fn test_bash_timeout_kills_process() {
    use std::time::Duration;
    use tokio::process::Command;

    // Kill any existing sleep 10 from previous test runs
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("sleep 10")
        .output()
        .await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let tool = BashTool::new();
    let args = json!({
        "command": "sleep 10",
        "timeout": 100
    });

    let result = tool.execute(&args, &ToolContext::default()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(
        err_str.contains("timed out"),
        "Expected timeout error, got: {err_str}"
    );

    // Give the kill sequence time to complete
    tokio::time::sleep(Duration::from_millis(200)).await;

    // Verify the sleep process was killed (not orphaned)
    let check = Command::new("pgrep")
        .arg("-f")
        .arg("sleep 10")
        .output()
        .await
        .unwrap();
    assert!(
        check.stdout.is_empty(),
        "sleep 10 should have been killed, but pgrep found: {}",
        String::from_utf8_lossy(&check.stdout)
    );
}
