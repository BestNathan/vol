---
type: concept
category: framework
tags: [json-rpc, transport, connection-trait, multi-agent, vol-llm-agent-channel]
created: 2026-05-08
updated: 2026-05-09
source_count: 2
---

# JSON-RPC Transport

**Category:** Transport implementation via `Connection` trait

**Related:** [[vol-llm-agent-channel-crate]], [[connection-trait]], [[connection-holder]], [[agent-router]], [[agent-dispatcher]], [[json-rpc-websocket]], [[jsonrpc-transport-refactoring]]

## Definition

`JsonRpcConnection` in `vol-llm-agent-channel::jsonrpc::connection` implements the `Connection` trait, providing JSON-RPC 2.0 over WebSocket. It replaces the former `JsonRpcHandler`/`EventBridgePlugin` architecture that bypassed the `Connection` trait entirely [[jsonrpc-transport-refactoring]].

## Key Points

- Implements `Connection` trait: `protocol() → "jsonrpc-ws"`, `send(Message)`, `recv()` [[jsonrpc-transport-refactoring]]
- `JsonRpcServer` accepts `Vec<AgentRegistration>` at startup, builds internal `AgentRouter` [[jsonrpc-transport-refactoring]]
- All registered agents' `ConnectionHolder`s are attached at connection startup — no detach/attach switching [[jsonrpc-transport-refactoring]]
- Wire format: `{"jsonrpc":"2.0","method":"agent.event","params":{"subscription":N,"result":{"req_id":"...","event_type":"...","data":{...}}}}` [[jsonrpc-transport-refactoring]]
- 49 integration tests cover all `AgentStreamEvent` variants, all 12 JSON-RPC methods, and error handling [[task-5-jsonrpc-integration-tests]]

## Architecture

```
ReActAgent
  └── ConnectionHolder (AgentPlugin)
       listen(event) → conn.send(Message::Event)
            ↓
       JsonRpcConnection
            ├── send() → wraps in JSON-RPC envelope → WebSocket text
            ├── recv() → parse JSON-RPC → dispatch handler
            └── run() loop → attach holders, process frames, detach on exit

JsonRpcServer
    ├── new(vec![AgentRegistration { agent_id, dispatcher, holder }])
    ├── into_axum_router() → Router with /ws endpoint
    └── AgentRouter (internal) → dispatches submit/cancel to correct agent
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
WebSocket text → parse_jsonrpc_request() → dispatch handler
    → agent.submit → AgentRouter.send() → AgentDispatcher → ReActAgent
    → file.list/read → std::fs operations (handled in run() loop)
```

## Method Categories

### Agent Operations (via Connection trait)
- `agent.submit` — submits via `AgentRouter`, optional `target` param for multi-agent. Returns `{ req_id }`.
- `agent.cancel` — cancels across all registered dispatchers. Returns `{ cancelled: bool }`.
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

A single `JsonRpcConnection` attaches to ALL registered agents' `ConnectionHolder`s at startup. Events from all agents flow through the same WebSocket. The `agent.submit` method accepts an optional `target` parameter to specify which agent to submit to — falls back to first registered agent if omitted. The frontend distinguishes events by content rather than connection.

## Server Setup

```rust
let server = JsonRpcServer::new(
    vec![AgentRegistration { agent_id, dispatcher, holder }],
    working_dir, store_dir,
).await;
let app = server.into_axum_router();
axum::serve(listener, app).await;
```

Endpoint: `/ws` on the configured port.

Example: `cargo run --example jsonrpc_agent_service -p vol-llm-agent-channel`

## Connection Trait Integration

`JsonRpcConnection` is the third `Connection` implementation alongside `WsConnection` (raw WebSocket binary) and `MemoryConnection` (in-memory mpsc). The `Connection` trait unifies all transports:

| Transport | Protocol | Bidirectional | Mount Style | Use Case |
|-----------|----------|---------------|-------------|----------|
| `WsConnection` | WebSocket binary | Yes | Fixed `/ws` | Real-time, native protocol |
| `JsonRpcConnection` | JSON-RPC 2.0 text | Yes | Fixed `/ws` | Web frontend, browser-compatible |
| `HttpTransport` | HTTP POST + SSE | Request-response | `.merge()` style | Simple REST |
| `MemoryConnection` | mpsc channel | Yes | Direct handle | Testing |

## Previous Architecture (deleted)

Before the refactoring [[jsonrpc-transport-refactoring]], the `jsonrpc` module used:
- `JsonRpcHandler` + `JsonRpcContext` — separate from `Connection` trait
- `EventBridgePlugin` — duplicate of `ConnectionHolder` functionality via `broadcast::Sender`
- `jsonrpsee` crate for RPC method registration

These were replaced by `JsonRpcConnection` + `JsonRpcServer`, which plug into the existing `Connection` trait and `ConnectionHolder` systems.
