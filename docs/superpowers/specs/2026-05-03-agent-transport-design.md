# Agent Transport Layer Design

**Date**: 2026-05-03
**Status**: Draft
**Author**: Claude Code

## Requirements

See `docs/superpowers/requirement/2026-05-03-agent-transport-requirement.md`.

TL;DR:
- `Connection` trait abstracting communication channels (WS, memory, etc.)
- `ConnectionHolder` — `AgentPlugin` wrapper registered on agent creation, forwards events to active connection
- WebSocket transport implementation using axum
- In-memory transport for local testing
- Agent and connection have independent lifecycles

## Architecture

Three layers, agent and connection are independent:

```
┌─────────────────────────────────────────────────┐
│                   Transport                      │
│  WsServer / WsConnection / MemoryConnection      │
├─────────────────────────────────────────────────┤
│                  Connection                      │
│  Connection trait + ConnectionHolder             │
│  (AgentPlugin wrapper for event forwarding)      │
├─────────────────────────────────────────────────┤
│                Agent Channel                     │
│  AgentDispatcher / AgentRouter / request types   │
└─────────────────────────────────────────────────┘
```

### Agent/Connection Relationship

```
Agent created → ConnectionHolder registered as plugin → agent lives independently
                     │
WS connect  →  holder.set(conn)  →  events stream to this connection
WS disconnect → holder.clear()  →  agent still alive, no connection
WS connect again → holder.set(new_conn) → events stream again
```

## File Structure

```
crates/vol-llm-agent-channel/
├── Cargo.toml
└── src/
    ├── connection.rs      # Connection trait + ConnectionHolder
    ├── dispatcher.rs      # AgentDispatcher (existing)
    ├── error.rs           # ChannelError (existing)
    ├── lib.rs             # Re-exports
    ├── protocol.rs        # InboundMessage, OutboundMessage
    ├── request.rs         # AgentRequest, RunResult (existing)
    ├── router.rs          # AgentRouter (existing)
    └── transport/
        ├── mod.rs         # Module root
        └── ws.rs          # WsServer + WsConnection
```

## Key Types

### `connection.rs`

```rust
/// Abstract connection for agent communication.
/// Implement for each transport protocol.
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    fn protocol(&self) -> &str;
    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>>;
    async fn send_event(&self, event: &AgentStreamEvent) -> Result<(), ConnectionError>;
    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError>;
}
```

```rust
/// Registered as AgentPlugin on agent creation.
/// Holds at most one active connection at a time.
/// Agent and connection have independent lifecycles.
pub struct ConnectionHolder {
    connection: Arc<RwLock<Option<Arc<dyn Connection>>>>,
}

impl ConnectionHolder {
    pub fn new() -> Self;

    /// Attach a connection. Disconnect existing one first.
    pub async fn attach(&self, conn: Arc<dyn Connection>);

    /// Detach current connection (if any).
    pub async fn detach(&self);

    /// Whether a connection is currently active.
    pub async fn is_connected(&self) -> bool;
}

#[async_trait]
impl AgentPlugin for ConnectionHolder {
    fn id(&self) -> PluginId { "connection_holder".to_string() }
    fn priority(&self) -> u32 { 50 }

    async fn listen(&self, event: &AgentStreamEvent, _ctx: &RunContext) {
        if let Some(conn) = self.connection.read().await.as_ref() {
            let _ = conn.send_event(event).await;
        }
    }
}
```

### `protocol.rs`

```rust
/// Messages received from client (inbound).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum InboundMessage {
    #[serde(rename = "submit")]
    Submit {
        req_id: String,
        target_id: String,
        input: String,
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    #[serde(rename = "cancel")]
    Cancel {
        req_id: String,
    },
}

/// Messages sent to client (outbound).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OutboundMessage {
    #[serde(rename = "connected")]
    Connected {
        agent_id: String,
    },
    #[serde(rename = "event")]
    Event {
        event: AgentStreamEvent,
    },
    #[serde(rename = "result")]
    Result {
        result: RunResult,
    },
    #[serde(rename = "error")]
    Error {
        req_id: Option<String>,
        message: String,
    },
}
```

### `transport/ws.rs`

