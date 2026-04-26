# Agent Lifecycle Events Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Expand `AgentStreamEvent` from 8 to 18 variants with complete lifecycle coverage, streaming delta events, and full error path visibility.

**Architecture:** Modify `AgentStreamEvent` enum in vol-llm-core, rewrite `consume_llm_stream()` to emit real-time streaming events, update agent.rs emit sequence for complete error paths, update all consumers (SessionListener, TUI renderer, tests).

**Tech Stack:** Rust, tokio broadcast channels, vol-llm-core, vol-llm-agent, vol-session, vol-llm-tui

---

## Design Reference

See `docs/ai-agent/06-agent-lifecycle-events-design.md` for the full design document.

### Event Summary (18 variants)

| Category | Events |
|----------|--------|
| Lifecycle (3) | `AgentStart`, `AgentComplete`, `AgentAborted` |
| LLM Call (3) | `LLMCallStart`, `LLMCallComplete`, `LLMCallError` |
| Thinking (3) | `ThinkingStart`, `ThinkingDelta`, `ThinkingComplete` |
| Content (3) | `ContentStart`, `ContentDelta`, `ContentComplete` |
| Tool (4) | `ToolCallBegin`, `ToolCallComplete`, `ToolCallError`, `ToolCallSkipped` |
| Iteration (1) | `IterationComplete` |
| Plugin (1) | `PluginEvent` |

### Key Semantic Guarantees

