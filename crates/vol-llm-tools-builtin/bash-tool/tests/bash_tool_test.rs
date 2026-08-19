//! Integration tests for the Bash tool

use serde_json::json;
use std::sync::Arc;
use vol_llm_sandbox::local::LocalSandbox;
use vol_llm_tool::{ExecutableTool, ToolContext};
use vol_llm_tools_builtin_bash::BashTool;

fn sandbox_context() -> (ToolContext, tempfile::TempDir) {
    let temp_dir = tempfile::TempDir::new().unwrap();
    let sandbox = Arc::new(LocalSandbox::new(Some(temp_dir.path().to_path_buf())));
    let ctx = ToolContext::default().with_sandbox(sandbox);
    (ctx, temp_dir)
}

#[tokio::test]
async fn test_bash_simple_command() {
    let tool = BashTool::new();
    let args = json!({
        "command": "echo hello"
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("hello"));
}

#[tokio::test]
async fn test_bash_rm_rf_blocked() {
    let tool = BashTool::new();
    let args = json!({
        "command": "rm -rf /"
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await;
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

    let result = tool.execute(&args, &ToolContext::for_test()).await;
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

    let result = tool.execute(&args, &ToolContext::for_test()).await;
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
    // Long sleep: guarantees the child is still alive when the 10ms timeout
    // fires (a short sleep races the 100ms poll loop and can exit naturally).
    let args = json!({
        "command": "sleep 30",
        "timeout": 10
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(err_str.contains("timed out") || err_str.contains("Timeout"));
}

#[tokio::test]
async fn test_bash_timeout_kills_process() {
    use std::time::Duration;
    use tokio::process::Command;

    // Kill any leftover `sleep 60` from previous test runs.
    // Anchored pattern: must NOT match the tool's own `bash -c sleep 60`
    // command line (an unanchored -f pattern would kill the bash wrapper).
    let _ = Command::new("pkill")
        .arg("-f")
        .arg("^sleep 60$")
        .output()
        .await;
    tokio::time::sleep(Duration::from_millis(50)).await;

    let tool = BashTool::new();
    let args = json!({
        "command": "sleep 60",
        "timeout": 10
    });

    let result = tool.execute(&args, &ToolContext::for_test()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{err}");
    assert!(
        err_str.contains("timed out"),
        "Expected timeout error, got: {err_str}"
    );

    // Give the kill sequence time to complete
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Verify the sleep process was killed (not orphaned)
    let check = Command::new("pgrep")
        .arg("-f")
        .arg("^sleep 60$")
        .output()
        .await
        .unwrap();
    assert!(
        check.stdout.is_empty(),
        "sleep 60 should have been killed, but pgrep found: {}",
        String::from_utf8_lossy(&check.stdout)
    );
}

#[tokio::test]
async fn test_bash_working_dir_parameter() {
    let (ctx, tmp) = sandbox_context();

    // Create a subdirectory
    let sub = tmp.path().join("sub");
    std::fs::create_dir_all(&sub).unwrap();

    let tool = BashTool::new();
    let args = json!({
        "command": "pwd",
        "working_dir": "sub"
    });

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    // The sandbox execute uses root as cwd by default, but working_dir
    // becomes the cwd in the CommandRequest. Verify output contains "sub".
    assert!(
        result.content.contains("sub"),
        "Expected working_dir to be reflected in output, got: {}",
        result.content
    );
}

#[tokio::test]
async fn test_bash_in_restricted_sandbox() {
    let (ctx, _tmp) = sandbox_context();

    let tool = BashTool::new();
    let args = json!({
        "command": "echo 'sandboxed'"
    });

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("sandboxed"));
}

#[tokio::test]
async fn test_bash_write_and_read_file_in_sandbox() {
    let (ctx, tmp) = sandbox_context();
    let tool = BashTool::new();

    // Write a file via bash
    let write_args = json!({
        "command": "echo 'content from bash' > output.txt"
    });
    let result = tool.execute(&write_args, &ctx).await.unwrap();
    assert!(result.success);

    // Verify file exists on disk (sandbox root is tmp.path())
    let file_path = tmp.path().join("output.txt");
    assert!(file_path.exists());
    let content = std::fs::read_to_string(&file_path).unwrap();
    assert_eq!(content.trim(), "content from bash");
}

#[tokio::test]
async fn test_bash_stdout_stderr_separation() {
    let (ctx, _tmp) = sandbox_context();
    let tool = BashTool::new();

    let args = json!({
        "command": "echo stdout-text && echo stderr-text >&2"
    });

    let result = tool.execute(&args, &ctx).await.unwrap();
    assert!(result.success);
    assert!(result.content.contains("stdout-text"));
    assert!(result.content.contains("stderr-text"));
    assert!(result.content.contains("stdout:"));
    assert!(result.content.contains("stderr:"));
}

#[tokio::test]
async fn test_bash_nonzero_exit_in_sandbox() {
    let (ctx, _tmp) = sandbox_context();
    let tool = BashTool::new();

    let args = json!({
        "command": "exit 42"
    });

    let result = tool.execute(&args, &ctx).await.unwrap();
    // Non-zero exit still succeeds at ToolResult level
    // (stderr/stdout captured, execution didn't crash)
    assert!(result.success);
}
