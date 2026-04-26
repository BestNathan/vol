# BashTool Timeout Graceful Termination Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix BashTool so that when a command times out, the child process and its entire process group are killed (SIGTERM → wait 5s → SIGKILL) instead of being orphaned.

**Architecture:** Replace `cmd.output()` wrapped in `tokio::time::timeout` with `cmd.spawn()` + `process_group(0)` to create an isolated process group. On timeout, send SIGTERM to the entire group via `nix::sys::signal::kill(Pid::from_pid(-pgid), SIGTERM)`, poll with `try_wait()` for 5 seconds, then SIGKILL if still alive. Collect partial output and return a descriptive error message.

**Tech Stack:** Rust, tokio, nix (Unix signals), async_trait

**Branch:** Work on `master` at `/root/nq-deribit/`

---

### Task 1: Add nix dependency

**Files:**
- Modify: `crates/vol-llm-tools-builtin/bash-tool/Cargo.toml`

- [ ] **Step 1: Add nix to Cargo.toml**

Add this line after `regex = "1.10"` in `[dependencies]`:

```toml
nix = { version = "0.29", features = ["signal"] }
```

- [ ] **Step 2: Verify dependency resolution**

Run: `cargo check -p vol-llm-tools-builtin-bash 2>&1 | head -5`
Expected: Downloads nix crate, no compile errors (we haven't used it yet).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tools-builtin/bash-tool/Cargo.toml
git commit -m "chore: add nix dependency for bash-tool process group signaling"
```

---

### Task 2: Write failing test — timeout should kill orphaned processes

**Files:**
- Modify: `crates/vol-llm-tools-builtin/bash-tool/tests/bash_tool_test.rs`

- [ ] **Step 1: Add the new test**

Append this test to the end of the test file:

```rust
#[tokio::test]
async fn test_bash_timeout_kills_process() {
    use std::time::Duration;
    use tokio::process::Command;

    // Kill any existing sleep 10 from previous test runs
    let _ = Command::new("pkill").arg("-f").arg("sleep 10").output().await;
    tokio::time::sleep(Duration::from_millis(200)).await;

    let tool = BashTool::new();
    let args = json!({
        "command": "sleep 10",
        "timeout": 100
    });

    let result = tool.execute(&args, &ToolContext::default()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{}", err);
    assert!(err_str.contains("timed out"), "Expected timeout error, got: {}", err_str);

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
```

- [ ] **Step 2: Run the new test to verify it fails**

Run: `cargo test -p vol-llm-tools-builtin-bash test_bash_timeout_kills_process -- --nocapture 2>&1`
Expected: FAIL — the timeout currently only drops the Future, `sleep 10` continues running, and `pgrep` finds it.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tools-builtin/bash-tool/tests/bash_tool_test.rs
git commit -m "test: add test verifying timeout kills orphaned processes"
```

---

### Task 3: Refactor execute() — switch from cmd.output() to cmd.spawn() with process_group

**Files:**
- Modify: `crates/vol-llm-tools-builtin/bash-tool/src/lib.rs` (lines 168-201, the execute block)

- [ ] **Step 1: Add Unix import and constant**

Add after `use tokio::time::timeout;` at line 8:

```rust
#[cfg(unix)]
use std::os::unix::process::CommandExt as _;
```

Add after `const DEFAULT_TIMEOUT_MS: u64 = 120_000;` at line 20:

```rust
/// Grace period after SIGTERM before escalating to SIGKILL.
const SIGTERM_GRACE_PERIOD: Duration = Duration::from_secs(5);
```

- [ ] **Step 2: Replace the execute body (lines 169-201)**

Replace the entire block from `// Build command` through the timeout match (lines 169-201) with this new code:

```rust
        // Build command
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(&params.command);

        // Create a new process group so we can kill the entire tree on timeout.
        #[cfg(unix)]
        cmd.process_group(0);

        // Set working directory: explicit param > sandbox root > process cwd
        if let Some(ref working_dir) = params.working_dir {
            cmd.current_dir(working_dir);
        } else if let Some(ref sandbox) = context.sandbox {
            cmd.current_dir(sandbox.root_path());
        }

        // Determine timeout
        let timeout_duration = params
            .timeout
            .map(Duration::from_millis)
            .unwrap_or(self.default_timeout);

        // Spawn and collect output with timeout
        let mut child = cmd
            .spawn()
            .map_err(|e| ToolError::ExecutionFailed(format!("Failed to spawn command: {}", e)))?;

        let output = match timeout(timeout_duration, child.wait_with_output()).await {
            Ok(Ok(output)) => output,
            Ok(Err(e)) => {
                // Process spawned but failed to execute (e.g., invalid binary)
                return Err(ToolError::ExecutionFailed(format!(
                    "Failed to execute command: {}",
                    e
                )));
            }
            Err(_) => {
                // Timeout: kill entire process group
                let pgid = child.id() as i32;
                let _ = nix::sys::signal::kill(
                    nix::unistd::Pid::from_pid(-pgid),
                    nix::sys::signal::Signal::SIGTERM,
                );

                // Poll with try_wait so we don't consume the output pipes
                let grace_start = std::time::Instant::now();
                let exited = loop {
                    if grace_start.elapsed() > SIGTERM_GRACE_PERIOD {
                        break false;
                    }
                    match child.try_wait() {
                        Ok(Some(_)) => break true,
                        Ok(None) => {}
                        Err(_) => break false,
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                };

                let output = child.wait_with_output().await.unwrap_or_default();

                if exited {
                    return Err(ToolError::ExecutionFailed(format!(
                        "Command timed out after {:?}. Sent SIGTERM to process group {}, process exited.",
                        timeout_duration, pgid
                    )));
                } else {
                    // Still running — SIGKILL the group
                    let _ = nix::sys::signal::kill(
                        nix::unistd::Pid::from_pid(-pgid),
                        nix::sys::signal::Signal::SIGKILL,
                    );
                    let _ = child.wait().await;
                    return Err(ToolError::ExecutionFailed(format!(
                        "Command timed out after {:?}. Sent SIGTERM then SIGKILL to process group {}.",
                        timeout_duration, pgid
                    )));
                }
            }
        };
```

- [ ] **Step 3: Remove unused `timeout` import**

The `timeout` function is still used, so no import removal needed.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-tools-builtin-bash 2>&1`
Expected: Compiles successfully. There may be a dead code warning for `BashToolError::Timeout` — this is fine, the variant is still used in the error enum.

- [ ] **Step 5: Run all bash-tool tests**

Run: `cargo test -p vol-llm-tools-builtin-bash -- --nocapture 2>&1`
Expected: All 6 tests pass including the new `test_bash_timeout_kills_process`.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tools-builtin/bash-tool/src/lib.rs
git commit -m "fix: kill process group on bash tool timeout instead of orphaning"
```

---

### Task 4: Full workspace verification

**Files:** No changes — verification only

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace 2>&1 | tail -10`
Expected: All crates compile. No new errors introduced.

- [ ] **Step 2: Run all workspace tests**

Run: `cargo test --workspace --lib 2>&1 | tail -20`
Expected: All tests pass (except pre-existing vol-llm-provider failure if any).

- [ ] **Step 3: Verify existing timeout test still passes**

Run: `cargo test -p vol-llm-tools-builtin-bash test_bash_timeout -- --nocapture 2>&1`
Expected: PASS — the existing test checks for "timed out" or "Timeout" in the error string, which the new error messages satisfy.

- [ ] **Step 4: Commit if any fixes were needed**

Only commit if changes were made in steps 1-3.

---

## Summary of Changes

| File | Change | Lines Changed |
|------|--------|--------------|
| `crates/vol-llm-tools-builtin/bash-tool/Cargo.toml` | Add `nix` dependency | +1 |
| `crates/vol-llm-tools-builtin/bash-tool/src/lib.rs` | Replace `cmd.output()` + `timeout()` with `cmd.spawn()` + process group + SIGTERM/SIGKILL sequence | ~55 → ~85 |
| `crates/vol-llm-tools-builtin/bash-tool/tests/bash_tool_test.rs` | Add `test_bash_timeout_kills_process` integration test | +40 |

**Behavioral change:** Before timeout → process orphaned. After timeout → SIGTERM to process group → 5s grace → SIGKILL → process tree killed, no orphans.
