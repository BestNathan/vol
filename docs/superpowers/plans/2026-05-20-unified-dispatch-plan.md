# Unified Message Dispatch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify dual dispatch paths into single `core.handle()` call through `core.serve(conn)`, eliminate `JsonRpcRequest` enum and double-serialization, make `Connection` trait `recv()`/`send()` actually used.

**Architecture:** `JsonRpcConnection` gains mpsc channel + `spawn_reader()` background task that parses WS frames to `AgentServerMessage` and pushes into channel. `AgentServerCore::serve(conn)` loops `recv() → handle() → send()`. All old handler methods, `JsonRpcRequest` enum, and `parse_jsonrpc_request()` are deleted. Agent logic moves into `AgentHandler`.

**Tech Stack:** Rust, tokio, async_trait, axum WebSocket, existing `AgentServerMessage`/`Operation` types.

---

### Task 1: Change `Connection::recv()` signature

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/connection.rs`

- [ ] **Step 1: Change `recv(&mut self)` to `recv(&self)`**

Replace line 20:
```rust
    async fn recv(&mut self) -> Option<Result<AgentServerMessage, ConnectionError>>;
```
With:
```rust
    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>>;
```

- [ ] **Step 2: Update MockConnection in tests**

In the test module (around line 108), change:
```rust
        async fn recv(&mut self) -> Option<Result<AgentServerMessage, ConnectionError>> { None }
```
To:
```rust
        async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> { None }
```

- [ ] **Step 3: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: FAIL — `JsonRpcConnection::recv()` still uses `&mut self`. That's expected, we fix it in Task 2.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/connection.rs
git commit -m "refactor: change Connection::recv signature to &self"
```

---

### Task 2: Rewrite `JsonRpcConnection` — add channel, reader, real recv

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

This is the core task. Replace the entire handler architecture with a channel-based one.

- [ ] **Step 1: Read current file to understand structure**

The file is 419 lines. We will:
1. Add mpsc channel fields
2. Add `spawn_reader()` method
3. Replace `recv()` stub with real implementation
4. Delete all handler methods

- [ ] **Step 2: Update imports**

Replace the import block (lines 7-20):

OLD:
```rust
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::sink::SinkExt;
use futures::stream::StreamExt;

use crate::agent_server_protocol::{AgentOperation, AgentPayload, AgentServerMessage, ErrorPayload, MessageKind, Operation, Payload};
use crate::connection::Connection;
use crate::error::ConnectionError;
use crate::request::AgentRequest;
use crate::server_core::AgentServerCore;

use super::serde_helpers::{parse_jsonrpc_request, to_jsonrpc_error, to_jsonrpc_event, to_jsonrpc_response, JsonRpcRequest};
```

NEW:
```rust
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::ws::{Message as WsMessage, WebSocket};
use futures::stream::StreamExt;

use crate::agent_server_protocol::{AgentServerMessage, ErrorPayload, MessageKind, Operation};
use crate::connection::Connection;
use crate::error::ConnectionError;

use super::serde_helpers::{to_jsonrpc_error, to_jsonrpc_event};
```

- [ ] **Step 3: Update struct fields**

Replace the struct (lines 22-34):

OLD:
```rust
pub struct JsonRpcConnection {
    /// WebSocket text sender (mutex-wrapped for concurrent sends).
    ws_tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    /// WebSocket text receiver.
    ws_rx: Arc<tokio::sync::Mutex<futures::stream::SplitStream<WebSocket>>>,
    /// Shared core — single source of truth for all resources.
    core: Arc<AgentServerCore>,
    /// Active subscription IDs.
    subscribers: Arc<tokio::sync::Mutex<Vec<u64>>>,
    /// Next subscription ID counter.
    next_sub_id: std::sync::atomic::AtomicU64,
}
```

