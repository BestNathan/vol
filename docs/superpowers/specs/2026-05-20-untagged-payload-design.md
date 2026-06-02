# Untagged Payload Design

## Summary

Remove `#[serde(tag = "domain", content = "data")]` from `Payload` enum,
delete `flatten_payload()`, and rewrite `decode_payload()` to use direct
`serde_json::from_value` per operation — since method-to-payload-variant
is a 1:1 mapping.

## Motivation

Currently `Payload` is internally tagged:

```json
{"domain": "file", "data": {"List": {"path": "/tmp"}}}
```

The `domain`/`data` wrapper is redundant: `method_to_operation(method)`
already tells us the exact domain (File/Agent/Session/...) and the operation
(List/Read/Submit/...) — which maps to exactly one payload variant. The
wrapper adds nothing for deserialization and `flatten_payload()` has to
strip it on the encode path.

## Design

### `Payload` → `#[serde(untagged)]`

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Payload {
    Agent(AgentPayload),
    File(FilePayload),
    Session(SessionPayload),
    Mcp(McpPayload),
    Skill(SkillPayload),
    Log(LogPayload),
    System(SystemPayload),
    Error(ErrorPayload),
}
```

Sub-payload enums keep their default externally-tagged serialization
(e.g., `FilePayload::List { path }` serializes as `{"List":{"path":"/tmp"}}`).
This avoids ambiguity between variants with identical field signatures
(e.g., `FilePayload::List { path }` and `FilePayload::Read { path }`).

### Encode path

`serde_json::to_value(&msg.payload)` now produces `{"SubmitResult":{"run_id":"x","response":{...}}}` instead of `{"domain":"agent","data":{"SubmitResult":{...}}}`.

`flatten_payload()` is deleted. Instead, `encode_jsonrpc_message` for
`Ack | Result` matches on the payload to extract inner data directly,
like `Error` already does:

```rust
MessageKind::Ack | MessageKind::Result => {
    let id = parse_message_id_for_jsonrpc(&msg.message_id);
    let result = payload_data(&msg.payload)
        .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))?;
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0", "id": id, "result": result,
    }))
    .map_err(|e| ConnectionError::ParseError(format!("serialization error: {e}")))
}
```

`payload_data()` extracts the inner value:

```rust
fn payload_data(payload: &Payload) -> Result<serde_json::Value, serde_json::Error> {
    match payload {
        Payload::Agent(p) => serde_json::to_value(p).map(strip_variant),
        Payload::File(p) => serde_json::to_value(p).map(strip_variant),
        Payload::Session(p) => serde_json::to_value(p).map(strip_variant),
        Payload::Mcp(p) => serde_json::to_value(p).map(strip_variant),
        Payload::Skill(p) => serde_json::to_value(p).map(strip_variant),
        Payload::Log(p) => serde_json::to_value(p).map(strip_variant),
        Payload::System(p) => serde_json::to_value(p).map(strip_variant),
        Payload::Error(p) => serde_json::to_value(p),
    }
}

fn strip_variant(val: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = val.as_object() {
        if obj.len() == 1 {
            if let Some((_key, inner)) = obj.iter().next() {
                return inner.clone();
            }
        }
    }
    val
}
```

Event and Command paths continue using `serde_json::to_value(&msg.payload)`
directly without stripping — their format is internal, not frontend-facing.

### Decode path — rewrite `decode_payload()`

Each operation arm now uses `serde_json::from_value::<XxxPayload>(value)?`
with variant validation:

```rust
Operation::File(FileOperation::List) => {
    let payload: FilePayload = serde_json::from_value(value)
        .map_err(|e| ProtocolError::PayloadDecodeFailed("file.list"))?;
    match &payload {
        FilePayload::List { .. } => Ok(Payload::File(payload)),
        _ => Err(ProtocolError::PayloadDecodeFailed("file.list")),
    }
}
Operation::File(FileOperation::Read) => {
    let payload: FilePayload = serde_json::from_value(value)
        .map_err(|e| ProtocolError::PayloadDecodeFailed("file.read"))?;
    match &payload {
        FilePayload::Read { .. } => Ok(Payload::File(payload)),
        _ => Err(ProtocolError::PayloadDecodeFailed("file.read")),
    }
}
```

Manual field-by-field extraction is replaced by serde deserialization.
The post-match validates the correct variant arrived.

### Inbound wire format change

Client-side calls change from:

```json
{"jsonrpc":"2.0","id":1,"method":"file.list","params":{"domain":"file","data":{"List":{"path":"/tmp"}}}}
```

To:

```json
{"jsonrpc":"2.0","id":1,"method":"file.list","params":{"List":{"path":"/tmp"}}}
```

`decode_jsonrpc_frame` already calls `decode_payload(operation, params)`
where `params` is the raw `params` value — no change needed to the gateway.

The `params` still contains the variant name wrapper (`{"List": ...}`)
because sub-payloads stay externally tagged for inbound safety.

### Files Changed

| File | Change |
|------|--------|
| `src/agent_server_protocol.rs` | `Payload`: `#[serde(tag="domain",content="data")]` → `#[serde(untagged)]` |
| `src/operation_codec.rs` | Rewrite `decode_payload()`: `serde_json::from_value` per operation |
| `src/gateway/jsonrpc_ws.rs` | Delete `flatten_payload()`. Add `payload_data()` + `strip_variant()`. Update `encode_jsonrpc_message` Ack/Result arm. |

### What stays the same

- `method_to_operation()` — unchanged
- `decode_jsonrpc_frame()` — unchanged  
- Sub-payload enums — still externally tagged
- Agent/sub-payload `#[serde(...)]` annotations on fields — unchanged
- All handler code — unchanged (handlers construct `Payload::Xxx(...)`, unaffected)

### Error handling

- `serde_json::from_value` failure → `ProtocolError::PayloadDecodeFailed(method_name)`
- Wrong variant after deserialization → same error
- Encode serialization failure → `ConnectionError::ParseError`
