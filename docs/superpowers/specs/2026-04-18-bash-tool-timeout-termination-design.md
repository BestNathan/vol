# BashTool Timeout Graceful Termination Design

> **Problem:** When `BashTool` times out via `tokio::time::timeout(cmd.output())`, the underlying `sh -c` child process (and its descendants) continues running as an orphan. The timeout only drops the Future — it does not kill the process.

> **Goal:** On timeout, send SIGTERM to the entire process group, wait briefly for graceful shutdown, then SIGKILL if still alive. Return partial output + termination reason to the LLM.

## Architecture

### Current Behavior

```
cmd.output() → Future wrapped in timeout()
  ├─ completes in time → return stdout/stderr
  └─ timeout fires → Future dropped → process orphaned ❌
```

### New Behavior

```
cmd.spawn() → Child handle + process group (setpgid)
  ├─ completes in time → collect output → return
  └─ timeout fires →
      ├─ SIGTERM to -pgid (entire group)
      ├─ wait 5s
      │   ├─ exited → collect output → return "timed out (SIGTERM)"
      │   └─ still running →
      │       ├─ SIGKILL to -pgid
      │       └─ collect output → return "timed out (SIGKILL)"
```

## Implementation Details

### File: `crates/vol-llm-tools-builtin/bash-tool/src/lib.rs`

#### 1. Replace `cmd.output()` with `cmd.spawn()`

The `execute()` method changes from:
```rust
let output = match timeout(timeout_duration, cmd.output()).await { ... }
```
to:
```rust
let mut child = cmd.spawn()?;
let pgid = child.id();  // process group = child PID
let output = match timeout(timeout_duration, child.wait_with_output()).await { ... }
```

#### 2. Process group setup

Use `std::process::CommandExt::process_group(0)` (available via `tokio::process::Command` which wraps `std::process::Command` on Unix):

```rust
use std::os::unix::process::CommandExt as _;
cmd.process_group(0);  // creates new process group, pgid = child pid
```

#### 3. Timeout termination sequence

```rust
const SIGTERM_GRACE_PERIOD: Duration = Duration::from_secs(5);

Err(_) => {
    // Timeout: send SIGTERM to entire process group
    let pgid = child.id() as i32;
    let _ = nix::sys::signal::kill(
        nix::unistd::Pid::from_pid(-pgid),
        nix::sys::signal::Signal::SIGTERM,
    );

    // Wait for graceful exit via polling — try_wait doesn't consume output pipes
    let grace_start = std::time::Instant::now();
    let exited = loop {
        if grace_start.elapsed() > SIGTERM_GRACE_PERIOD {
            break false;
        }
        if child.try_wait().unwrap().is_some() {
            break true;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };

    let output = child.wait_with_output().await.unwrap_or_default();

    if exited {
        return Err(ToolError::ExecutionFailed(
            format!("Command timed out after {:?}. Sent SIGTERM to process group {}, process exited.",
                timeout_duration, pgid)
        ));
    } else {
        // Still running — SIGKILL
        let _ = nix::sys::signal::kill(
            nix::unistd::Pid::from_pid(-pgid),
            nix::sys::signal::Signal::SIGKILL,
        );
        let _ = child.wait().await;
        return Err(ToolError::ExecutionFailed(
            format!("Command timed out after {:?}. Sent SIGTERM then SIGKILL to process group {}.",
                timeout_duration, pgid)
        ));
    }
}
```

#### 4. Dependencies

Add to `crates/vol-llm-tools-builtin/bash-tool/Cargo.toml`:
```toml
nix = { version = "0.29", features = ["signal", "process"] }
```

Platform note: `nix::sys::signal::kill` and `CommandExt::process_group` are Unix-only. This is acceptable — the project runs on Linux. `#[cfg(unix)]` guards should be added for any non-Unix paths.

### File: `crates/vol-llm-tools-builtin/bash-tool/tests/bash_tool_test.rs`

#### New test: `test_bash_timeout_kills_process`

```rust
#[tokio::test]
async fn test_bash_timeout_kills_process() {
    let tool = BashTool::new();
    let args = json!({
        "command": "sleep 10",
        "timeout": 100
    });

    let result = tool.execute(&args, &ToolContext::default()).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    let err_str = format!("{}", err);
    assert!(err_str.contains("timed out"));

    // Verify the sleep process was killed (not orphaned)
    tokio::time::sleep(Duration::from_millis(200)).await;
    let check = Command::new("pgrep")
        .arg("-f")
        .arg("sleep 10")
        .output()
        .await
        .unwrap();
    assert!(check.stdout.is_empty(), "sleep 10 should have been killed");
}
```

## Data Flow

```
LLM Agent → BashTool.execute()
  │
  ├─ cmd.process_group(0)          # new pgid
  ├─ child = cmd.spawn()           # get Child handle
  ├─ timeout(child.wait_with_output())
  │   │
  │   ├─ success → return stdout/stderr
  │   │
  │   └─ timeout →
  │       ├─ kill(SIGTERM, -pgid)  # signal entire group
  │       ├─ timeout(5s, child.wait())
  │       │   ├─ exited → return "timed out (SIGTERM)" + partial output
  │       │   └─ still alive →
  │       │       ├─ kill(SIGKILL, -pgid)
  │       │       └─ return "timed out (SIGKILL)" + partial output
```

## Error Messages

| Scenario | Error Message |
|----------|--------------|
| SIGTERM succeeded | `Command timed out after X. Sent SIGTERM to process group Y, process exited.` |
| SIGKILL required | `Command timed out after X. Sent SIGTERM then SIGKILL to process group Y.` |

## Backwards Compatibility

- No API changes — `BashParams` struct unchanged
- Existing tests pass unchanged
- Only behavior change: orphaned processes are now killed (intended fix)

## Risks

1. **Process group ID conflicts** — Using child PID as pgid is standard POSIX behavior. Conflict risk is negligible for short-lived agent tool commands.
