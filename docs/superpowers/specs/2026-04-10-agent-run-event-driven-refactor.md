# Agent Run Event-Driven Refactor

**Date:** 2026-04-10  
**Author:** Claude Code  
**Status:** Approved - Ready for Implementation

---

## Overview

Refactor `ReActAgent::run()` to be purely event-driven:
- **No return value** - `run()` returns `Result<(), AgentError>`
- **No stream receiver** - Remove `AgentStreamReceiver` from return type
- **All observation through plugins** - Events emitted via `run_ctx.emit()`, observed by plugins (e.g., ObservabilityPlugin)
- **Event integrity** - All events emitted unconditionally, no filtering or modification

---

## Design Principles

1. **Single source of truth** - `RunContext` event bus is the only event delivery mechanism
2. **Complete event log** - Every event is emitted, including edge cases (empty thinking, etc.)
3. **Plugin observability** - Plugins observe and record events; `run()` itself returns nothing
4. **No duplication** - Remove `tx` channel sending logic; only `run_ctx.emit()` is used

---

## Architecture

### Before

```
┌─────────────────────────────────────────────────────────────┐
│ ReActAgent::run()                                            │
│                                                              │
│  ┌───────────────┐      ┌─────────────────────────────┐    │
│  │ spawned task  │      │ run() returns               │    │
│  │               │      │ AgentStreamReceiver         │    │
│  │ emit() ───────┼──────┤                             │    │
│  │ tx.send() ────┼──────┤ External code consumes      │    │
│  │               │      │ events via receiver         │    │
│  └───────────────┘      └─────────────────────────────┘    │
│                                                              │
│  Two delivery paths: event bus + channel                     │
└─────────────────────────────────────────────────────────────┘
```

### After

```
┌─────────────────────────────────────────────────────────────┐
│ ReActAgent::run()                                            │
│                                                              │
│  ┌───────────────┐                                           │
│  │ spawned task  │                                           │
│  │               │                                           │
│  │ emit() ───────┼──► RunContext event bus                   │
│  │               │       │                                    │
│  │ return Ok(()) │       ▼                                    │
│  │               │    Plugins observe & record               │
│  └───────────────┘    (ObservabilityPlugin, etc.)            │
│                                                              │
│  Single delivery path: event bus only                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Changes by File

### 1. `crates/vol-llm-agent/src/react/agent.rs`

**`run()` signature:**
```rust
// Before
pub async fn run(
    &self,
    user_input: &str,
    context: ToolContext,
) -> Result<AgentStreamReceiver, AgentError>

// After
pub async fn run(
    &self,
    user_input: &str,
    context: ToolContext,
) -> Result<(), AgentError>
```

**Remove `tx` channel:**
```rust
// Remove these lines:
// let (tx, rx) = mpsc::channel(100);
// let _ = tx.send(Err(...)).await;  // all error sends
// let _ = tx.send(Ok(event)).await; // all event sends
```

**Keep `run_ctx.emit()`:**
```rust
// All events go through event bus only
run_ctx.emit(event).await;
```

**Emit all events unconditionally:**
```rust
// Before: only emit if thinking has content
if !thinking.is_empty() {
    run_ctx.emit(AgentStreamEvent::ThinkingComplete { thinking });
}

// After: always emit
run_ctx.emit(AgentStreamEvent::ThinkingComplete { thinking });
```

**Return at end:**
```rust
// Before: Ok(AgentStreamReceiver::new(rx))
// After: Ok(())
```

---

### 2. `crates/vol-llm-agent/src/react/stream.rs`

**`AgentStreamReceiver` usage:**
- Keep the type for backward compatibility (may be used elsewhere)
- Remove from `run()` return type
- `AgentStreamEvent` remains unchanged

---

### 3. `crates/vol-llm-agent/src/observability/plugin.rs`

**Log format change (方案 A - structured fields):**

```rust
// Before: may have filtered/modified event data
fn create_log_entry(&self, event: &AgentStreamEvent, ctx: &RunContext) -> LogEntry {
    let (event_name, data) = match event {
        // ... custom logic, filtering, etc.
    };
    // ...
}

// After: directly serialize full event
fn create_log_entry(&self, event: &AgentStreamEvent, ctx: &RunContext) -> LogEntry {
    let data = serde_json::to_value(event)
        .unwrap_or_else(|e| json!({"error": format!("Serialization failed: {}", e)}));
    
    LogEntry {
        timestamp: Utc::now(),
        run_id: ctx.run_id.clone(),
        agent_id: self.logger.agent_id.clone(),
        event_type: std::mem::discriminant(event).to_string(), // or extract variant name
        data,
    }
}
```

**Alternative: Match each variant and extract fields cleanly:**
```rust
fn event_to_data(event: &AgentStreamEvent) -> serde_json::Value {
    match event {
        AgentStreamEvent::AgentStart { input } => json!({ "input": input }),
        AgentStreamEvent::ThinkingComplete { thinking } => json!({ "thinking": thinking }),
        AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
            json!({ "tool_name": tool_name, "arguments": arguments })
        }
        AgentStreamEvent::ToolCallComplete { tool_name, result } => {
            json!({ "tool_name": tool_name, "result": result })
        }
        AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer } => {
            json!({
                "iteration": iteration,
                "tool_calls": tool_calls,
                "final_answer": final_answer,
            })
        }
        AgentStreamEvent::AgentComplete { response } => json!({ "response": response }),
        AgentStreamEvent::AgentAborted { reason } => json!({ "reason": reason }),
        AgentStreamEvent::PluginEvent { name, data } => json!({ "name": name, "data": data }),
    }
}
```

**Output JSONL format:**
```json
{"timestamp":"2026-04-10T12:00:00Z","run_id":"run_abc","agent_id":"test","event_type":"ThinkingComplete","data":{"thinking":"..."}}
```

---

### 4. `crates/vol-llm-agent/examples/agent_observability_test.rs`

**Update example to not consume stream:**

```rust
// Before
let stream_result = agent.run(query, context).await;
match stream_result {
    Ok(mut stream) => {
        while let Some(event) = stream.recv().await {
            // handle events
        }
    }
    Err(e) => eprintln!("Error: {}", e),
}