NEW:
```rust
pub struct JsonRpcConnection {
    /// WebSocket text sender (mutex-wrapped for concurrent sends).
    ws_tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    /// Receive end of the mpsc channel — populated by `spawn_reader()`.
    msg_rx: tokio::sync::Mutex<tokio::sync::mpsc::Receiver<Result<AgentServerMessage, ConnectionError>>>,
}
```

- [ ] **Step 4: Rewrite constructor**

Replace `new()` (lines 36-47):

```rust
impl JsonRpcConnection {
    /// Create a new `JsonRpcConnection` and start the background reader.
    pub fn new(ws: WebSocket) -> Self {
        let (ws_tx, ws_rx) = ws.split();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<AgentServerMessage, ConnectionError>>(64);

        let conn = Self {
            ws_tx: Arc::new(tokio::sync::Mutex::new(ws_tx)),
            msg_rx: tokio::sync::Mutex::new(rx),
        };

        // Spawn background reader task.
        tokio::spawn(Self::reader(tx, ws_rx));

        conn
    }

    /// Background task: read WS frames, decode to AgentServerMessage, push into channel.
    async fn reader(
        tx: tokio::sync::mpsc::Sender<Result<AgentServerMessage, ConnectionError>>,
        ws_rx: futures::stream::SplitStream<WebSocket>,
    ) {
        use futures::sink::SinkExt;
        let mut ws_rx = ws_rx;
        loop {
            let msg = ws_rx.next().await;
            let Some(msg) = msg else {
                tracing::info!("WebSocket connection closed");
                break;
            };
            match msg {
                Ok(WsMessage::Text(text)) => {
                    match crate::gateway::jsonrpc_ws::decode_jsonrpc_frame(&text) {
                        Ok(agent_msg) => {
                            if tx.send(Ok(agent_msg)).await.is_err() {
                                break; // Server dropped receiver, stop
                            }
                        }
                        Err(e) => {
                            let _ = tx.send(Err(e)).await;
                        }
                    }
                }
                Ok(WsMessage::Close(_)) => {
                    tracing::info!("WebSocket close received");
                    break;
                }
                Ok(WsMessage::Ping(_)) => {
                    // Pong is auto-replied by axum
                }
                Ok(WsMessage::Pong(_)) => {}
                Ok(WsMessage::Binary(_)) => {
                    tracing::debug!("Ignoring binary message");
                }
                Err(e) => {
                    tracing::warn!(%e, "WebSocket receive error");
                    break;
                }
            }
        }
    }
```

- [ ] **Step 5: Delete all old methods**

Delete the following methods from `impl JsonRpcConnection`:
- `run()` (lines 50-120)
- `handle_text_frame()` (lines 122-235)
- `handle_core_dispatch()` (lines 237-262)
- `handle_submit()` (lines 266-289)
- `handle_cancel()` (lines 291-295)
- `handle_subscribe()` (lines 297-301)
- `handle_unsubscribe()` (lines 303-307)
- `handle_approve()` (lines 309-311)
- `handle_agent_list()` (lines 313-318)
- `process_run_result()` (lines 320-340)

Keep only `send_ws_text()` (renamed to be used by `send()`).

- [ ] **Step 6: Replace `Connection` impl**

OLD (lines 351-396 — the `impl Connection` block with stubs):
```rust
#[async_trait]
impl Connection for JsonRpcConnection {
    fn protocol(&self) -> &str {
        "jsonrpc-ws"
    }

    async fn recv(&mut self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        None
    }

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
        // ... long match statement for events/errors ...
    }
}
```

NEW `recv()` — replace the stub with real channel read:

```rust
    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        self.msg_rx.lock().await.recv().await
    }
```

NEW `send()` — simplified, with `send_ws_text` inlined:

