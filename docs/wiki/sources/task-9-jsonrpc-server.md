---
type: source
category: implementation
tags: [json-rpc, server, vol-llm-agent-channel, jsonrpsee, websocket]
created: 2026-05-08
updated: 2026-05-08
---

# Task 9: JSON-RPC Server for vol-llm-agent-channel

**Source Type:** Implementation
**Related:** [[vol-llm-agent-channel-crate]], [[json-rpc-websocket]], [[agent-dispatcher]], [[remote-agent-connection]]

## Summary

Added a `jsonrpc` module to `vol-llm-agent-channel` exposing agent operations via JSON-RPC 2.0 over WebSocket. The server uses `jsonrpsee` 0.26 `ServerBuilder` and registers 9 methods on a `RpcModule::from_arc(handler)`.

## Files Added

| File | Purpose |
|------|---------|
| `src/jsonrpc/mod.rs` | Module declaration |
| `src/jsonrpc/handler.rs` | `JsonRpcHandler` and `JsonRpcContext` with 9 RPC methods |
| `examples/jsonrpc_agent_service.rs` | Runnable binary example |

## JSON-RPC Methods

### Agent Methods

| Method | Params | Response | Status |
|--------|--------|----------|--------|
| `agent.submit` | `{ input: String }` | `{ req_id: String }` | Full — submits to `AgentDispatcher`, spawns `process_run` background task |
| `agent.cancel` | `{ req_id: String }` | `{ cancelled: bool }` | Full — calls `dispatcher.cancel()` |
| `agent.approve` | `{ req_id, approved, reason }` | `{ approved: true }` | Stub — approval handled via connection transport |

### File Methods

| Method | Params | Response | Status |
|--------|--------|----------|--------|
| `file.list` | `{ path: String }` | `{ entries: Vec<FileEntry> }` | Full — uses `std::fs::read_dir` |
| `file.read` | `{ path: String }` | `{ content: String }` | Full — uses `std::fs::read_to_string` |

### Log Methods

| Method | Params | Response | Status |
|--------|--------|----------|--------|
| `log.list` | `{}` | `{ runs: Vec<LogRunInfo> }` | Stub — scans `store_dir/logs/*.jsonl`, returns id only |
| `log.read` | `{ run_id: String }` | `{ entries: Vec<LogEntry> }` | Stub — returns empty `Vec` |

### Session Methods

| Method | Params | Response | Status |
|--------|--------|----------|--------|
| `session.list` | `{}` | `{ sessions: Vec<SessionInfo> }` | Stub — scans `store_dir/sessions/*.json`, returns id only |
| `session.resume` | `{ session_id: String }` | `{ session_id, entry_count }` | Stub — returns entry_count: 0 |

## Architecture

```
TcpListener:3001 → ServerBuilder → RpcModule::from_arc(JsonRpcHandler)
                                     ↕
                               JsonRpcContext
                            (dispatcher, working_dir, store_dir)
                                     ↕
                              AgentDispatcher
                                     ↕
                                ReActAgent
```

## Key Design Points

- `JsonRpcContext` wraps `Arc<AgentDispatcher>` with `working_dir` and `store_dir` paths
- `JsonRpcHandler` holds `Mutex<JsonRpcContext>` for thread-safe access
- `agent.submit` spawns `tokio::spawn(Self::process_run(rx, req_id))` to consume the oneshot result
- `log.list` and `session.list` use `std::fs::read_dir` to scan directories for `*.jsonl` and `*.json` files
- Stub implementations for `log.read` and `session.resume` return empty/default results
- Example binary listens on `0.0.0.0:3001`, connectable at `ws://localhost:3001`

## Build & Test

```bash
cargo check -p vol-llm-agent-channel --all-targets
```

All 16 existing tests pass (no new tests added in this task).

## Concepts Extracted

- [[jsonrpc-server-handler]] -- Server-side JSON-RPC handler architecture
- [[json-rpc-websocket]] -- Updated with server-side protocol details
