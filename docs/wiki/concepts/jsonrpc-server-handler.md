---
type: concept
category: framework
tags: [json-rpc, server, handler, jsonrpsee, agent, vol-llm-agent-channel]
created: 2026-05-08
updated: 2026-05-08
source_count: 1
---

# JSON-RPC Server Handler

**Category:** Server-side JSON-RPC architecture

**Related:** [[vol-llm-agent-channel-crate]], [[agent-dispatcher]], [[json-rpc-websocket]], [[remote-agent-connection]], [[task-9-jsonrpc-server]]

## Definition

`JsonRpcHandler` and `JsonRpcContext` in `vol-llm-agent-channel::jsonrpc::handler` provide a server-side JSON-RPC 2.0 interface for agent operations, file access, log browsing, and session management.

## Key Points
- `JsonRpcContext` holds `Arc<AgentDispatcher>`, `working_dir: String`, and `store_dir: String` [[task-9-jsonrpc-server]]
- `JsonRpcHandler` wraps `JsonRpcContext` in a `Mutex` for thread-safe concurrent access [[task-9-jsonrpc-server]]
- 9 JSON-RPC methods registered on `RpcModule::from_arc(handler)` via `jsonrpsee` 0.26 `ServerBuilder` [[task-9-jsonrpc-server]]
- Methods grouped into agent (`submit`, `cancel`, `approve`), file (`list`, `read`), log (`list`, `read`), and session (`list`, `resume`) [[task-9-jsonrpc-server]]

## Architecture

```
Client (ws://localhost:3001)
    ↕ JSON-RPC 2.0 over WebSocket
ServerBuilder::default().build("0.0.0.0:3001")
    ↕ RpcModule::from_arc(JsonRpcHandler)
JsonRpcHandler { ctx: Mutex<JsonRpcContext> }
    ↕
JsonRpcContext { dispatcher, working_dir, store_dir }
    ↕
AgentDispatcher → ReActAgent
```

## Method Categories

### Agent Operations (core)
- `agent.submit` -- full: submits to `AgentDispatcher`, spawns background `process_run` task
- `agent.cancel` -- full: calls `dispatcher.cancel(req_id)`
- `agent.approve` -- stub: approval handled via connection transport layer, not JSON-RPC

### File Operations (filesystem)
- `file.list` -- full: uses `std::fs::read_dir`, sorts dirs first, returns `FileEntry { name, is_dir, size }`
- `file.read` -- full: uses `std::fs::read_to_string`, returns raw content

### Log Operations (stub)
- `log.list` -- stub: scans `store_dir/logs/*.jsonl`, returns `LogRunInfo { id, timestamp: "unknown", count: 0 }`
- `log.read` -- stub: returns empty `Vec<LogEntry>`

### Session Operations (stub)
- `session.list` -- stub: scans `store_dir/sessions/*.json`, returns `SessionInfo { id, entry_count: 0, created_at: "unknown" }`
- `session.resume` -- stub: returns `{ session_id, entry_count: 0 }`

## Stub vs Full

Full implementations use real `std::fs` operations and integrate with `AgentDispatcher`. Stub implementations return empty results or scan directory names without parsing content. The stub status reflects that log reading and session resumption need `vol-session` integration to read actual stored data.

## Server Setup

```rust
let server = ServerBuilder::default()
    .build("0.0.0.0:3001")
    .await?;
let mut module = RpcModule::from_arc(handler);
module.register_async_method("agent.submit", ...);
// ... register 8 more methods
server.start(module);
```

Example binary: `cargo run --example jsonrpc_agent_service -p vol-llm-agent-channel`

## Thread Safety

`JsonRpcHandler` uses `Mutex<JsonRpcContext>` (tokio sync mutex) because multiple WebSocket clients may call methods concurrently. The `AgentDispatcher` itself is `Arc`-wrapped and thread-safe internally.