```rust
    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
        match (&msg.kind, &msg.operation, &msg.payload) {
            (MessageKind::Event, Operation::Agent(AgentOperation::Event), Payload::Agent(AgentPayload::Event { event, .. })) => {
                match serde_json::from_value::<vol_llm_agent::react::AgentStreamEvent>(event.clone()) {
                    Ok(agent_event) => {
                        let sub_id = 0u64;
                        let text = to_jsonrpc_event(&agent_event, sub_id, "");
                        let mut tx = self.ws_tx.lock().await;
                        tx.send(WsMessage::Text(text))
                            .await
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
                    }
                    Err(e) => {
                        tracing::error!(%e, ?event, "failed to deserialize AgentStreamEvent in send");
                        let envelope = serde_json::json!({
                            "jsonrpc": "2.0",
                            "method": "agent.event",
                            "params": {
                                "subscription": 0,
                                "result": {
                                    "event_type": "unknown",
                                    "data": event,
                                },
                            },
                        });
                        let text = serde_json::to_string(&envelope)
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
                        let mut tx = self.ws_tx.lock().await;
                        tx.send(WsMessage::Text(text))
                            .await
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
                    }
                }
            }
            (MessageKind::Error, _, Payload::Error(ErrorPayload { message, .. })) => {
                let text = to_jsonrpc_error(None, -32000, message.clone());
                let mut tx = self.ws_tx.lock().await;
                tx.send(WsMessage::Text(text))
                    .await
                    .map_err(|e| ConnectionError::WsSendError(e.to_string()))
            }
            _ => {
                let text = crate::gateway::jsonrpc_ws::encode_jsonrpc_message(msg)
                    .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
                let mut tx = self.ws_tx.lock().await;
                tx.send(WsMessage::Text(text))
                    .await
                    .map_err(|e| ConnectionError::WsSendError(e.to_string()))
            }
        }
    }
```

Note: The `send()` method uses the `gateway` codec's `encode_jsonrpc_message()` for all non-event/error responses. This eliminates the separate `serialize_agent_event` path in `send()` for the standard result path. We also need to add back the necessary imports: `AgentOperation`, `AgentPayload`, `Payload` from `agent_server_protocol`.

- [ ] **Step 7: Update the test module**

The test at the bottom of the file (`test_jsonrpc_event_format`) tests `to_jsonrpc_event` which is still exported from `serde_helpers`. Keep that test. Change the import to use `use crate::jsonrpc::serde_helpers::to_jsonrpc_event;` instead of `use super::*`.

- [ ] **Step 8: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: FAIL — `JsonRpcServer::handle_ws` still calls `JsonRpcConnection::new(ws, core)` with two args. That's Task 4.

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/connection.rs
git commit -m "refactor: rewrite JsonRpcConnection with mpsc channel and spawn_reader"
```

---

### Task 3: Delete `JsonRpcRequest` and `parse_jsonrpc_request()`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs`
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/mod.rs` (if needed)

- [ ] **Step 1: Read serde_helpers.rs to identify what to keep**

Keep: `serialize_agent_event()`, `to_jsonrpc_event()`, `to_jsonrpc_response()`, `to_jsonrpc_error()`.
Delete: `JsonRpcEnvelope` struct, `JsonRpcRequest` enum, `parse_jsonrpc_request()`.

- [ ] **Step 2: Delete lines 16-569 — everything from `JsonRpcEnvelope` through the end of `parse_jsonrpc_request`**

Specifically:
- Remove `JsonRpcEnvelope` struct (lines 16-22)
- Remove `JsonRpcRequest` enum (lines 30-134)
- Remove `parse_jsonrpc_request()` function (lines 376-570)

Keep the imports (`use serde::Deserialize;`, `use vol_llm_agent::react::AgentStreamEvent;`).

Remove unused imports: `serde::Deserialize` was only used by `JsonRpcEnvelope` — remove if clippy says so. (Actually, `serialize_agent_event` uses `serde_json::Value` which doesn't need `Deserialize`. Remove the `use serde::Deserialize;`.)

- [ ] **Step 3: Remove `Deserialize` import if present**

The import `use serde::Deserialize;` on line 9 was only used by `JsonRpcEnvelope`. Remove it.

- [ ] **Step 4: Update the test module at the bottom of the file**

The tests for `parse_skill_list`, `parse_skill_get`, `parse_skill_get_missing_name` all test `parse_jsonrpc_request()` — remove these tests (they test deleted code).

Keep the tests that test `to_jsonrpc_event`, `to_jsonrpc_response`, `to_jsonrpc_error` if any. Actually, the current test module only has `parse_*` tests. All 3 tests are for `parse_jsonrpc_request`. Remove the entire test module.

- [ ] **Step 5: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS (the only remaining usage of `parse_jsonrpc_request` was in `connection.rs`, already deleted in Task 2).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs
git commit -m "refactor: remove JsonRpcRequest enum and parse_jsonrpc_request"
```

