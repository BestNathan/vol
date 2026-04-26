# Tool Failure Recovery Design

## Problem

Currently, when a tool call fails in the ReAct agent loop, the entire run aborts immediately. The LLM never sees the error message and has no opportunity to retry or adjust arguments.

## Goal

Change tool failure from a terminal error to a recoverable event — return the error as a `Tool` role message so the LLM can observe it and retry on the next turn.

## Architecture

### Current Flow (Terminal)
```
Tool execution fails → emit ToolCallError → record error → emit AgentAborted → set_error → return Err → run ends
```

### New Flow (Recoverable)
```
Tool execution fails → emit ToolCallError → record error → add Message::tool(error) → continue loop
```

The LLM sees the error message on its next turn and can retry with corrected arguments.

## Changes

### `crates/vol-llm-agent/src/react/agent.rs`

**Tool execution `Err` branch (lines ~395-422):**

Replace the abort logic:
```rust
// OLD: abort run
Err(e) => {
    run_ctx.emit(AgentStreamEvent::tool_call_error(...)).await;
    run_ctx.record_tool_call(ToolCallRecord { ... success: false }).await;
    run_ctx.emit(AgentStreamEvent::agent_aborted(reason.clone())).await;
    run_ctx.set_error(reason).await;
    return Err(AgentError::ToolExecution { ... });
}
```

With continue-on-error:
```rust
// NEW: add error message and continue
Err(e) => {
    let duration_ms = tool_begin.elapsed().as_millis() as u64;
    run_ctx.emit(AgentStreamEvent::tool_call_error(
        call.id.clone(), call.name.clone(), e.to_string(),
        Some(duration_ms),
    )).await;

    let error_content = format!("Tool '{}' error: {}", call.name, e);
    run_ctx.add_message(Message::tool(error_content, call.id.clone())).await;

    continue;
}
```

Error message format: `Tool '{name}' error: {error}` — no arguments included (they already appear in the preceding `ToolCallBegin` message).

### No other files change

| Component | Reason |
|-----------|--------|
| `SessionRecorderPlugin` | Already records `ToolCallError` as `Message::tool` — no change needed |
| `AgentConfig` | No new retry limit field — trust the model to self-regulate via `max_iterations` |
| `HitlPlugin` | Unaffected — intercepts before tool execution, not after failure |
| `AgentStreamEvent::ToolCallError` | Already exists with correct fields — no change needed |

## Error Handling

The agent no longer returns `Err(AgentError::ToolExecution)` for tool failures. Instead:

- Tool failure is emitted as `ToolCallError` event (for observers/TUI)
- Tool failure is recorded in `tool_call_records` with `success: false`
- Tool failure is added to session as `Message::tool` (for LLM visibility)
- Loop continues — next LLM call will see the error

If the LLM keeps producing the same failing tool call, `max_iterations` will eventually stop the run.

## Testing

- Update existing tests that expect `Err(AgentError::ToolExecution)` — they should now expect `Ok` with an error message in the final response
- Add a test: tool fails → error message in session → loop continues → LLM gets another turn
