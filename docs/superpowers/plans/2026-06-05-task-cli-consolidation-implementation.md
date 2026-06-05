# Task CLI Consolidation — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `--block`/`--timeout` to `task output` CLI, then remove 7 individual `task_xxx` tools from AgentRuntime, keeping only the unified `task` CLI tool.

**Architecture:** Extend the CLI parser/commands/executor for output blocking; remove `register_all()` call from `AgentRuntime` while keeping `register_cli()` and `TaskHandler`.

**Tech Stack:** Rust, clap, vol-llm-task, vol-llm-runtime

---

### Task 1: Add --block and --timeout to task output CLI

**Files:**
- Modify: `crates/vol-llm-task/src/cli/commands.rs`
- Modify: `crates/vol-llm-task/src/cli/parser.rs`
- Modify: `crates/vol-llm-task/src/cli/executor.rs`

- [ ] **Step 1: Add block/timeout fields to ParsedCommand::Output**

In `crates/vol-llm-task/src/cli/commands.rs`, change the `Output` variant:

```rust
// Before:
Output {
    id: u64,
    json: bool,
},

// After:
Output {
    id: u64,
    block: bool,
    timeout_ms: u64,
    json: bool,
},
```

- [ ] **Step 2: Add --block and --timeout clap args to parser**

In `crates/vol-llm-task/src/cli/parser.rs`, in the `output` subcommand definition (around line 151-160), add two new args:

```rust
.subcommand(
    Command::new("output")
        .about("Read task output")
        .arg(
            Arg::new("id")
                .long("id")
                .required(true)
                .value_parser(value_parser!(u64))
                .help("Task ID"),
        )
        .arg(
            Arg::new("block")
                .long("block")
                .action(ArgAction::SetTrue)
                .help("Wait for task to complete before returning output"),
        )
        .arg(
            Arg::new("timeout")
                .long("timeout")
                .value_parser(value_parser!(u64))
                .default_value("30000")
                .help("Max wait time in milliseconds (default 30000)"),
        ),
)
```

- [ ] **Step 3: Update parser match arm for output**

In `parser.rs`, update the `Some(("output", m))` arm (around line 278):

```rust
// Before:
Some(("output", m)) => Ok(ParsedCommand::Output {
    id: *m.get_one::<u64>("id").unwrap_or(&0),
    json,
}),

// After:
Some(("output", m)) => Ok(ParsedCommand::Output {
    id: *m.get_one::<u64>("id").unwrap_or(&0),
    block: m.get_flag("block"),
    timeout_ms: *m.get_one::<u64>("timeout").unwrap_or(&30000),
    json,
}),
```

- [ ] **Step 4: Add blocking poll logic to executor**

In `crates/vol-llm-task/src/cli/executor.rs`, replace the `ParsedCommand::Output` arm (lines 237-277) with:

