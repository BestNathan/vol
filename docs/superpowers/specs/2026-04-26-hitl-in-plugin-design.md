# Move HITL Logic into Plugin System

## Context

HITL (Human-in-the-Loop) and `unsafe_mode` logic is currently scattered across `agent.rs`, `run_context.rs`, and `hitl.rs`, creating tight coupling between the run loop and approval flows. The run loop directly manages an approval channel, spawns CLI approval handlers, and calls `request_tool_approval()` / `request_continue_approval()`.

This design moves all HITL concerns into the plugin system where they belong: `HitlPlugin` handles approval decisions via `intercept()`, and the run loop only emits events and respects `PluginDecision`.

## Current State

```
agent.rs run() ──┬── Phase 1.5: unsafe_mode branch (auto-approve / handler / CLI / deny)
                 ├── request_tool_approval() per tool call (lines 439)
                 └── request_continue_approval() on max iterations (lines 324)

run_context.rs ──┬── approval_tx mpsc channel
                 ├── ApprovalRequest / ApprovalResponse types
                 ├── request_tool_approval()
                 └── request_continue_approval()

hitl.rs ─────────┬── ApprovalChannel trait, ApprovalHandler trait
                 ├── spawn_custom_approval_handler()
                 └── run_cli_approval_loop()

PluginContext ─── separate struct (subset of RunContext fields)
```

## After

```
agent.rs run() ─── emit + intercept() → PluginDecision (zero HITL knowledge)

HitlPlugin ─────── intercept(ToolCallBegin) → check ctx.tools.sensitivity() → wait stdin → Continue/Skip/Abort
                   intercept(IterationComplete) → iteration > max? → wait stdin → Continue/Abort

RunContext ─────── No approval_tx, no ApprovalRequest, no approval methods.
                   New() returns only (RunContext, plugin_rx) — no approval_rx.

PluginContext ─── Deleted. All plugins use &RunContext directly.
```

## Changes

### 1. `crates/vol-llm-agent/src/react/plugin.rs`

- Delete `PluginContext` struct
- Replace all `&PluginContext` in `AgentPlugin` trait with `&RunContext`
- Add `RunContext` to this file's imports

### 2. `crates/vol-llm-agent/src/react/run_context.rs`

- Delete `ApprovalRequest` / `ApprovalResponse` types
- Delete `approval_tx` field from `RunContext`
- Delete `CONTINUE_SENTINEL` constant
- Delete `request_tool_approval()` / `request_continue_approval()` methods
- Update `new()` to return `(Self, mpsc::Receiver<PluginRequest>)` (remove approval_rx)
- Update `Clone` impl accordingly

### 3. `crates/vol-llm-agent/src/react/agent.rs`

- Delete `AgentConfig::unsafe_mode` and `AgentConfig::approval_handler` fields
- Delete Phase 1.5 approval handler spawning (lines 205-226)
- Delete max-iterations `request_continue_approval()` block — replace with:
  ```rust
  if iteration > config.max_iterations {
      run_ctx.emit(AgentStreamEvent::max_iterations_reached(iteration, config.max_iterations)).await;
      match run_ctx.intercept(&AgentStreamEvent::iteration_complete(iteration, vec![], None)).await {
          Ok(PluginDecision::Continue) => {
              run_ctx.reset_iteration();
              continue;
          }
          _ => {
              let reason = format!("Max iterations ({}) reached", config.max_iterations);
              run_ctx.emit(AgentStreamEvent::agent_aborted(reason.clone())).await;
              return Err(crate::AgentError::MaxIterationsReached { max: config.max_iterations });
          }
      }
  }
  ```
- Delete `request_tool_approval()` call in tool execution block — the `ToolSensitivity::RequiresApproval` check and approval flow are now HitlPlugin's responsibility in `intercept()`
- Update `RunContext::new()` callsites to handle 2-tuple return

### 4. `crates/vol-llm-agent/src/react/hitl.rs`

Rewrite `HitlPlugin` to handle approval logic inline in `intercept()`:

- `HitlPlugin` gets `channel: Arc<dyn ApprovalChannel>` (approval channel interface stays)
- `intercept(ToolCallBegin)`: use `ctx.tools.tool_sensitivity()` to check if tool needs approval, if so call `channel.request_approval()` and return Continue/Skip/Abort
- `intercept(IterationComplete)`: check if iteration exceeded max, ask for continuation, return Continue/Abort
- Delete old ApprovalChannel trait and ApprovalHandler trait — these move to a minimal form used only internally by HitlPlugin for stdin waiting
- Move `run_cli_approval_loop()` logic into HitlPlugin as a private helper

### 5. `crates/vol-llm-agent/src/react/plugin_stream.rs`

- Update `spawn_listener_task()` and `run_interceptor_loop()` to use `&RunContext` instead of `&PluginContext`
- Update call sites in `agent.rs` to pass `&RunContext` (no more `plugin_context_from_run_ctx()`)

### 6. Callers

- `crates/vol-llm-agents/src/coding/agent.rs` — remove `unsafe_mode` from `AgentConfig`
- `crates/vol-llm-tui/src/` — remove `approval_handler` from `AgentConfig`. TUI HITL interaction is replaced by implementing a TuiApprovalChannel that waits for user input from the TUI's approval UI and returns the result synchronously.
- `crates/vol-llm-agent/src/plugins/hitl_cli.rs` / `hitl_http.rs` — update to new HitlPlugin interface. These files provide ApprovalChannel implementations (blocking stdin wait / HTTP wait) used by HitlPlugin.
- All test files — update for new return types and removed types

## Verification

```bash
cargo check --workspace
cargo test --workspace -- --test-threads=1
```