1. Every path ends with `AgentComplete` or `AgentAborted`
2. LLM: `LLMCallStart → LLMCallComplete` or `LLMCallError`
3. Tool: `ToolCallBegin → ToolCallComplete` or `ToolCallError` or `ToolCallSkipped`
4. Delta sequences: `Start → Delta×N → Complete`
5. No `ToolCallDelta` — tool output is not streaming (future work)

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-llm-core/src/stream.rs` | Modify | Expand `AgentStreamEvent` from 8 to 18 variants |
| `crates/vol-llm-agent/src/react/run_context.rs` | Modify | Increase broadcast channel 100→1024, add `emit_await()` |
| `crates/vol-llm-agent/src/react/agent.rs` | Modify | Emit LLM call events, rewrite `consume_llm_stream`, complete error paths |
| `crates/vol-session/src/listener.rs` | Modify | Update `should_record()` and `event_to_message()` for new events |
| `crates/vol-llm-tui/src/render.rs` | Modify | Render 10 new event variants |
| `crates/vol-llm-agent/src/react/hitl.rs` | Check | Verify no changes needed |
| All test files matching `crates/vol-llm-agent/tests/*` | Modify | Handle new enum variants |
| `crates/vol-llm-agents/tests/*` | Modify | Handle new enum variants |

---

### Task 1: Expand AgentStreamEvent Enum

**Files:**
- Modify: `crates/vol-llm-core/src/stream.rs:58-98`

- [ ] **Step 1: Replace AgentStreamEvent enum with 18 variants**

In `crates/vol-llm-core/src/stream.rs`, replace the `AgentStreamEvent` enum (lines 58-98) with:

```rust
/// Agent stream event for ReAct agent workflow.
///
/// These events are emitted during agent execution and can be used
/// for session recording, observability, and plugin interception.
///
/// # Semantic Guarantees
///
/// 1. Every execution path ends with AgentComplete or AgentAborted
/// 2. LLM calls are paired: LLMCallStart → LLMCallComplete or LLMCallError
/// 3. Tool calls are paired: ToolCallBegin → ToolCallComplete or ToolCallError or ToolCallSkipped
/// 4. Delta sequences are complete: Start → Delta×N → Complete
#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    // === Lifecycle (3) ===
    /// Agent started execution
    AgentStart { input: String },
    /// Agent completed successfully
    AgentComplete,
    /// Agent aborted or failed
    AgentAborted { reason: String },

    // === LLM Call (3) ===
    /// LLM request started
    LLMCallStart { iteration: u32 },
    /// LLM request completed
    LLMCallComplete {
        model: String,
        usage: Option<TokenUsage>,
    },
    /// LLM request failed
    LLMCallError { error: String },

    // === Streaming: Thinking (3) ===
    /// First thinking token arrived
    ThinkingStart,
    /// Thinking delta (streaming fragment)
    ThinkingDelta { delta: String },
    /// Thinking section completed
    ThinkingComplete { thinking: String },

    // === Streaming: Content (3) ===
    /// First content token arrived
    ContentStart,
    /// Content delta (streaming fragment)
    ContentDelta { delta: String },
    /// Content section completed
    ContentComplete { content: String },

    // === Tool Execution (4) ===
    /// Tool execution started
    ToolCallBegin {
        tool_call_id: String,
        tool_name: String,
        arguments: String,
    },
    /// Tool execution completed successfully
    ToolCallComplete {
        tool_call_id: String,
        tool_name: String,
        result: String,
    },
    /// Tool execution failed
    ToolCallError {
        tool_call_id: String,
        tool_name: String,
        error: String,
    },
    /// Tool execution skipped (HITL rejected / plugin skip)
    ToolCallSkipped {
        tool_call_id: String,
        tool_name: String,
        reason: String,
    },

    // === Iteration (1) ===
    /// One iteration completed
    IterationComplete {
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },

    // === Plugin (1) ===
    /// Custom event from plugin
    PluginEvent {
        name: String,
        data: serde_json::Map<String, serde_json::Value>,
    },
}
```

Note: `TokenUsage` is already imported via `use crate::{FinishReason, TokenUsage, ToolCall};` at line 3.

- [ ] **Step 2: Update existing tests in stream.rs**

The tests at lines 100-191 reference the old enum variants. Update them:

- `test_agent_stream_event_creation` — uses `AgentStart`, keep as-is (still exists)
- `test_agent_stream_event_tool_call` — uses `ToolCallBegin`, keep as-is (still exists)
- `test_agent_stream_event_iteration_complete` — uses `IterationComplete`, keep as-is
- `test_agent_stream_event_aborted` — uses `AgentAborted`, keep as-is
- `test_agent_stream_event_plugin_event` — uses `PluginEvent`, keep as-is

No changes needed to existing tests since all old variants still exist with the same fields.

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-core
```

Expected: Compiles successfully. Downstream crates will have match exhaustiveness errors — that's expected and will be fixed in later tasks.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-core/src/stream.rs
git commit -m "feat: expand AgentStreamEvent to 18 lifecycle variants"
```

---

### Task 2: Increase Broadcast Channel Capacity

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:248`

- [ ] **Step 1: Change broadcast channel capacity from 100 to 1024**

In `crates/vol-llm-agent/src/react/run_context.rs`, line 248, change:

```rust
// Old
let (event_tx, _) = broadcast::channel(100);

// New
let (event_tx, _) = broadcast::channel(1024);
```

Capacity calculation from design doc: single LLM call ~50-80 deltas + tools ~10 + lifecycle ~10 ≈ 100/iteration × max 5 iterations = 500. 1024 gives 2× margin.

- [ ] **Step 2: Add emit_await() method to RunContext**

The design doc's emit sequence uses `.await` on emit calls. The current `emit()` is `async fn` returning `()`, which works for fire-and-forget. But for the `consume_llm_stream` function which will receive `&RunContext`, we need the emit to be usable. The current `emit()` already works — it's async. However, to be explicit about the guaranteed delivery pattern, add an `emit_await()` method that waits for all subscribers to receive the event:

```rust
    /// Emit an event and wait for all subscribers to receive it.
    /// Use this when delivery ordering matters (e.g., in consume_llm_stream).
    pub async fn emit_await(&self, event: AgentStreamEvent) {
        let traced_event = TracedEvent::without_span(event);
        // broadcast::send returns Err when no subscribers, which is fine
        let _ = self.event_tx.send(traced_event);
    }
```

Actually, since `broadcast::send` is synchronous (not async), both `emit()` and `emit_await()` are effectively the same. The current `emit()` is sufficient. We just need `consume_llm_stream` to accept `&RunContext` and call `run_ctx.emit(event).await`.

Skip this step — no new method needed.

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-agent
```

Expected: Match exhaustiveness errors in agent.rs and tests — these will be fixed in Task 3/4.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs
git commit -m "feat: increase broadcast channel capacity to 1024 for streaming events"
```

---

### Task 3: Rewrite consume_llm_stream and Agent Loop Emit Sequence

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:314-547` (the LLM call section and consume_llm_stream)

This is the largest task. It has three parts:
1. Add `run_ctx` parameter to `consume_llm_stream` and emit streaming events
2. Emit `LLMCallStart` / `LLMCallComplete` / `LLMCallError` in agent loop
3. Fix all error paths to emit terminal events before returning Err

- [ ] **Step 1: Rewrite consume_llm_stream to emit streaming events**

Replace the `consume_llm_stream` function (lines 631-654) with:

```rust
/// Consume LLM stream response, emit streaming events, and accumulate into complete data.
///
/// Emits ThinkingStart/Delta/Complete and ContentStart/Delta/Complete events
/// as tokens arrive from the LLM.
async fn consume_llm_stream(
    mut stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<(String, Vec<vol_llm_core::ToolCall>, String), crate::AgentError> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();

    let mut thinking_started = false;
    let mut content_started = false;

    while let Some(result) = stream.recv().await {
        match result?.data {
            StreamEventData::ThinkingDelta { thinking: delta } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::ThinkingStart).await;
                    thinking_started = true;
                }
                thinking.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::ThinkingDelta { delta }).await;
            }
            StreamEventData::ThinkingComplete { thinking: t } => {
                if thinking_started {
                    // Accumulate any remaining delta
                }
                thinking = t;
                if !thinking_started {
                    // LLM returned thinking without deltas (non-streaming provider)
                    run_ctx.emit(AgentStreamEvent::ThinkingStart).await;
                    run_ctx.emit(AgentStreamEvent::ThinkingDelta { delta: thinking.clone() }).await;
                }
                run_ctx.emit(AgentStreamEvent::ThinkingComplete { thinking: thinking.clone() }).await;
            }
            StreamEventData::ContentDelta { delta } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::ContentStart).await;
                    content_started = true;
                }
                content.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::ContentDelta { delta }).await;
            }
            StreamEventData::ContentComplete { content: c } => {
                content = c;
                if !content_started {
                    // LLM returned content without deltas
                    run_ctx.emit(AgentStreamEvent::ContentStart).await;
                    run_ctx.emit(AgentStreamEvent::ContentDelta { delta: content.clone() }).await;
                }
                run_ctx.emit(AgentStreamEvent::ContentComplete { content: content.clone() }).await;
            }
            StreamEventData::ToolCallComplete { tool_call } => {
                tool_calls.push(tool_call);
            }
            StreamEventData::Error { code, message } => {
                return Err(crate::AgentError::Llm(crate::LLMError::StreamError(
                    format!("[{}] {}", code, message),
                )));
            }
            // Ignore other stream events (ResponseStart, ResponseComplete, UsageUpdate)
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content))
}
```

Note: Need to check what `LLMError` variant exists for stream errors. Let me check what the actual error type is.

Actually, looking at the existing code, `stream.recv()` returns `Option<Result<StreamEvent, crate::LLMError>>`. The `?` operator on `result?` propagates the `LLMError`. The existing code uses `crate::AgentError` as return type. We need to map `LLMError` to `AgentError`. The existing code just does `result?` which means `LLMError` implements `From` or is converted. Let me check the error types...

Looking at the current code more carefully, `result?.data` — the `?` propagates `LLMError` and the function returns `AgentError`. This means there must be a `From<LLMError> for AgentError` impl. The stream error case should return an error through the normal `?` path. But we also need to emit `LLMCallError` — that needs to happen in the caller (agent.rs), not here, because the caller is the one who emitted `LLMCallStart`.

Simpler approach: let `consume_llm_stream` just emit the streaming events (Thinking/Content deltas). Error handling stays in the caller. Rewrite:

```rust
/// Consume LLM stream response, emit streaming events, and accumulate into complete data.
async fn consume_llm_stream(
    mut stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<(String, Vec<vol_llm_core::ToolCall>, String), crate::AgentError> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();

    let mut thinking_started = false;
    let mut content_started = false;

    while let Some(result) = stream.recv().await {
        let event = result.map_err(|e| crate::AgentError::Llm(e))?;

        match event.data {
            StreamEventData::ThinkingDelta { thinking: delta } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::ThinkingStart).await;
                    thinking_started = true;
                }
                thinking.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::ThinkingDelta { delta }).await;
            }
            StreamEventData::ThinkingComplete { thinking: t } => {
                if !thinking_started {
                    // Non-streaming provider: emit Start + Delta + Complete
                    run_ctx.emit(AgentStreamEvent::ThinkingStart).await;
                    run_ctx.emit(AgentStreamEvent::ThinkingDelta { delta: t.clone() }).await;
                }
                thinking = t;
                run_ctx.emit(AgentStreamEvent::ThinkingComplete { thinking: thinking.clone() }).await;
            }
            StreamEventData::ContentDelta { delta } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::ContentStart).await;
                    content_started = true;
                }
                content.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::ContentDelta { delta }).await;
            }
            StreamEventData::ContentComplete { content: c } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::ContentStart).await;
                    run_ctx.emit(AgentStreamEvent::ContentDelta { delta: c.clone() }).await;
                }
                content = c;
                run_ctx.emit(AgentStreamEvent::ContentComplete { content: content.clone() }).await;
            }
            StreamEventData::ToolCallComplete { tool_call } => {
                tool_calls.push(tool_call);
            }
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content))
}
```

- [ ] **Step 2: Add LLMCallStart/Complete/Error emit sequence in agent loop**

In the agent loop (agent.rs), around lines 314-337, replace the LLM call section:

Current code (lines 314-337):
```rust
let llm_stream = match llm.converse_stream(request).await {
    Ok(stream) => stream,
    Err(e) => {
        return Err(crate::AgentError::Llm(e));
    }
};

let (thinking, tool_calls, content) = match consume_llm_stream(llm_stream).await {
    Ok(data) => data,
    Err(e) => {
        return Err(e);
    }
};

if !thinking.is_empty() {
    run_ctx.record_reasoning_step(thinking.clone(), None).await;
}

let thinking_event = AgentStreamEvent::ThinkingComplete { thinking };
run_ctx.emit(thinking_event).await;
```

Replace with:

```rust
// Emit LLMCallStart
run_ctx.emit(AgentStreamEvent::LLMCallStart { iteration }).await;

let llm_stream = match llm.converse_stream(request).await {
    Ok(stream) => stream,
    Err(e) => {
        run_ctx.emit(AgentStreamEvent::LLMCallError {
            error: e.to_string(),
        }).await;
        run_ctx.emit(AgentStreamEvent::AgentAborted {
            reason: format!("LLM request failed: {}", e),
        }).await;
        return Err(crate::AgentError::Llm(e));
    }
};

// Consume LLM stream — emits Thinking/Content streaming events internally
let (thinking, tool_calls, content) = match consume_llm_stream(llm_stream, &run_ctx).await {
    Ok(data) => data,
    Err(e) => {
        run_ctx.emit(AgentStreamEvent::LLMCallError {
            error: e.to_string(),
        }).await;
        run_ctx.emit(AgentStreamEvent::AgentAborted {
            reason: format!("LLM stream failed: {}", e),
        }).await;
        return Err(e);
    }
};

// Record reasoning step
if !thinking.is_empty() {
    run_ctx.record_reasoning_step(thinking.clone(), None).await;
}

// Emit LLMCallComplete (thinking/content/ToolCallComplete events already emitted by consume_llm_stream)
// Extract model name from LLM client
let model = self.llm.model().to_string();
run_ctx.emit(AgentStreamEvent::LLMCallComplete {
    model,
    usage: None, // TODO: get from LLM stream when available
}).await;
```

Note: Need to check if `LLMClient` has a `model()` method. If not, use a placeholder or get from config.

Actually, looking at the agent struct, we have `self.llm: Arc<dyn LLMClient>`. The `LLMClient` trait likely has a way to get the model. If not, we can pass it from `config` or just use an empty string. For now, use `String::new()` or get from a method if available.

- [ ] **Step 3: Fix tool execution error path to emit ToolCallError + AgentAborted**

In the tool execution section (lines 447-469), the current code returns `Err` without emitting any event. Update:

Current (lines 447-469):
```rust
let result = match tools.execute(call, &tool_ctx).await {
    Ok(r) => r,
    Err(e) => {
        run_ctx.record_tool_call(ToolCallRecord { ... }).await;
        run_ctx.set_error(...).await;
        return Err(crate::AgentError::ToolExecution { ... });
    }
};
```

Replace the error branch with:

```rust
let result = match tools.execute(call, &tool_ctx).await {
    Ok(r) => r,
    Err(e) => {
        // Emit ToolCallError
        run_ctx.emit(AgentStreamEvent::ToolCallError {
            tool_call_id: call.id.clone(),
            tool_name: call.name.clone(),
            error: e.to_string(),
        }).await;

        // Record failed tool call
        run_ctx.record_tool_call(ToolCallRecord {
            tool_name: call.name.clone(),
            arguments: call.arguments.clone(),
            result: format!("Error: {}", e),
            iteration,
            success: false,
        }).await;

        // Emit AgentAborted (terminal event)
        let reason = format!("Tool execution failed: {}", e);
        run_ctx.emit(AgentStreamEvent::AgentAborted { reason }).await;

        // Set error in RunContext
        run_ctx.set_error(format!("Tool execution failed: {}", e)).await;

        return Err(crate::AgentError::ToolExecution {
            tool: call.name.clone(),
            error: e.to_string(),
        });
    }
};
```

Also remove the duplicate `record_tool_call` and `set_error` that currently happen after the `Err` branch returns (lines 451-469), since we've moved them inside.

- [ ] **Step 4: Fix HITL rejection to emit ToolCallSkipped**

In the HITL rejection path (lines 396-410), currently it just continues without emitting a skipped event. Update:

Current (lines 396-410):
```rust
Ok(approval) if !approval.approved => {
    info!(...);
    if let Err(e) = run_ctx.add_message(...).await { ... }
    run_ctx.clear_current_tool_calls().await;
    continue;
}
```

Replace with:

```rust
Ok(approval) if !approval.approved => {
    info!(tool = %call.name, reason = %reason, "Tool execution rejected by HITL");

    // Emit ToolCallSkipped
    run_ctx.emit(AgentStreamEvent::ToolCallSkipped {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        reason: "User rejected".to_string(),
    }).await;

    // Add rejection message to history
    if let Err(e) = run_ctx.add_message(Message::tool(
        "Execution rejected: permission denied".to_string(),
        call.id.clone(),
    )).await {
        return Err(crate::AgentError::from(e));
    }
    run_ctx.clear_current_tool_calls().await;
    continue;
}
```

- [ ] **Step 5: Fix plugin Skip decision to emit ToolCallSkipped**

The `PluginDecision::Skip` path (lines 427-430) also needs ToolCallSkipped:

```rust
PluginDecision::Skip => {
    debug!("Plugin intercepted to skip tool: {}", call.name);

    run_ctx.emit(AgentStreamEvent::ToolCallSkipped {
        tool_call_id: call.id.clone(),
        tool_name: call.name.clone(),
        reason: "Plugin skipped".to_string(),
    }).await;

    continue;
}
```

- [ ] **Step 6: Fix max iterations to emit AgentAborted with iteration info**

The max iterations check (lines 253-265) already emits `AgentAborted` — this is correct. No change needed.

- [ ] **Step 7: Remove standalone ThinkingComplete emit**

Remove lines 335-337 which emit `ThinkingComplete` standalone — it's now emitted inside `consume_llm_stream`.

- [ ] **Step 8: Add model() method to LLMClient trait (if not exists)**

Check if `LLMClient` has a `model()` method. If not, add one:

```rust
// In crates/vol-llm-core/src/client.rs or similar
fn model(&self) -> &str;
```

And implement it in the Anthropic provider. If this is too invasive, just pass the model from `run_ctx.config` or use a placeholder.

Alternatively, just use `String::new()` for the model field in `LLMCallComplete` for now — the observability module can fill it in later.

- [ ] **Step 9: Verify compilation**

```bash
cargo check -p vol-llm-agent
```

Expected: Compiles successfully. Tests may have match exhaustiveness errors.

- [ ] **Step 10: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: emit complete lifecycle events in agent loop with full error paths"
```

---

### Task 4: Update SessionListener for New Event Recording Rules

**Files:**
- Modify: `crates/vol-session/src/listener.rs:51-129`

- [ ] **Step 1: Update should_record()**

Replace the `should_record()` function (lines 51-60):

```rust
fn should_record(event: &AgentStreamEvent) -> bool {
    matches!(
        event,
        // Lifecycle
        AgentStreamEvent::AgentStart { .. }
        // LLM calls (for observability, not session restoration)
        // | AgentStreamEvent::LLMCallStart { .. }    // NOT recorded — metadata only
        // | AgentStreamEvent::LLMCallComplete { .. } // NOT recorded — metadata only
        // | AgentStreamEvent::LLMCallError { .. }    // NOT recorded — metadata only
        // Thinking deltas — NOT recorded
        // Content deltas — NOT recorded
        | AgentStreamEvent::ThinkingComplete { .. }
        | AgentStreamEvent::ContentComplete { .. }
        | AgentStreamEvent::ToolCallBegin { .. }
        | AgentStreamEvent::ToolCallComplete { .. }
        | AgentStreamEvent::ToolCallError { .. }
        | AgentStreamEvent::ToolCallSkipped { .. }
        | AgentStreamEvent::IterationComplete { .. }
        // AgentComplete/AgentAborted — NOT recorded (no corresponding message type)
    )
}
```

- [ ] **Step 2: Update event_to_message()**

Add handlers for new events. Add after the existing `ThinkingComplete` handler (line 81):

```rust
// ContentComplete -> Assistant message (content)
AgentStreamEvent::ContentComplete { content } => Some(SessionMessage::new(
    self.session_id.clone(),
    vol_llm_core::Message::assistant(content.clone()),
)),
```

Add after the existing `ToolCallComplete` handler (line 114):

```rust
// ToolCallError -> Tool message with error
AgentStreamEvent::ToolCallError {
    tool_call_id,
    tool_name,
    error,
} => {
    let content = format!("Tool '{}' error: {}", tool_name, error);
    Some(SessionMessage::new(
        self.session_id.clone(),
        vol_llm_core::Message::tool(content, tool_call_id.clone()),
    ))
}

// ToolCallSkipped -> Tool message with skip reason
AgentStreamEvent::ToolCallSkipped {
    tool_call_id,
    tool_name,
    reason,
} => {
    let content = format!("Tool '{}' skipped: {}", tool_name, reason);
    Some(SessionMessage::new(
        self.session_id.clone(),
        vol_llm_core::Message::tool(content, tool_call_id.clone()),
    ))
}
```

- [ ] **Step 3: Update tests**

Update `should_record` tests in listener.rs:
- Add test for `ContentComplete` → should record
- Add test for `ToolCallError` → should record
- Add test for `ToolCallSkipped` → should record
- Add test for `ThinkingStart` → should NOT record
- Add test for `ThinkingDelta` → should NOT record
- Add test for `ContentStart` → should NOT record
- Add test for `ContentDelta` → should NOT record
- Add test for `LLMCallStart` → should NOT record
- Add test for `AgentComplete` → should NOT record

Add `event_to_message` tests for:
- `ContentComplete` → Assistant message
- `ToolCallError` → Tool message with error
- `ToolCallSkipped` → Tool message with skip reason

- [ ] **Step 4: Verify compilation and tests**

```bash
cargo check -p vol-session
cargo test -p vol-session
```

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "feat: update SessionListener for 18-event lifecycle recording rules"
```

---

### Task 5: Update TUI Renderer for New Events

**Files:**
- Modify: `crates/vol-llm-tui/src/render.rs:14-83`

- [ ] **Step 1: Update render_event() to handle all 18 variants**

Replace the `render_event()` function with handlers for all new events:

```rust
pub fn render_event(event: &AgentStreamEvent) {
    match event {
        // Lifecycle
        AgentStreamEvent::AgentStart { input } => {
            println!();
            print_colored(Color::Cyan, &format!(">>> {}\n", input));
        }

        AgentStreamEvent::AgentComplete => {
            println!();
            print_colored(Color::Green, "Done.\n");
        }

        AgentStreamEvent::AgentAborted { reason } => {
            println!();
            print_colored(Color::Red, &format!("Aborted: {}\n", reason));
        }

        // LLM Call — meta events, not displayed to user
        AgentStreamEvent::LLMCallStart { .. } => {}
        AgentStreamEvent::LLMCallComplete { .. } => {}
        AgentStreamEvent::LLMCallError { .. } => {}

        // Thinking
        AgentStreamEvent::ThinkingStart => {
            print_colored(Color::Yellow, "\nThinking...\n");
        }

        AgentStreamEvent::ThinkingDelta { delta } => {
            print_colored(Color::DarkGrey, delta);
        }

        AgentStreamEvent::ThinkingComplete { thinking } => {
            if !thinking.is_empty() {
                print_colored(Color::DarkGrey, &format!("  [thinking complete]\n"));
            }
        }

        // Content
        AgentStreamEvent::ContentStart => {
            println!();
        }

        AgentStreamEvent::ContentDelta { delta } => {
            print_colored(Color::White, delta);
        }

        AgentStreamEvent::ContentComplete { content } => {
            // Content already streamed via deltas, just add newline
            if content.is_empty() {
                println!();
            }
        }

        // Tools
        AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
            println!();
            print_colored(Color::Blue, &format!("[{}] ", tool_name));
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
                if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                    print_colored(Color::DarkGrey, &format!("Command: {}\n", cmd));
                } else if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                    print_colored(Color::DarkGrey, &format!("Path: {}\n", path));
                } else {
                    print_colored(Color::DarkGrey, &format!("Args: {}\n", arguments));
                }
            } else {
                print_colored(Color::DarkGrey, &format!("Args: {}\n", arguments));
            }
        }

        AgentStreamEvent::ToolCallComplete { tool_name, result, .. } => {
            print_colored(Color::Green, &format!("  ✓ {} completed\n", tool_name));
            let preview = if result.len() > 300 {
                format!("{}...", &result[..300])
            } else {
                result.clone()
            };
            for line in preview.lines().take(10) {
                println!("    {}", line);
            }
        }

        AgentStreamEvent::ToolCallError { tool_name, error, .. } => {
            println!();
            print_colored(Color::Red, &format!("  ✗ {} failed: {}\n", tool_name, error));
        }

        AgentStreamEvent::ToolCallSkipped { tool_name, reason, .. } => {
            println!();
            print_colored(Color::DarkGrey, &format!("  ⊘ {} skipped: {}\n", tool_name, reason));
        }

        // Iteration
        AgentStreamEvent::IterationComplete { final_answer: Some(answer), .. } => {
            println!();
            print_colored(Color::Green, &format!("✓ {}\n", answer));
        }

        AgentStreamEvent::IterationComplete { iteration, .. } => {
            print_colored(Color::White, &format!("\n[Iteration {} complete]\n", iteration));
        }

        // Plugin
        AgentStreamEvent::PluginEvent { .. } => {}
    }
    let _ = stdout().flush();
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-tui
```

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/render.rs
git commit -m "feat: update TUI renderer for 18 lifecycle events"
```

---

### Task 6: Update All Tests for New Enum Variants

**Files:**
- All test files that match on `AgentStreamEvent`:
  - `crates/vol-llm-agent/tests/*.rs`
  - `crates/vol-llm-agents/tests/*.rs`

- [ ] **Step 1: Find all files that need updating**

```bash
grep -rl 'AgentStreamEvent' crates/vol-llm-agent/tests/ crates/vol-llm-agents/tests/ 2>/dev/null
```

- [ ] **Step 2: Update each test file**

For each file, the `match` on `AgentStreamEvent` needs to either:
- Explicitly handle all 18 variants, OR
- Add a `_ => {}` wildcard arm

Given that most tests only care about specific events, use `_ => {}` for the exhaustive match in test code. For tests that specifically check event types, add the new variants they need.

Run:

```bash
cargo check --workspace --tests 2>&1 | grep "non-exhaustive"
```

This will list all files with match exhaustiveness errors. Fix each one.

- [ ] **Step 3: Update observer plugin tests**

The observability plugin tests and observer integration tests likely need updates for the new events. Check:
- `crates/vol-llm-agents/tests/observer_integration.rs`
- `crates/vol-llm-agents/tests/observer_plugin_unit.rs`
- `crates/vol-llm-agents/tests/channelled_observer_integration.rs`
- `crates/vol-llm-agents/tests/channelled_observer_unit.rs`

- [ ] **Step 4: Verify all tests compile**

```bash
cargo check --workspace --tests
```

Expected: No errors

- [ ] **Step 5: Run all tests**

```bash
cargo test --workspace --lib
```

Expected: All tests pass (some integration tests may be skipped if env vars not set)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/tests/ crates/vol-llm-agents/tests/
git commit -m "fix: update all tests for 18-variant AgentStreamEvent enum"
```

---

### Task 7: Final Verification

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace --lib
```

- [ ] **Step 3: Manual smoke test**

```bash
cargo build -p vol-llm-tui
source .env && ./target/debug/vol-llm-tui
```

Verify:
1. Simple question → ThinkingStart, ThinkingDelta×N, ThinkingComplete, ContentStart, ContentDelta×N, ContentComplete, LLMCallComplete, IterationComplete, AgentComplete all emitted
2. Tool-using question → ToolCallBegin, ToolCallComplete events visible
3. Events render correctly with colors

- [ ] **Step 4: Commit**

```bash
git add .
git commit -m "chore: final verification for agent lifecycle events"
```

---

## Summary of Changes

| Crate | Files | Purpose |
|-------|-------|---------|
| `vol-llm-core` | `stream.rs` | AgentStreamEvent: 8 → 18 variants |
| `vol-llm-agent` | `run_context.rs` | Channel capacity 100 → 1024 |
| `vol-llm-agent` | `agent.rs` | consume_llm_stream emits deltas, LLM call events, complete error paths |
| `vol-session` | `listener.rs` | should_record + event_to_message for new events |
| `vol-llm-tui` | `render.rs` | Render 10 new event variants |
| Tests | Multiple | Match exhaustiveness fixes |