// After
let result = agent.run(query, context).await;
match result {
    Ok(()) => println!("Agent completed successfully"),
    Err(e) => eprintln!("Agent error: {}", e),
}
// Events are logged by ObservabilityPlugin to files
```

---

### 5. `crates/vol-llm-agent/examples/agent_cli_approval.rs`

**HITL example needs update:**
- Currently consumes stream for display
- Can keep display logic but use plugin events instead
- Or use a "DisplayPlugin" that prints to stdout

**Recommended:** Keep stdout printing in a simple plugin or callback mechanism.

---

### 6. Tests

**Update tests that consume stream:**

```rust
// Before
let mut stream = agent.run("test", ctx).await.unwrap();
while let Some(event) = stream.recv().await {
    // assert on events
}

// After: use a test plugin to capture events, or check log files
```

**New test pattern:**
```rust
#[tokio::test]
async fn test_agent_emits_events() {
    let (tx, mut rx) = mpsc::channel(100);
    
    // Create a test plugin that captures events
    let capture_plugin = CapturePlugin::new(tx);
    
    let agent = ReActAgent::builder()
        .with_llm(mock_llm)
        .with_plugin(capture_plugin)
        .build().unwrap();
    
    agent.run("test", ctx).await.unwrap();
    
    // Verify events were captured
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    assert!(!events.is_empty());
}
```

---

## Event Completeness Guarantee

**All events must be emitted:**

| Event | Condition | Before | After |
|-------|-----------|--------|-------|
| `AgentStart` | Always | ✓ | ✓ |
| `ThinkingComplete` | When thinking not empty | ✓ (conditional) | ✓ (always) |
| `ToolCallBegin` | Per tool call | ✓ | ✓ |
| `ToolCallComplete` | Per tool call | ✓ | ✓ |
| `IterationComplete` | Per iteration | ✓ | ✓ |
| `AgentComplete` | When final answer | ✓ | ✓ |
| `AgentAborted` | On error/abort | ✓ | ✓ |
| `PluginEvent` | From plugins | ✓ | ✓ |

**Key change:** `ThinkingComplete` emitted even when `thinking.is_empty()`.

---

## Migration Path

### Phase 1: Core Changes
1. Update `run()` signature
2. Remove `tx` channel and `rx` return
3. Remove all `tx.send()` calls
4. Ensure all `run_ctx.emit()` calls remain

### Phase 2: Observability Plugin
1. Update `create_log_entry()` to serialize full event
2. Update log format to structured fields
3. Verify JSONL output is complete

### Phase 3: Examples
1. Update `agent_observability_test.rs`
2. Update `agent_cli_approval.rs`
3. Update any other examples

### Phase 4: Tests
1. Update existing tests
2. Add test for event completeness
3. Verify all tests pass

---

## Testing Strategy

### Unit Tests
- `test_agent_run_returns_ok`
- `test_thinking_complete_emitted_even_when_empty`
- `test_all_events_emitted_in_order`

### Integration Tests
- Run agent with observability plugin
- Verify log file contains all events
- Verify event order matches execution

### Manual Testing
- Run `agent_observability_test.rs`
- Check log files for completeness
- Verify JSONL format

---

## Rollback Plan

If issues found:
1. Revert `run()` signature change
2. Restore `tx` channel sends
3. Keep observability plugin changes (can work with either approach)

---

## Related Files

- `crates/vol-llm-agent/src/react/agent.rs` - Core agent logic
- `crates/vol-llm-agent/src/react/run_context.rs` - Event bus
- `crates/vol-llm-agent/src/observability/plugin.rs` - Logging
- `crates/vol-llm-agent/src/react/stream.rs` - Event types
- `crates/vol-llm-agent/examples/` - Examples to update

---

## Success Criteria

1. ✓ `run()` returns `Result<(), AgentError>`
2. ✓ No `tx` channel usage in agent loop
3. ✓ All events emitted via `run_ctx.emit()`
4. ✓ `ThinkingComplete` emitted unconditionally
5. ✓ Observability plugin logs full event data
6. ✓ All examples updated and working
7. ✓ All tests pass

---

## Future Considerations

**Not in scope for this refactor:**
- Removing `AgentStreamReceiver` type entirely (may have external users)
- Changing plugin API
- Restructuring event types

**Future enhancements:**
- Consider adding event IDs for tracing
- Consider adding parent span IDs for correlation
- Consider adding event timestamps at emit time
