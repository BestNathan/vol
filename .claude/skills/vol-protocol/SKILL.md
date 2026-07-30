---
name: vol-protocol
description: Use when adding new operations, methods, or payloads to the vol-agent JSON-RPC protocol. Covers all files that MUST be updated — missing any causes silent failures at runtime.
---

# Extending the vol-agent Protocol

Adding a new operation (e.g., `agent.get_capabilities`) touches multiple files. Missing one causes silent failures (no compile error, no test failure, runtime hang).

## Checklist: 6 files, in order

### 1. Protocol wire types — `agent_server_protocol.rs`

Path: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs`

- [ ] **Add variant to operation enum** (e.g., `AgentOperation::GetCapabilities`)
- [ ] **Add variant to payload enum** (e.g., `AgentPayload::GetCapabilities { ... }`)
- [ ] **Add method_name mapping** in `Operation::method_name()`
- [ ] **Add decode arm** in `Payload::from_operation()` for params deserialization
- [ ] **Run tests**: `cargo test -p vol-llm-agent-protocol`

### 2. JSON-RPC codec — `operation_codec.rs`

Path: `crates/vol-llm-agent-protocol/src/operation_codec.rs`

**This is the one that was missed. There is no compile error if you skip it.**

- [ ] **Add method string mapping** in `method_to_operation()`:
  ```rust
  "agent.new_operation" => Ok(Operation::Agent(AgentOperation::NewOp)),
  ```

Without this, the JSON-RPC decoder returns `UnknownMethod` for any incoming request, the WebSocket connection closes immediately, and the handler is never invoked. Unit tests pass because they test the handler directly, not through the codec.

### 3. Data plane handler — `capability.rs` (or new handler file)

Path: `crates/vol-agent-server/src/data_plane/handlers/`

- [ ] **Create or update handler** implementing `DomainHandler`
- [ ] Add `operations()` returning the new `Operation` variants
- [ ] Implement `handle()` with match arms for each operation
- [ ] **Add `pub mod` declaration** in `handlers/mod.rs`

### 4. Handler registration — `data_plane/core.rs`

Path: `crates/vol-agent-server/src/data_plane/core.rs`

- [ ] **Import the handler** in the handler imports block
- [ ] **Register handler** in `DataPlaneServerCoreBuilder::build()`:
  ```rust
  handler_registry.register(Arc::new(NewHandler::new(...)))
      .map_err(|e| format!("failed to register NewHandler: {e}"))?;
  ```
- [ ] **Also register in `for_test()`** for tests

### 5. AgentHandler catch-all arms — `data_plane/handlers/agent.rs`

Path: `crates/vol-agent-server/src/data_plane/handlers/agent.rs`

- [ ] **Add dead-arm fallthrough** for new operations (prevents match panics if dispatcher misroutes):
  ```rust
  (AgentOperation::NewOp, _) => Err(ProtocolError::PayloadDecodeFailed("agent.new_op")),
  ```
  This is NOT in the `operations()` list — it's a safety net in the `handle()` method.

### 6. Control plane routing (optional, only if CP must forward to DP)

Path: `crates/vol-agent-server/src/control_plane/handlers/client.rs`

- [ ] **Add to `operations()`** if CP should accept the operation from clients
- [ ] **Add routing logic** in `handle()` to forward to the correct DP node via `ControlRouter`

## Verification

After all 6 files:

```bash
# Compile check
cargo check -p vol-llm-agent-protocol -p vol-agent-server

# Run protocol tests
cargo test -p vol-llm-agent-protocol

# Run handler tests  
cargo test -p vol-agent-server -- <test_name>

# Test end-to-end with port-forward
kubectl port-forward -n vol-agent-system <dp-pod> 3002:3002
echo '{"jsonrpc":"2.0","method":"agent.<new_op>","params":{...},"id":1}' | websocat -n1 ws://localhost:3002/ws
```

## Files Summary

| # | File | What | Miss detection |
|---|------|------|---------------|
| 1 | `agent_server_protocol.rs` | Wire types, enums, method_name | Compile error if enum variant missing |
| 2 | **`operation_codec.rs`** | method_to_operation mapping | **SILENT** — runtime UnknownMethod, WS closes |
| 3 | `data_plane/handlers/*.rs` | DomainHandler impl | Compile error via handler registration |
| 4 | `data_plane/core.rs` | Handler registration | `?` causes startup error |
| 5 | `data_plane/handlers/agent.rs` | Dead-arm safety net | Match exhaustiveness warning |
| 6 | `control_plane/handlers/client.rs` | CP routing (optional) | "unsupported client operation" error |
