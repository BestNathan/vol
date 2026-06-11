---
type: concept
category: framework
tags: [json-rpc, transport, connection-trait, multi-agent, vol-llm-agent-channel]
created: 2026-05-08
updated: 2026-06-10
source_count: 3
---

# JSON-RPC Transport

**Category:** Generic JSON-RPC transport over the `Connection` and service abstractions

**Related:** [[vol-llm-agent-channel-crate]], [[connection-trait]], [[connection-holder]], [[agent-router]], [[agent-dispatcher]], [[json-rpc-websocket]], [[jsonrpc-transport-refactoring]], [[task-4-quality-issues-cleanup]]

## Definition

JSON-RPC transport now lives under `vol_llm_agent_channel::transport::jsonrpc::*`. `JsonRpcConnection` implements the `Connection` trait for JSON-RPC 2.0 over WebSocket, while `JsonRpcServer<S>` is generic over any `JsonRpcMessageService` and accepts both the service and an explicit WebSocket mount path. Concrete agent registration, routing, dispatch, and holder attachment are owned by `vol-agent-server` data-plane code.

## Key Points

- Implements `Connection` trait: `protocol() → "jsonrpc-ws"`, `send(Message)`, `recv()` [[jsonrpc-transport-refactoring]]
- `JsonRpcServer<S>` accepts a `JsonRpcMessageService` implementation plus configured route path, and does not own agent registration/routing [[agent-server-control-data-plane-implementation-plan]]
- Concrete `ConnectionHolder`, `AgentRouter`, and `AgentDispatcher` behavior lives in `vol-agent-server::data_plane` [[agent-server-data-plane-core-move]]
- Wire format: `{"jsonrpc":"2.0","method":"agent.event","params":{"subscription":N,"result":{"req_id":"...","event_type":"...","data":{...}}}}` [[jsonrpc-transport-refactoring]]
- 49 integration tests cover all `AgentStreamEvent` variants, all 12 JSON-RPC methods, and error handling [[task-5-jsonrpc-integration-tests]]

## Architecture

```
Client WebSocket
      ↓
JsonRpcServer<S>
    ├── new(service, configured_ws_path)
    └── into_axum_router() → Router mounted at configured path
      ↓
JsonRpcConnection
    ├── send() → wraps Message in JSON-RPC envelope → WebSocket text
    ├── recv() → parse WebSocket text → Message
    └── delegates request handling to JsonRpcMessageService
      ↓
JsonRpcMessageService implementation
    └── vol-agent-server::data_plane handles concrete agent routing/dispatch
```

## Data Flow

### Agent Events (outbound)
```
AgentStreamEvent → ConnectionHolder::listen() → Connection::send(Message::Event)
    → JsonRpcConnection deserializes to AgentStreamEvent
    → serialize_agent_event() → to_jsonrpc_event() → WebSocket text frame
```

### Client Requests (inbound)
```
WebSocket text → parse_jsonrpc_request() → JsonRpcMessageService::handle_message()
    → vol-agent-server data-plane service → concrete domain handlers/routing
```

## Method Categories

### Agent Operations (via service abstraction)
- `agent.submit` — handled by the configured `JsonRpcMessageService`; the concrete data-plane service routes optional `target` values and returns `{ req_id }`.
- `agent.cancel` — handled by the configured service and returns `{ cancelled: bool }`.
- `agent.subscribe` — adds subscription ID for event notifications. Returns `{ subscription_id }`.
- `agent.unsubscribe` — removes subscription. Returns `{ unsubscribed: bool }`.
- `agent.approve` — stub, always returns `{ approved: true }`.

### File Operations (filesystem, handled in run() loop)
- `file.list` — `std::fs::read_dir`, sorted dirs first, returns `{ entries: [{ name, is_dir, size }] }`.
- `file.read` — `std::fs::read_to_string`, returns `{ content }`.

### Log Operations (stub)
- `log.list` — returns `{ runs: [] }`.
- `log.read` — returns `{ entries: [] }`.

### Session Operations (stub)
- `session.list` — returns `{ sessions: [] }`.
- `session.resume` — returns `{ session_id, entry_count: 0 }`.

## Multi-Agent Event Model

The JSON-RPC transport is service-agnostic: events and requests pass through a single WebSocket connection, while the configured `JsonRpcMessageService` owns routing semantics. In the current application, `vol-agent-server::data_plane` routes agent methods and target selection.

## Server Setup

```rust
let server = JsonRpcServer::new(service, configured_ws_path);
let app = server.into_axum_router();
axum::serve(listener, app).await;
```

Endpoint path is supplied by the server configuration. The current `vol-agent-server` startup path uses `config.control_plane.client_ws_path`, defaulting to `/ws`.

## Connection Trait Integration

`JsonRpcConnection` is the third `Connection` implementation alongside `WsConnection` (raw WebSocket binary) and `MemoryConnection` (in-memory mpsc). The `Connection` trait unifies all transports:

| Transport | Protocol | Bidirectional | Mount Style | Use Case |
|-----------|----------|---------------|-------------|----------|
| `WsConnection` | WebSocket binary | Yes | Generic service/path | Real-time, native protocol |
| `JsonRpcConnection` | JSON-RPC 2.0 text | Yes | Configured path via `JsonRpcServer::new` | Web frontend, browser-compatible |
| `MemoryConnection` | mpsc channel | Yes | Direct handle | Testing |

## Previous Architecture (deleted)

Before the refactoring [[jsonrpc-transport-refactoring]], the `jsonrpc` module used:
- `JsonRpcHandler` + `JsonRpcContext` — separate from `Connection` trait
- `EventBridgePlugin` — duplicate of `ConnectionHolder` functionality via `broadcast::Sender`
- `jsonrpsee` crate for RPC method registration

Those pieces were first replaced by `JsonRpcConnection` plus a channel-owned server. The current boundary keeps `JsonRpcConnection` and generic `JsonRpcServer<S>` in the channel crate while concrete holder, router, dispatcher, and domain-handler behavior lives in `vol-agent-server`.
