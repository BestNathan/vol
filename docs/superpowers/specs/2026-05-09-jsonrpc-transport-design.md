# Design: JSON-RPC as Connection Trait Implementation

## Background

The current codebase has two parallel event-bridging mechanisms:
- **ConnectionHolder** (`AgentPlugin`) forwards agent events through the `Connection` trait
- **EventBridgePlugin** (`AgentPlugin`) forwards agent events through a separate `broadcast::Sender`

The JSON-RPC handler (`jsonrpc/handler.rs`) maintains its own `JsonRpcHandler`, `JsonRpcContext`, `EventBridgePlugin`, and subscription mechanism — completely bypassing the `Connection` trait that `WsConnection` and `MemoryConnection` already use. This creates duplicate event paths, redundant plugin registrations, and code that cannot be reused by other transports.

## Problem Statement

1. **Not reusable**: JSON-RPC logic is tightly coupled to `JsonRpcHandler`. Adding JSON-RPC support to multi-agent or other contexts requires duplicating the handler, plugin, and subscription setup.
2. **Too redundant**: `EventBridgePlugin` does what `ConnectionHolder` already does — listen to agent events and forward them. The only difference is the output format (JSON-RPC envelope vs. raw `Message`).
3. **Event duplication**: Two plugins each sending events led to duplicate messages in the UI (already fixed as a band-aid, but the architectural root cause remains).

## Goals

1. `JsonRpcConnection` implements the `Connection` trait, plugging into the existing `ConnectionHolder` plugin
2. `ConnectionHolder` becomes the **single** event bridge — `EventBridgePlugin` is deleted
3. JSON-RPC wire format is preserved — web frontend sees no change
4. File/session/log operations remain available as JSON-RPC extensions, handled within the connection's `run()` loop
5. Multi-agent can reuse `JsonRpcConnection` with different sender/receiver IDs

## Non-Goals

- Changing the JSON-RPC wire protocol (frontend must not need updates)
- Changing the raw WebSocket transport (`WsConnection` continues as-is)
- Adding new JSON-RPC methods beyond what already exists
- Modifying the `Message` enum or `Connection` trait interface (they are already sufficient)

## Architecture

### Overview

```
┌────────────────────────────────────────────────────────┐
│                     ReActAgent                          │
│                                                         │
│  ┌──────────────────────────────────────────────────┐  │
│  │         ConnectionHolder (AgentPlugin)            │  │
│  │  listen(event) → conn.send(Message::Event)       │  │
│  └──────────┬───────────┬────────────┬──────────────┘  │
│             │           │            │                 │
│             ▼           ▼            ▼                 │
│     WsConnection   JsonRpcConnection  MemoryConnection │
│     (raw Message)  (JSON-RPC 2.0)    (mpsc)           │
│         │              │              │                │
│         ▼              ▼              ▼                │
│     WS binary      WS JSON-RPC    test/inline          │
└────────────────────────────────────────────────────────┘
```

### Core Components

#### `JsonRpcConnection` (new file: `src/jsonrpc/connection.rs`)

Implements `Connection`. Wraps a WebSocket, translates between `Message` and JSON-RPC wire format.

**Struct fields:**
```rust
pub struct JsonRpcConnection {
    ws_tx: Arc<Mutex<SplitSink<WebSocket, WsMessage>>>,
    ws_rx: SplitStream<WebSocket>,
    holder: Arc<ConnectionHolder>,
    dispatcher: Arc<AgentDispatcher>,
    agent_id: String,
    subscribers: Vec<u64>,  // JSON-RPC subscription IDs
    next_sub_id: u64,
}
```

**`Connection` trait:**
- `protocol()` → `"jsonrpc-ws"`
- `send(Message)`:
  - `Message::Event { event, .. }` → wraps in JSON-RPC subscription format:
    `{"jsonrpc":"2.0","method":"agent.event","params":{"subscription":N,"result":{"req_id":"...","event_type":"...","data":{...}}}}`
  - `Message::Result { req_id, result, .. }` → JSON-RPC response to the `agent.submit` call. Uses stored request IDs to match responses.
  - `Message::Error { req_id, message, .. }` → JSON-RPC error response
  - `Message::Connected { .. }` → plain JSON-RPC response or raw send
- `recv()`:
  - Parses incoming JSON-RPC text
  - `agent.submit` → returns `Some(Ok(Message::Submit { ... }))`
  - `agent.cancel` → returns `Some(Ok(Message::Cancel { ... }))`
  - `agent.subscribe` → internal, registers subscriber, does NOT return a `Message`
  - `agent.unsubscribe` → internal
  - file.*, session.*, log.* → internal, does NOT return a `Message`

**`run()` loop:**
The connection loop that owns the WebSocket. Handles both `Connection`-level messaging and JSON-RPC-extended methods:

1. On connect: send `Message::Connected`, attach to holder
2. Loop: read WebSocket text frame
   - Parse as JSON-RPC request
   - If `agent.submit` or `agent.cancel`: use `Connection.recv()` path → the dispatcher processes it via the `Message` system
   - If `agent.subscribe`/`unsubscribe`: manage subscriber list internally
   - If `file.list`, `file.read`, `session.list`, `session.resume`, `log.list`, `log.read`: handle internally (file I/O, no agent involvement)
   - If `agent.approve`: return stub response