---

### Task 4: Simplify `JsonRpcServer` and add `core.serve()`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/server.rs`
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`

- [ ] **Step 1: Add `serve()` to `AgentServerCore`**

In `server_core.rs`, add the `serve()` method to `impl AgentServerCore`. Place it after the `handle()` method:

```rust
    /// Serve incoming messages from a connection, dispatching each to the handler registry.
    ///
    /// Loops `recv() → handle() → send()` until the connection closes or errors.
    pub async fn serve(&self, conn: impl crate::connection::Connection) {
        while let Some(result) = conn.recv().await {
            let responses = match result {
                Ok(msg) => match self.handle(msg).await {
                    Ok(resp) => resp,
                    Err(e) => vec![crate::agent_server_protocol::AgentServerMessage::new_error(
                        uuid::Uuid::new_v4().to_string(),
                        crate::agent_server_protocol::Operation::System(
                            crate::agent_server_protocol::SystemOperation::Connected,
                        ),
                        crate::agent_server_protocol::ErrorPayload {
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
```

- [ ] **Step 2: Simplify `JsonRpcServer` and `handle_ws`**

Replace the entire `server.rs` with:

```rust
//! JSON-RPC server managing multiple agent connections on a single WebSocket endpoint.

use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::routing::get;

use crate::server_core::AgentServerCore;

use super::connection::JsonRpcConnection;

/// JSON-RPC server providing a `/ws` WebSocket endpoint.
pub struct JsonRpcServer {
    core: Arc<AgentServerCore>,
}

impl JsonRpcServer {
    /// Create a new server wrapping the given core.
    pub fn new(core: Arc<AgentServerCore>) -> Self {
        Self { core }
    }

    /// Build an axum `Router` with the JSON-RPC WebSocket endpoint at `/ws`.
    pub fn into_axum_router(self) -> Router {
        let server = Arc::new(self);

        Router::new()
            .route(
                "/ws",
                get(move |ws: WebSocketUpgrade| {
                    let server = server.clone();
                    async move { ws.on_upgrade(move |socket| handle_ws(socket, server)) }
                }),
            )
    }
}

async fn handle_ws(socket: WebSocket, server: Arc<JsonRpcServer>) {
    let conn = JsonRpcConnection::new(socket);
    server.core.serve(conn).await;
}
```

- [ ] **Step 3: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/server_core.rs crates/vol-llm-agent-channel/src/jsonrpc/server.rs
git commit -m "feat: add AgentServerCore::serve() and simplify JsonRpcServer"
```

---

### Task 5: Wire agent logic into `AgentHandler`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`
- Modify: `crates/vol-llm-agent-channel/src/router.rs` (add cancel)

- [ ] **Step 1: Add `cancel()` to `AgentRouter`**

In `router.rs`, after the `send()` method (after line 47), add:

```rust
    /// Cancel a request by req_id across all registered dispatchers.
    pub async fn cancel(&self, req_id: &str) -> bool {
        for dispatcher in self.dispatchers.read().await.values() {
            if dispatcher.cancel(req_id).await {
                return true;
            }
        }
        false
    }
```

- [ ] **Step 2: Compile check router change**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 3: Update `AgentHandler` — add real Submit/Cancel/Subscribe/Unsubscribe/Approve logic**

Replace the entire `domain/agent.rs` with:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;

use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload, ProtocolError,
};
use crate::connection::ConnectionHolder;
use crate::domain::handler::DomainHandler;
use crate::request::AgentRequest;
use crate::router::AgentRouter;

/// Handler for agent-domain operations.
pub struct AgentHandler {
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
}

impl AgentHandler {
    pub fn new(
        router: AgentRouter,
        holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    ) -> Self {
        Self { router, holders }
    }
}

#[async_trait]
impl DomainHandler for AgentHandler {
    fn name(&self) -> &str {
        "agent"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Agent(AgentOperation::Submit),
            Operation::Agent(AgentOperation::Cancel),
            Operation::Agent(AgentOperation::Subscribe),
            Operation::Agent(AgentOperation::Unsubscribe),
            Operation::Agent(AgentOperation::Approve),
            Operation::Agent(AgentOperation::List),
            Operation::Agent(AgentOperation::Event),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Agent(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("agent")),
        };
        match (op, message.payload) {
            (AgentOperation::Submit, Payload::Agent(AgentPayload::Submit { input, target, metadata: _ })) => {
                let target_id = {
                    let holders = self.holders.lock().unwrap();
                    target
                        .filter(|t| holders.contains_key(t))
                        .or_else(|| holders.keys().next().cloned())
                        .unwrap_or_else(|| "agent".to_string())
                };

                let request = AgentRequest::new(&target_id, &input);
                let req_id = request.req_id.clone();

                match self.router.send(&target_id, request).await {
                    Ok(rx) => {
                        // Spawn background task to await result.
                        let router = self.router.clone();
                        tokio::spawn(async move {
                            Self::process_run_result(rx, &req_id, &router).await;
                        });

                        let run_id = uuid::Uuid::new_v4().to_string();
                        Ok(vec![
                            AgentServerMessage::new_ack(
                                message.message_id.clone(),
                                Operation::Agent(AgentOperation::Submit),
                                Payload::Agent(AgentPayload::SubmitAck {
                                    run_id: run_id.clone(),
                                    accepted: true,
                                }),
                            ),
                            AgentServerMessage::new_result(
                                message.message_id,
                                Operation::Agent(AgentOperation::Submit),
                                Payload::Agent(AgentPayload::SubmitResult {
                                    run_id,
                                    response: serde_json::json!({"req_id": req_id}),
                                }),
                            ),
                        ])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Agent(AgentOperation::Submit),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "agent_submit_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
            (AgentOperation::Cancel, Payload::Agent(AgentPayload::Cancel { req_id })) => {
                let cancelled = self.router.cancel(&req_id).await;
                let run_id = uuid::Uuid::new_v4().to_string();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Cancel),
                    Payload::Agent(AgentPayload::CancelResult {
                        run_id,
                        cancelled,
                    }),
                )])
            }
            (AgentOperation::Subscribe, Payload::Agent(AgentPayload::Subscribe { .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Subscribe),
                    Payload::Agent(AgentPayload::SubscribeResult {
                        subscription_id: uuid::Uuid::new_v4().to_string(),
                    }),
                ),
            ]),
            (AgentOperation::Unsubscribe, Payload::Agent(AgentPayload::Unsubscribe { subscription_id })) => {
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Unsubscribe),
                    Payload::Agent(AgentPayload::UnsubscribeResult {
                        subscription_id,
                        removed: true,
                    }),
                )])
            }
            (AgentOperation::Approve, Payload::Agent(AgentPayload::Approve { run_id, .. })) => Ok(vec![
                AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::Approve),
                    Payload::Agent(AgentPayload::ApproveResult {
                        run_id,
                        accepted: true,
                    }),
                ),
            ]),
            (AgentOperation::List, _) => {
                let agents: Vec<serde_json::Value> = self
                    .holders
                    .lock()
                    .unwrap()
                    .keys()
                    .map(|k| serde_json::json!({ "id": k, "name": k }))
                    .collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::List),
                    Payload::Agent(AgentPayload::ListResult { agents }),
                )])
            }
            (AgentOperation::Event, Payload::Agent(AgentPayload::Event { run_id, event })) => Ok(vec![
                AgentServerMessage::new_event(
                    message.message_id,
                    Operation::Agent(AgentOperation::Event),
                    Payload::Agent(AgentPayload::Event { run_id, event }),
                ),
            ]),
            (AgentOperation::Submit, _) => Err(ProtocolError::PayloadDecodeFailed("agent.submit")),
            (AgentOperation::Cancel, _) => Err(ProtocolError::PayloadDecodeFailed("agent.cancel")),
            (AgentOperation::Subscribe, _) => Err(ProtocolError::PayloadDecodeFailed("agent.subscribe")),
            (AgentOperation::Unsubscribe, _) => Err(ProtocolError::PayloadDecodeFailed("agent.unsubscribe")),
            (AgentOperation::Approve, _) => Err(ProtocolError::PayloadDecodeFailed("agent.approve")),
            (AgentOperation::Event, _) => Err(ProtocolError::PayloadDecodeFailed("agent.event")),
        }
    }

    async fn process_run_result(
        rx: tokio::sync::oneshot::Receiver<crate::request::RunResult>,
        req_id: &str,
        _router: &AgentRouter,
    ) {
        match rx.await {
            Ok(result) => {
                match &result.response {
                    Ok(response) => {
                        tracing::info!(%req_id, run_id = ?result.run_id, iterations = response.iterations, "agent run completed");
                    }
                    Err(e) => {
                        tracing::error!(%req_id, %e, "agent run failed");
                    }
                }
            }
            Err(_) => {
                tracing::warn!(%req_id, "agent run receiver dropped (possibly cancelled)");
            }
        }
    }
}
```

- [ ] **Step 4: Compile check**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/agent.rs crates/vol-llm-agent-channel/src/router.rs
git commit -m "feat: wire real agent logic into AgentHandler, add router.cancel()"
```