```rust
ParsedCommand::Output {
    id,
    block,
    timeout_ms,
    json: _,
} => {
    let task_id = TaskId(id);

    if block {
        let start = std::time::Instant::now();
        loop {
            let task = store
                .get(&task_id)
                .await
                .map_err(|e| format!("Failed to get task: {}", e))?
                .ok_or_else(|| format!("Task {} not found", task_id))?;

            match task.status {
                TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Killed => break,
                _ => {
                    if start.elapsed().as_millis() as u64 >= timeout_ms {
                        return Ok(ToolResult {
                            success: false,
                            content: format!(
                                "Timeout waiting for task {} ({}ms)",
                                task_id, timeout_ms
                            ),
                            error: Some("timeout".to_string()),
                            data: Some(serde_json::json!({
                                "taskId": id.to_string(),
                                "status": task.status.to_string()
                            })),
                            call_id: String::new(),
                        });
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                }
            }
        }
    }

    // Read output (same as before)
    let task = store
        .get(&task_id)
        .await
        .map_err(|e| format!("Failed to get task: {}", e))?
        .ok_or_else(|| format!("Task {} not found", task_id))?;

    match (task.output_file.as_ref(), task.result.as_ref()) {
        (Some(path), _) => match tokio::fs::read_to_string(path).await {
            Ok(output) => Ok(ToolResult {
                success: true,
                content: output,
                error: None,
                data: Some(serde_json::json!({"taskId": id.to_string()})),
                call_id: String::new(),
            }),
            Err(e) => Ok(ToolResult {
                success: false,
                content: format!("Failed to read output file: {}", e),
                error: Some(format!("IO error: {}", e)),
                data: Some(serde_json::json!({"taskId": id.to_string()})),
                call_id: String::new(),
            }),
        },
        (_, Some(result)) => Ok(ToolResult {
            success: true,
            content: result.output_truncated.clone(),
            error: None,
            data: Some(serde_json::json!({"taskId": id.to_string()})),
            call_id: String::new(),
        }),
        _ => Ok(ToolResult {
            success: false,
            content: format!("No output for task {}", task_id),
            error: Some("No output available".to_string()),
            data: Some(serde_json::json!({"taskId": id.to_string()})),
            call_id: String::new(),
        }),
    }
}
```

- [ ] **Step 5: Add parser test for --block and --timeout flags**

In `parser.rs` tests module, add two new tests after `test_parse_output`:

```rust
#[test]
fn test_parse_output_with_block() {
    let cmd = parse("output --id 7 --block").unwrap();
    match cmd {
        ParsedCommand::Output { id, block, .. } => {
            assert_eq!(id, 7);
            assert!(block);
            // default when not specified
        }
        _ => panic!("expected Output"),
    }
}

#[test]
fn test_parse_output_with_block_and_timeout() {
    let cmd = parse("output --id 7 --block --timeout 60000").unwrap();
    match cmd {
        ParsedCommand::Output { id, block, timeout_ms, .. } => {
            assert_eq!(id, 7);
            assert!(block);
            assert_eq!(timeout_ms, 60000);
        }
        _ => panic!("expected Output"),
    }
}
```

- [ ] **Step 6: Run tests**

```bash
cargo test -p vol-llm-task -- cli
```

Expected: all parser + executor tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-task/src/cli/
git commit -m "feat(task-cli): add --block and --timeout to task output command

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```

---

### Task 2: Remove individual task_xxx tools from AgentRuntime

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs`

- [ ] **Step 1: Remove register_all() from AgentRuntimeBuilder::build()**

In `crates/vol-llm-runtime/src/lib.rs`, around lines 271-275, remove the `register_all()` line:

```rust
// Before:
vol_llm_task::tools::register_all(&mut tool_registry, task_store.clone());
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());

// After:
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
```

- [ ] **Step 2: Remove register_all() from AgentRuntime::for_test()**

Same change in the `for_test()` constructor (around lines 193-199):

```rust
// Before:
vol_llm_task::tools::register_all(&mut tool_registry, task_store.clone());
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());

// After:
vol_llm_task::tools::register_cli(&mut tool_registry, task_store.clone());
```

- [ ] **Step 3: Check compilation**

```bash
cargo check -p vol-llm-runtime 2>&1
```

Expected: zero errors.

- [ ] **Step 4: Run affected tests**

```bash
cargo test -p vol-llm-runtime 2>&1
cargo test -p vol-llm-task 2>&1
```

Expected: all pass. The `register_all` function and individual tool files still exist (not deleted), they're just not called from AgentRuntime anymore.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-runtime/src/lib.rs
git commit -m "refactor(runtime): remove individual task_xxx tools, keep only task CLI

AgentRuntime now registers only the unified 'task' CLI tool via register_cli().
The 7 individual tools (task_create, task_get, etc.) are no longer registered.
TaskHandler (task.list/task.get protocol) is unchanged.

Co-Authored-By: Claude Opus 4.8 <noreply@anthropic.com>"
```
