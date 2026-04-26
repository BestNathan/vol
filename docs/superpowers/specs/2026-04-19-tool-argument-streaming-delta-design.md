# Tool Call Argument Streaming Delta Event

> **Problem:** When the LLM streams tool call arguments via `partial_json`, the streaming layer silently accumulates them without emitting any intermediate events. For large argument payloads (e.g., Write tool with long file content), callers see no activity for seconds — poor UX.

> **Goal:** Add a single `ToolCallArgumentDelta` event that fires for each `partial_json` chunk, allowing downstream consumers (TUI, observability, etc.) to know the agent is actively working.

## Architecture

### Current Behavior

```
LLM Stream:
  content_block_start { type: "tool_use", id: "call_123", name: "write" }
  content_block_delta { delta: { partial_json: "{\"file_path\": " }  ← silently accumulated
  content_block_delta { delta: { partial_json: "\"main.rs\", " }      ← silently accumulated
  content_block_delta { delta: { partial_json: "\"content\": \"..."} } ← silently accumulated
  content_block_stop → ToolCallComplete { tool_call }               ← only event emitted
```

### New Behavior

```
LLM Stream:
  content_block_start → (records id/name internally)
  content_block_delta { partial_json } → ToolCallArgumentDelta { tool_call_id, tool_name, delta }
  content_block_delta { partial_json } → ToolCallArgumentDelta { tool_call_id, tool_name, delta }
  content_block_delta { partial_json } → ToolCallArgumentDelta { tool_call_id, tool_name, delta }
  content_block_stop → ToolCallComplete { tool_call }               ← unchanged
```

## Implementation Details

### File: `crates/vol-llm-core/src/stream.rs`

#### 1. Add `ToolCallArgumentDelta` to `StreamEventData`

Add to the enum (around line 30, in the Tool calls section):

```rust
// Tool call argument streaming
ToolCallArgumentDelta {
    tool_call_id: String,
    tool_name: String,
    delta: String,
},
```

#### 2. Add `ToolCallArgumentDelta` to `AgentStreamEvent`

Add to the enum (around line 163, in the Tool Execution section):

```rust
// === Tool Argument Streaming (1) ===
ToolCallArgumentDelta {
    timestamp: chrono::DateTime<chrono::Utc>,
    tool_call_id: String,
    tool_name: String,
    delta: String,
},
```

#### 3. Add constructor helper

In `impl AgentStreamEvent`:

```rust
pub fn tool_call_argument_delta(tool_call_id: String, tool_name: String, delta: String) -> Self {
    Self::ToolCallArgumentDelta {
        timestamp: chrono::Utc::now(),
        tool_call_id,
        tool_name,
        delta,
    }
}
```

### File: `crates/vol-llm-core/src/streaming.rs`

#### 1. Enhance `ToolCallBuilder` to track id/name for delta emission

`ToolCallBuilder` already has `id: Option<String>` and `name: Option<String>` fields. No change needed to struct.

#### 2. Modify `handle_content_block_delta` to emit delta events

Replace the `partial_json` block (lines 153-159):

```rust
// Before:
if let Some(input) = data["delta"]["partial_json"].as_str() {
    if let Some(ref mut builder) = self.current_tool_call {
        builder.arguments.push_str(input);
    }
    return None;
}

// After:
if let Some(input) = data["delta"]["partial_json"].as_str() {
    if let Some(ref mut builder) = self.current_tool_call {
        builder.arguments.push_str(input);
        let tool_call_id = builder.id.clone().unwrap_or_default();
        let tool_name = builder.name.clone().unwrap_or_default();
        return Some(Ok(StreamEvent {
            id: self.next_id(),
            data: StreamEventData::ToolCallArgumentDelta {
                tool_call_id,
                tool_name,
                delta: input.to_string(),
            },
        }));
    }
}
```

### File: `crates/vol-llm-agent/src/react/agent.rs`

#### 1. Add match arm in `consume_llm_stream`

In the `consume_llm_stream` function's event match (around line 706), add before `_ => {}`:

```rust
StreamEventData::ToolCallArgumentDelta { tool_call_id, tool_name, delta } => {
    run_ctx
        .emit(AgentStreamEvent::tool_call_argument_delta(
            tool_call_id.clone(),
            tool_name.clone(),
            delta.clone(),
        ))
        .await;
}
```

## What's NOT Changed

- `ToolCallBegin` — fires in the act phase during tool execution, unchanged
- `ToolCallComplete` — fires at `content_block_stop` with full accumulated arguments, unchanged
- TUI render layer — no changes for now, consumers can add handling as needed
- `StreamEventData` serialization format — new variant uses existing `#[serde(tag = "type", rename_all = "snake_case")]`

## Data Flow

```
LLM Provider → StreamingSession.process_anthropic_sse()
  ├─ content_block_start (tool_use) → stores id/name in builder (no event)
  ├─ content_block_delta (partial_json) → ToolCallArgumentDelta { id, name, chunk }
  └─ content_block_stop → ToolCallComplete { tool_call }

consume_llm_stream:
  ├─ StreamEvent::ToolCallArgumentDelta → emit AgentStreamEvent::ToolCallArgumentDelta
  └─ StreamEvent::ToolCallComplete → (existing handling, adds to tool_calls vec)

Downstream consumers (future):
  ├─ TUI: show "working..." indicator, update token progress
  ├─ Observability: log argument streaming latency
  └─ Session: optionally record argument chunks
```

## Backwards Compatibility

- New enum variant — all existing `match` statements need a `_` arm or explicit handling
- No existing API signatures change
- No behavior change for callers that don't listen for this event

## Risks

1. **Event volume:** For very large arguments with many small partial_json chunks, many delta events fire. Broadcast channel capacity (1024) is sufficient — each chunk is small.
2. **Partial JSON:** Delta content is raw partial JSON, not valid JSON by itself. Consumers should not parse individual deltas — only the accumulated result from `ToolCallComplete`.