3. On disconnect: detach from holder

**JSON-RPC serialization helpers:**
```rust
fn to_jsonrpc_event(event: &AgentStreamEvent, sub_id: u64, req_id: &str) -> String;
fn from_jsonrpc_request(text: &str) -> Result<JsonRpcRequest, ParseError>;
```

The `serialize_agent_event` function from the current `handler.rs` moves here.

#### `JsonRpcServer` (new file: `src/jsonrpc/server.rs`)

Like `WsServer`. Manages JSON-RPC connections, provides axum router.

```rust
pub struct JsonRpcServer {
    dispatcher: Arc<AgentDispatcher>,
    holder: Arc<ConnectionHolder>,
    agent_id: String,
    working_dir: String,
    store_dir: String,
}

impl JsonRpcServer {
    pub fn new(dispatcher, holder, agent_id, working_dir, store_dir) -> Self;
    pub fn into_axum_router(self) -> Router;
}
```

The WebSocket upgrade handler creates a `JsonRpcConnection`, attaches it to the holder, and runs the connection loop.

#### `EventBridgePlugin` — deleted

No longer needed. `ConnectionHolder` already does this job via the `Connection` trait. The `broadcast::Sender<AgentEvent>` field on `JsonRpcContext` is removed. The `current_req_id` mutex is removed.

#### `JsonRpcHandler` — deleted

Replaced by `JsonRpcServer` + `JsonRpcConnection`. All methods (agent_submit, agent_cancel, file_list, file_read, etc.) move into `JsonRpcConnection.run()`.

### Data Flow

#### Agent Events (outbound)

```
Agent emits AgentStreamEvent
  → ConnectionHolder.listen() serializes to serde_json::Value
  → ConnectionHolder calls conn.send(Message::Event { event: value, .. })
  → JsonRpcConnection.send() wraps in JSON-RPC subscription envelope
  → WebSocket sends text frame
  → Frontend receives identical JSON as before
```

#### Client Requests (inbound)

```
Frontend sends JSON-RPC request
  → JsonRpcConnection.recv() parses JSON-RPC
  → If agent.submit → converts to Message::Submit
  → ConnectionHolder/Dispatcher processes the Message
  → Response comes back through Connection.send()
```

#### File/Session Operations (inbound, JSON-RPC-only)

```
Frontend sends file.list JSON-RPC request
  → JsonRpcConnection.recv() recognizes it's not a Message
  → Handles internally in run() loop (std::fs operations)
  → Sends JSON-RPC response directly via ws_tx
```

### Error Handling

- **Parse errors**: Invalid JSON-RPC → send JSON-RPC error response `{"jsonrpc":"2.0","error":{"code":-32700,"message":"Parse error"}}`
- **Unknown methods**: → JSON-RPC error `{"code":-32601,"message":"Method not found"}`
- **Connection errors**: WebSocket disconnect → `holder.detach()`, loop exits
- **Agent errors**: `Message::Error` returned through JSON-RPC error response
- **File I/O errors**: Return as JSON-RPC error response with the OS error message

### Edge Cases

1. **Multiple subscribers**: JSON-RPC supports multiple subscriptions on one connection. `subscribers` vec tracks them. Events are sent once per connection (not per subscriber ID) — the sub ID is just included in the params.
2. **Empty req_id**: Current `EventBridgePlugin` skips events when `req_id` is empty. `JsonRpcConnection` tracks the current request ID during submit processing, same behavior.
3. **Concurrent submits**: The dispatcher already handles FIFO. JSON-RPC just adds request ID correlation.
4. **WebSocket ping/pong**: Handled in `recv()` by skipping to next frame (same as `WsConnection`).

## File Change Summary

| File | Action | Responsibility |
|------|--------|----------------|
| `src/jsonrpc/connection.rs` | **Create** | `JsonRpcConnection` struct, `Connection` impl, `run()` loop, JSON-RPC serialization |
| `src/jsonrpc/server.rs` | **Create** | `JsonRpcServer` struct, axum router, WS upgrade handler |
| `src/jsonrpc/handler.rs` | **Delete** | Replaced by connection.rs + server.rs |
| `src/jsonrpc/mod.rs` | **Modify** | Export new modules, remove old |
| `src/lib.rs` | **Modify** | Update public exports if needed |
| `src/transport/ws.rs` | **No change** | Continues working as-is |
| `src/connection.rs` | **No change** | Trait and holder unchanged |
| `vol-llm-ui/src/bin/web_server.rs` (or equivalent) | **Modify** | Use `JsonRpcServer` instead of current setup |
| `vol-llm-ui/src/web/client.rs` | **No change** | Frontend unchanged |

## Success Criteria

1. `EventBridgePlugin` is deleted, no regression in event delivery
2. `JsonRpcConnection` passes all existing JSON-RPC integration tests
3. Web frontend receives identical events with no code changes
4. `cargo test` passes for vol-llm-agent-channel
5. `file.list`, `file.read`, `session.list`, etc. still work via JSON-RPC
