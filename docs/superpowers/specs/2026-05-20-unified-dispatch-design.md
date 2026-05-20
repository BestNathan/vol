# Unified Message Dispatch Design

## Summary

Unify `JsonRpcConnection`'s dual dispatch paths into a single
`core.handle()` call, eliminate the `JsonRpcRequest` enum and its
double-serialization, and make the `Connection` trait's `recv()`/`send()`
abstraction actually used — `AgentServerCore` gains a `serve(conn)` method
that accepts any `impl Connection`.

## Motivation

`handle_text_frame` currently has two parallel dispatch mechanisms and a
dead abstraction:

1. **Agent operations** are parsed into `JsonRpcRequest` enum variants,
   then dispatched to methods on `JsonRpcConnection` that directly access
   `core.router()` and `core.holders()` — bypassing `HandlerRegistry` entirely.

2. **Non-agent operations** are parsed into `JsonRpcRequest` variants,
   then `handle_core_dispatch()` serializes them BACK into a JSON string,
   then `decode_jsonrpc_frame()` parses that string into `AgentServerMessage`,
   then finally calls `core.handle()`.

3. The `Connection` trait defines `recv() -> Option<Result<AgentServerMessage>>`
   but `JsonRpcConnection::recv()` always returns `None`. The abstraction
   is dead code.

## Design

### Data Flow (After)

```
WebSocket frame
    ↓
spawn_reader() (background task)
    → decode_jsonrpc_frame()
    → AgentServerMessage
    → mpsc channel
    ↓
core.serve(conn):
    loop {
        conn.recv()? → core.handle(msg)? → conn.send(resp)
    }
```

### `Connection` trait

`recv()` changes from `&mut self` to `&self` so the connection can be
held in an `Arc` for concurrent `send()` (by `ConnectionHolder`) and
`recv()` (by `serve()`).

```rust
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    fn protocol(&self) -> &str;
    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>>;
    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError>;
}
```

### `JsonRpcConnection`

- Gains an internal `Mutex<tokio::sync::mpsc::Receiver<AgentServerMessage>>`.
- `spawn_reader()` — background task: read WS frames, call
  `decode_jsonrpc_frame()` to build `AgentServerMessage`, push into mpsc channel.
  On WS close or error, the channel drops naturally.
- `recv()` — pulls from the mpsc `Receiver`.
- `send()` — unchanged, sends WS text frame.
- All old methods removed: `run()`, `handle_text_frame()`,
  `handle_core_dispatch()`, `handle_submit()`, `handle_cancel()`,
  `handle_subscribe()`, `handle_unsubscribe()`, `handle_approve()`,
  `handle_agent_list()`, `process_run_result()`.

### `AgentServerCore::serve()`

```rust
impl AgentServerCore {
    pub async fn serve(&self, conn: impl Connection) {
        while let Some(result) = conn.recv().await {
            let responses = match result {
                Ok(msg) => match self.handle(msg).await {
                    Ok(resp) => resp,
                    Err(e) => vec![AgentServerMessage::new_error(
                        uuid::Uuid::new_v4().to_string(),
                        Operation::System(SystemOperation::Connected),
                        ErrorPayload {
                            code: "dispatch_error".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: false,
                        },
                    )],
                },
                Err(e) => {
                    tracing::warn!(%e, "connection receive error");
                    break;
                }
            };
            for resp in responses {
                if let Err(e) = conn.send(resp).await {
                    tracing::warn!(%e, "connection send error");
                    return;
                }
            }
        }
    }
}
```

This works with any `impl Connection` — WebSocket, HTTP SSE, memory channel,
test mocks.

### `JsonRpcServer` → simplified

`handle_ws` reduces to creating a connection and handing it to core:

```rust
async fn handle_ws(socket: WebSocket, core: Arc<AgentServerCore>) {
    let conn = JsonRpcConnection::new(socket);
    core.serve(conn).await;
}
```

`JsonRpcServer` keeps its `into_axum_router()` method, but the struct
becomes essentially an `Arc<AgentServerCore>` wrapper.

### `AgentHandler`

The agent operations (Submit, Cancel, Subscribe, Unsubscribe, Approve, List)
currently have stub implementations in `AgentHandler::handle()`. The real
logic lives in `JsonRpcConnection`'s methods (`handle_submit` etc.) which
call `core.router()` and `core.holders()`.

Move this real logic into `AgentHandler::handle()`:

- **Submit**: use `self.router.send()` (via `AgentDispatcher`) → return
  run_id and spawn background result processing.
- **Cancel**: walk dispatchers via router to cancel.
- **Subscribe/Unsubscribe**: manage a subscription set in `AgentHandler`
  (replaces `JsonRpcConnection.subscribers`).
- **Approve**: forward approval to the dispatcher.
- **List**: already uses `self.holders`, unchanged.
- **Event**: already a pass-through, unchanged.

`AgentHandler` already stores `router: AgentRouter` and `holders`. It
may need a `subscriptions: Arc<Mutex<Vec<u64>>>` field.

Note: Agent streaming events (outbound, triggered by `ConnectionHolder`)
already go through `Connection::send()`. They are not affected by this
change.

### Files Deleted or Reduced

| File | Change |
|------|--------|
| `src/jsonrpc/serde_helpers.rs` | Remove `JsonRpcRequest` enum, `parse_jsonrpc_request()`, `JsonRpcEnvelope`. Keep response/event builders. |
| `src/jsonrpc/connection.rs` | Remove all handler methods. Add `spawn_reader()`, mpsc channel. `recv()` returns real data. |
| `src/jsonrpc/server.rs` | Simplify `handle_ws` to `core.serve(conn)`. |
| `src/connection.rs` | `recv(&mut self)` → `recv(&self)`. |
| `src/server_core.rs` | Add `serve()` method. |
| `src/domain/agent.rs` | Fill in real logic for Submit/Cancel/Subscribe/Unsubscribe/Approve. |

### Files Unchanged

- `src/agent_server_protocol.rs` — protocol types unchanged.
- `src/domain/` (except agent.rs) — other 6 handlers unchanged.
- `src/domain/handler.rs`, `src/domain/registry.rs` — registry unchanged.
- `src/gateway/jsonrpc_ws.rs` — `decode_jsonrpc_frame()` / `encode_jsonrpc_message()` already correct.
- `src/router.rs`, `src/dispatcher.rs` — already support the operations AgentHandler needs.

### Error Handling

- **Parse error**: `decode_jsonrpc_frame()` returns `Err` → sends
  `-32700 Parse error` via `conn.send()`, continues loop.
- **Unknown method**: `HandlerRegistry::dispatch()` returns
  `ProtocolError::UnknownMethod` → encoded as `-32601` in response.
- **Handler error**: Handler returns `Err(ProtocolError::*)` →
  `serve()` wraps in error response message, sends via `conn.send()`.
- **Connection drop**: `conn.recv()` returns `None` or `Err` → loop exits.
