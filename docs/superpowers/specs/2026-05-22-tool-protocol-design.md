# Design: Tool Protocol Operations

## Summary

Add `ToolOperation` / `ToolPayload` to the agent server protocol for listing and calling system tools. Backend provides `ToolHandler` using `ToolRegistry`. Frontend `tools_panel` updates to use `tool.list` / `tool.call` JSON-RPC methods.

## Protocol

### New `ToolOperation` enum

```rust
pub enum ToolOperation {
    List,
    Call,
}
```

Wire methods: `tool.list`, `tool.call`

### New `ToolPayload` enum

```rust
pub enum ToolPayload {
    List,
    ListResult { tools: Vec<serde_json::Value> },
    Call {
        tool_name: String,
        arguments: serde_json::Value,
    },
    CallResult {
        tool_name: String,
        result: serde_json::Value,
    },
}
```

### Wire format

```
tool.list → {"method":"tool.list","params":{}}
            result: {"tools":[{"name":"...","description":"...","parameters":{...}},...]}

tool.call → {"method":"tool.call","params":{"tool_name":"x","arguments":{...}}}
            result: {"tool_name":"x","result":{"success":true,"content":"..."}}
```

### `Operation` / `Payload` enums

Add `Tool(ToolOperation)` to `Operation`, `Tool(ToolPayload)` to `Payload`.

## Backend

### `ToolHandler` (`domain/tool.rs`)

Depends on `Arc<ToolRegistry>`. Implements `DomainHandler`.

- `List`: `registry.definitions()` → `ToolPayload::ListResult`
- `Call`: builds `ToolCall` + `ToolContext`, calls `registry.execute()` → `ToolPayload::CallResult`

Registered in `AgentServerCore::build()`.

### `AgentServerCore`

Pass `tool_registry` to `ToolHandler::new()`.

### `operation_codec.rs`

Add `"tool.list"` → `ToolOperation::List`, `"tool.call"` → `ToolOperation::Call`.

### `agent_server_protocol.rs`

Add `ToolOperation` enum, `ToolPayload` enum, `from_operation` decode paths, add variants to `Operation`/`Payload`.

## Frontend

### `client.rs`

Add `tool_list()` and `tool_call()` methods (JSON-RPC raw format).

### `tools_panel.rs` / state

Update to use `tool.list` for populating the tools list and `tool.call` for invoking tools, replacing any old MCP-based or mock data.

## Files

| File | Change |
|------|--------|
| `src/agent_server_protocol.rs` | `ToolOperation`, `ToolPayload`, decode, `Operation`/`Payload` variants |
| `src/operation_codec.rs` | Method strings for tool ops |
| `src/domain/mod.rs` | Add `tool` module |
| `src/domain/tool.rs` | **New** — `ToolHandler` |
| `src/server_core.rs` | Register `ToolHandler` |
| `src/web/client.rs` | Add `tool_list()`, `tool_call()` |
| `src/web/components/tools_panel.rs` | Wire to new RPC methods |
