# Design: UI JSON-RPC Protocol Update

## Summary

Update `vol-llm-ui/src/web/client.rs` raw JSON wire format to match the new `vol-llm-agent-channel` protocol. Key changes: `req_id` → `run_id`, event format simplified from nested `{subscription, result: {req_id, event_type, data}}` to flat `{run_id, event}`, and `agent_event_to_ui` remapped to `AgentStreamEvent` CamelCase variant keys.

## Type changes

### AgentEvent struct (client.rs:24-30)

```rust
// Before
pub struct AgentEvent {
    pub req_id: String,
    pub event_type: String,
    pub data: serde_json::Value,
}

// After
pub struct AgentEvent {
    pub run_id: String,
    pub event: serde_json::Value,
}
```

### Event wire format

**Before:**
```json
{"jsonrpc":"2.0","method":"agent.event","params":{"subscription":1,"result":{"req_id":"x","event_type":"agent_start","data":{"input":"hi"}}}}
```

**After (from `encode_jsonrpc_message` for MessageKind::Event → AgentPayload::Event):**
```json
{"jsonrpc":"2.0","method":"agent.event","params":{"run_id":"x","event":{"AgentStart":{"timestamp":"...","input":"hi"}}}}
```

### handle_message event parsing (client.rs:835-859)

Old code checks `params.result.req_id` + `params.result.event_type` + `params.result.data`. New code extracts `params.run_id` + `params.event` directly.

### agent.cancel params (client.rs:301-304)

```json
// Before
{"jsonrpc":"2.0","method":"agent.cancel","params":{"req_id":"<id>"},"id":<N>}

// After
{"jsonrpc":"2.0","method":"agent.cancel","params":{"run_id":"<id>"},"id":<N>}
```

### agent.submit (client.rs:220-224)

No structural change — `"input": "<text>"` is backward-compatible with `AgentInput` deserialization. Optionally add `"target"` key.

## agent_event_to_ui remapping (app.rs:55-126)

`AgentStreamEvent` serializes as externally-tagged CamelCase enum. Old `event_type` snake_case → new variant key + field mapping:

| Old event_type | New variant key | Field changes |
|---|---|---|
| `agent_start` | `AgentStart` | `data.input` → `event.AgentStart.input` |
| `agent_complete` | `AgentComplete` | `data.response` → `event.AgentComplete.response` |
| `agent_error` | (removed — `LLMCallError` / `AgentAborted`) | |
| `agent_aborted` | `AgentAborted` | `data.reason` → `event.AgentAborted.reason` |
| `thinking_start` | `ThinkingStart` | no data fields |
| `thinking_delta` | `ThinkingDelta` | `data.delta` → `event.ThinkingDelta.delta` |
| `thinking_complete` | `ThinkingComplete` | `data.thinking` replaces old; now `event.ThinkingComplete.thinking` |
| `llm_call_start` | `LLMCallStart` | `data.iteration` → `event.LLMCallStart.iteration` |
| `llm_call_complete` | `LLMCallComplete` | `data.model` → `event.LLMCallComplete.model` |
| `llm_call_error` | `LLMCallError` | `data.error` → `event.LLMCallError.error` |
| `content_start` | `ContentStart` | no data fields |
| `content_delta` | `ContentDelta` | `data.delta` → `event.ContentDelta.delta` |
| `content_complete` | `ContentComplete` | `data.content` → `event.ContentComplete.content` |
| `tool_call_begin` | `ToolCallBegin` | adds `tool_call_id`; `data.tool_name` → `event.ToolCallBegin.tool_name`; `data.arguments` → `event.ToolCallBegin.arguments` |
| `tool_call_complete` | `ToolCallComplete` | adds `tool_call_id`; `data.duration_ms` now `Option<u64>` |
| `tool_call_error` | `ToolCallError` | adds `tool_call_id`; `data.duration_ms` now `Option<u64>` |
| `tool_call_skipped` | `ToolCallSkipped` | adds `tool_call_id`; `data.duration_ms` now `Option<u64>` |
| `max_iterations_reached` | `MaxIterationsReached` | `data.current` → `event.MaxIterationsReached.current_iteration`; `data.max` → `max_iterations` |
| `iteration_continued` | `IterationContinued` | `data.from_iteration` → `event.IterationContinued.from_iteration` |
| `iteration_complete` | `IterationComplete` | `data.iteration` → `event.IterationComplete.iteration`; `data.final_answer` → `final_answer` |

## Files touched

| File | Changes |
|------|---------|
| `crates/vol-llm-ui/src/web/client.rs` | `AgentEvent` struct fields, `handle_message` event parsing, `cancel` params key |
| `crates/vol-llm-ui/src/web/components/app.rs` | `agent_event_to_ui` — remap all 19 event types to new variant keys + field names |

## No dependency changes

`vol-llm-ui` continues using only `serde` + `serde_json`. No new crate deps.
