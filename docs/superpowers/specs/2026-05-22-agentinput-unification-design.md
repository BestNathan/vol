# Design: AgentInput unification across agent-channel

## Summary

Replace `input: String` with `input: AgentInput` in `AgentPayload::Submit`, `AgentRequest`, and the dispatcher call chain. Drop redundant `run_id` and `metadata` fields where `AgentInput` already provides them. Fix the broken `run_with_id` call in the dispatcher — `ReActAgent` only exposes `run_input(AgentInput)`.

## Type changes

### `AgentPayload::Submit`

```rust
// Before
Submit {
    input: String,
    target: Option<String>,
    metadata: Option<serde_json::Map<String, serde_json::Value>>,
    run_id: Option<String>,
}

// After
Submit {
    input: AgentInput,
    target: Option<String>,
}
```

`run_id` and `metadata` are dropped from Submit — they are carried inside `AgentInput`.

### `AgentRequest`

```rust
// Before
AgentRequest {
    run_id: String,
    target_id: String,
    sender_id: Option<String>,
    input: String,
    metadata: HashMap<String, serde_json::Value>,
}

// After
AgentRequest {
    target_id: String,
    sender_id: Option<String>,
    input: AgentInput,
}
```

`run_id` and `metadata` are dropped from AgentRequest. `run_id` access becomes `request.input.run_id` (returns `Option<&str>`). Builders `new()` and `with_run_id()` are updated to accept `AgentInput`.

### `RunResult`

No structural change. `run_id` and `target_id` remain — they are populated from the request at dispatch time, not from the request struct.

### `PendingRequest`

No structural change (internal wrapper, unchanged).

## Dispatch flow

```
Submit { input: AgentInput, target }
  -> AgentHandler builds AgentRequest { target_id, input }
  -> AgentRouter::send()
  -> AgentDispatcher::submit()
  -> dispatcher calls agent.run_input(request.input)
  -> ReActAgent extracts run_id, parts, metadata from AgentInput
```

## Wire decode

`Payload::from_operation` for `AgentOperation::Submit` (agent_server_protocol.rs:156-175) — the local decode struct `P` drops `metadata` and `run_id` fields, keeps only `input: AgentInput` and `target: Option<String>`.

## AgentHandler adaption

`domain/agent.rs` lines 73-76: no longer constructs `AgentRequest::with_run_id(run_id, ...)` with a separate `run_id`. Instead passes the `AgentInput` directly. Target resolution logic is unchanged. `run_id` for the ack/result is read from `request.input.run_id`, falling back to a generated UUID (matching ReActAgent's behavior).

## Dispatcher adaption

`dispatcher.rs` line 136: `agent.run_with_id(...)` replaced with `agent.run_input(request.input.clone())`. Logging reads `run_id` from `request.input.run_id`. `cancel()` continues to match by `run_id` — iterates the queue calling `p.request.input.run_id.as_deref() == Some(run_id)`.

## Backwards compatibility

`AgentInput` already implements `Deserialize` accepting both a plain JSON string (`"input": "hello"` -> `AgentInput::text("hello")`) and a structured object. Existing JSON-RPC clients sending string input continue to work unchanged. Structured clients gain the ability to send multimodal input (text + image) through the same field.

## Error handling

`AgentInput::new()` (empty parts) is valid at rest. If submitted through the channel and dispatched, `ReActAgent::run_input` will call `to_message_content()` which returns `AgentInputError` for empty parts. This is surfaced as `AgentError::InvalidInput` to the caller, which is consistent with the current behavior.

## Files touched

| File | Change |
|------|--------|
| `crates/vol-llm-agent-channel/src/agent_server_protocol.rs` | Submit variant: `input: String` -> `input: AgentInput`, drop `metadata`/`run_id`. Decode struct `P` updated. |
| `crates/vol-llm-agent-channel/src/request.rs` | `AgentRequest`: drop `run_id`/`metadata`, `input: String` -> `input: AgentInput`. Builders updated, tests updated. |
| `crates/vol-llm-agent-channel/src/dispatcher.rs` | `run_with_id` -> `run_input`, access run_id from `request.input.run_id`. `cancel()` matching updated. Tests updated. |
| `crates/vol-llm-agent-channel/src/domain/agent.rs` | Submit handler: build `AgentRequest` directly from decoupled `AgentInput`, read run_id from input. |
| `crates/vol-llm-agent-channel/Cargo.toml` | No change — `vol-llm-agent` is already a direct dependency. `AgentInput` is already re-exported. |

## Test coverage

- `request.rs` tests: update `AgentRequest::new` / `with_run_id` test assertions
- `dispatcher.rs` tests: update `AgentRequest` construction and cancel matching
- `router.rs` tests: update `AgentRequest` construction
- E2E JSON-RPC test (`tests/`): verify string input (backwards compat) and structured input (multimodal) via agent.submit
