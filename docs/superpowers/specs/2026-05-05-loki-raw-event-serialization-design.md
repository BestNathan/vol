---
name: loki-raw-event-serialization-design
description: Loki plugin sends full AgentStreamEvent JSON instead of hand-assembled subsets, while still filtering delta events. Uses event's own timestamp.
status: draft
created: 2026-05-05
---

# Loki Raw Event Serialization Design

## Context

The LokiPlugin currently builds log lines by manually extracting a subset of fields from each `AgentStreamEvent` variant via `event_data()`. This loses data: `LLMCallStart.messages`, `ToolCallComplete.timestamp`, `IterationComplete.timestamp`, etc. are all dropped.

## Decision

Serialize the full `AgentStreamEvent` as JSON for the Loki log line, with `run_id`, `session_id`, and `agent_id` metadata fields merged in. Delta events are still filtered by `should_send()` as before.

## Changes

### 1. `AgentStreamEvent` gets `Serialize` derive

Add `Serialize` to the derive in `vol-llm-core/src/stream.rs`:

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum AgentStreamEvent { ... }
```

This requires all variant field types to also be `Serialize`, which they already are (`Message`, `ToolCall`, `TokenUsage`, `chrono::DateTime<Utc>` all implement it).

### 2. Add `timestamp()` helper to `AgentStreamEvent`

Each variant has a `timestamp: chrono::DateTime<chrono::Utc>` field. A new method extracts it:

```rust
impl AgentStreamEvent {
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::AgentStart { timestamp, .. } => *timestamp,
            // ... all variants
        }
    }
}
```

### 3. Simplify `LokiPlugin::create_loki_entry`

Replace the hand-assembled `line_map` with:

1. Serialize the full event: `let event_json = serde_json::to_value(event).unwrap()`
2. Extract it as a `Map`, inject `run_id`, `session_id`, `agent_id`
3. Convert back to string for the log line

The `event_name` helper is kept (it's used to know which event type occurred, useful for filtering in Loki queries). The `event_data` and `event_tool_name` helpers are removed.

The `model` label on `LLMCallComplete` is still handled by pattern-matching the event to extract the model value for the label.

### 4. Simplify `event_name`

The `unreachable!` arms for delta variants remain (since `should_send()` filters them before `event_name` is called). No functional change.

### 5. Update tests

Tests that assert specific line content are updated to match serialized output. `test_loki_entry_tool_call` verifies that all event fields (`timestamp`, `tool_call_id`, `tool_name`, `arguments`) appear in the line, not just the previously-extracted subset.

## Not Changed

- `should_send()` — delta filtering stays identical
- Loki client, batch writer, flush logic
- Label building (namespace, agent, agent_id, model)
- Plugin interface (`intercept`, `listen`)

## Log Line Format

Before:
```json
{"timestamp":"2026-05-05T10:00:00Z","event":"ToolCallBegin","run_id":"r1","session_id":"s1","agent_id":"k8s_ops_agent","tool_call_id":"c1","tool_name":"bash","arguments":"{}"}
```

After (same fields plus event's own timestamp and any fields previously omitted):
```json
{"event":"ToolCallBegin","timestamp":"2026-05-05T10:00:00Z","tool_call_id":"c1","tool_name":"bash","arguments":"{}","run_id":"r1","session_id":"s1","agent_id":"k8s_ops_agent"}
```

For `LLMCallStart`, the `messages` array is now included (previously dropped).