```rust
/// WebSocket connection implementation of Connection trait.
pub struct WsConnection {
    tx: WebSocketSender,  // tokio_tungstenite::WebSocketStream sender
    rx: Mutex<WebSocketReceiver>,
}

impl WsConnection {
    pub fn new(stream: WebSocketStream<Upgraded>) -> Self;
}

#[async_trait]
impl Connection for WsConnection {
    fn protocol(&self) -> &str { "ws" }
    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>>;
    async fn send_event(&self, event: &AgentStreamEvent) -> Result<(), ConnectionError>;
    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError>;
}
```

```rust
/// WebSocket server that serves WS connections.
/// Created once, handles multiple WS connections.
pub struct WsServer {
    router: Arc<AgentRouter>,
}

impl WsServer {
    pub fn new(router: Arc<AgentRouter>) -> Self;

    /// Create an axum router with WS endpoint.
    pub fn into_axum_router(self) -> Router;
}
```

### `transport/memory.rs`

```rust
/// In-memory connection for local testing.
pub struct MemoryConnection {
    rx: Mutex<mpsc::Receiver<InboundMessage>>,
    tx: mpsc::Sender<OutboundMessage>,
}

impl MemoryConnection {
    /// Create a pair: the connection + a channel for the test to send/recv.
    pub fn new() -> (Self, MemoryHandle);
}

impl Connection for MemoryConnection { ... }

/// Test handle for controlling the connection from tests.
pub struct MemoryHandle {
    tx: mpsc::Sender<InboundMessage>,
    rx: mpsc::Receiver<OutboundMessage>,
}
```

## Data Flow

### Submit → Execute → Result

```
Client sends:  { "type": "submit", "req_id": "r1", "target_id": "agent_a", "input": "hello" }
     │
     ▼
WsConnection.recv() → InboundMessage::Submit
     │
     ▼
dispatcher.submit(AgentRequest { req_id: "r1", target_id: "agent_a", input: "hello" })
     │
     ├── oneshot::Receiver returned immediately
     │
     ▼
agent.run("hello")  ← ConnectionHolder.listen() receives events during run
     │
     │   During run:
     │   agent emits: AgentStart → holder.listen() → conn.send_event() → WS client
     │   agent emits: ThinkingDelta → holder.listen() → conn.send_event() → WS client
     │   agent emits: ToolCallBegin → holder.listen() → conn.send_event() → WS client
     │   ...
     │
     ▼
run() returns AgentResponse
     │
     ▼
oneshot::Receiver resolves → RunResult
     │
     ▼
WsConnection.send_result(RunResult) → WS client receives:
     { "type": "result", "result": { "req_id": "r1", "response": { ... } } }
```

### Cancel

```
Client sends:  { "type": "cancel", "req_id": "r1" }
     │
     ▼
WsConnection.recv() → InboundMessage::Cancel
     │
     ▼
dispatcher.cancel("r1") → removes from queue, closes oneshot
     │
     ▼
Client receives: { "type": "error", "req_id": "r1", "message": "request cancelled" }
```

### Connection Attach/Detach

```
Agent created → ConnectionHolder registered as plugin → agent lives independently
     │
WS connect → holder.attach(ws_conn) → events forward to this WS
     │
agent.run() → events stream to WS client in real-time
     │
WS disconnect → holder.detach() → agent still alive, no active connection
     │
WS connect again → holder.attach(new_ws_conn) → events forward again
```

## Edge Cases

| Edge Case | Behavior |
|-----------|----------|
| Client sends message while agent is busy | Request queued, client continues to receive events |
| Client disconnects during agent execution | `send_event()` returns error, silently ignored |
| Client sends invalid JSON | Send `{ "type": "error", "message": "..." }` back |
| Client sends request for unknown agent | Send error back |
| Multiple WS connections to same agent | Not supported — `attach()` replaces existing connection |
| `attach()` called while connection active | `detach()` current connection first |

## Testing Strategy

- Unit tests for `ConnectionHolder` (attach, detach, is_connected)
- Unit tests for `InboundMessage`/`OutboundMessage` serialization
- Integration test with `MemoryConnection`: submit request, verify result
- Integration test with `MemoryConnection`: cancel request, verify not executed
- Integration test: agent runs, `MemoryConnection` receives streaming events