---

### Task 6: Run full test suite and fix issues

**Files:**
- All tests in `crates/vol-llm-agent-channel/`

- [ ] **Step 1: Run all tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1 | tail -20`
Expected: All tests PASS (some test code may need updating since `JsonRpcConnection` interface changed).

- [ ] **Step 2: Fix connection_protocol_test.rs**

The test file `tests/connection_protocol_test.rs` may create `JsonRpcConnection::new(ws, core)` — needs to update to just `JsonRpcConnection::new(ws)`.

Read and fix the file: `crates/vol-llm-agent-channel/tests/connection_protocol_test.rs`

Also check `tests/jsonrpc_integration.rs` and `tests/jsonrpc_ws_gateway_test.rs` — tests that call `parse_jsonrpc_request` will need to be removed or rewritten.

- [ ] **Step 3: Run clippy**

Run: `cargo clippy -p vol-llm-agent-channel 2>&1 | tail -10`
Expected: No new warnings.

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: update tests for unified dispatch"
```

---

### Task 7: Build example and verify

**Files:**
- `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`
- `crates/vol-llm-agent-channel/src/lib.rs`

- [ ] **Step 1: Check example compiles**

Run: `cargo check --example jsonrpc_agent_service -p vol-llm-agent-channel 2>&1`
Expected: PASS. The example creates `AgentServerCore` and `JsonRpcServer`, which should still work with the new API.

- [ ] **Step 2: Check `JsonRpcRequest` is removed from `lib.rs` exports**

Search `lib.rs` for `JsonRpcRequest` — if re-exported, remove it.

- [ ] **Step 3: Commit**

```bash
git commit -m "chore: verify example compiles with unified dispatch"
```
---

### Task 8: Final verification — full test suite

- [ ] **Step 1: Run full test suite**

Run: `cargo test -p vol-llm-agent-channel 2>&1`
Expected: All tests PASS, 0 failures.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy -p vol-llm-agent-channel 2>&1 | head -20`
Expected: No errors, only pre-existing warnings.

- [ ] **Step 3: Done**
