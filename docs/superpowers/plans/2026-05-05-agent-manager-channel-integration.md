# Agent Manager Channel Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Unify `vol-llm-agent-channel` protocol into a single `Message` type with sender/receiver fields, then integrate it into `vol-agent-manager` by replacing its own WebSocket protocol.

**Architecture:** Replace `InboundMessage`/`OutboundMessage` with a unified `Message` enum in the channel crate. Update `Connection` trait to `recv() -> Message` and `send(Message)`. Then have vol-agent-manager depend on the channel crate and use its types directly, deleting its own ws/protocol.rs.

**Tech Stack:** Rust, tokio, axum WebSocket, serde, async-trait

---

## File Structure

| Phase | File | Action | Responsibility |
|-------|------|--------|----------------|
| 1 | `crates/vol-llm-agent-channel/src/protocol.rs` | Rewrite | Replace InboundMessage/OutboundMessage with unified Message enum |
| 1 | `crates/vol-llm-agent-channel/src/connection.rs` | Modify | Update Connection trait and tests to use Message |
| 1 | `crates/vol-llm-agent-channel/src/lib.rs` | Modify | Update public exports |
| 1 | `crates/vol-llm-agent-channel/src/transport/ws.rs` | Rewrite | Update WsConnection to use Message, simplify Connection impl |
| 1 | `crates/vol-llm-agent-channel/src/transport/memory.rs` | Modify | Update MemoryConnection to use Message |
| 1 | `crates/vol-llm-agent-channel/src/dispatcher.rs` | Modify | Use Message types where applicable |
| 1 | `crates/vol-llm-agent-channel/src/router.rs` | Modify | Use Message types where applicable |
| 2 | `crates/vol-agent-manager/Cargo.toml` | Modify | Add vol-llm-agent-channel dependency |
| 2 | `crates/vol-agent-manager/src/ws/protocol.rs` | Delete | Remove all local protocol types |
| 2 | `crates/vol-agent-manager/src/ws/mod.rs` | Modify | Remove protocol module export |
| 2 | `crates/vol-agent-manager/src/ws/handler.rs` | Rewrite | Use Connection trait and Message types |
| 2 | `crates/vol-agent-manager/src/ws/server.rs` | Modify | Wire up WsConnection |
| 2 | `crates/vol-agent-manager/src/lib.rs` | Modify | Update imports |

---

### Task 1: Unified Message Type in vol-llm-agent-channel

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/protocol.rs`
- Modify: `crates/vol-llm-agent-channel/src/lib.rs`

- [ ] **Step 1: Rewrite protocol.rs with unified Message enum**

Replace the entire contents of `crates/vol-llm-agent-channel/src/protocol.rs` with:

```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Unified message type for all agent communication.
///
/// Direction is determined by `sender` and `receiver` fields, not by the type name.
/// The same message can be both received and sent on any connection.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    Submit {
        req_id: String,
        sender: String,
        receiver: String,
        input: String,
        #[serde(default)]
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    Cancel {
        req_id: String,
        sender: String,
        receiver: String,
    },
    Connected {
        sender: String,
        receiver: String,
    },
    Event {
        sender: String,
        receiver: String,
        event: serde_json::Value,
    },
    Result {
        req_id: String,
        sender: String,
        receiver: String,
        result: serde_json::Value,
    },
    Error {
        req_id: Option<String>,
        sender: String,
        receiver: String,
        message: String,
    },
}
```

- [ ] **Step 2: Update lib.rs public exports**

Replace line 17 in `crates/vol-llm-agent-channel/src/lib.rs`:

Change:
```rust
pub use protocol::{InboundMessage, OutboundMessage};
```

To:
```rust
pub use protocol::Message;
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/protocol.rs crates/vol-llm-agent-channel/src/lib.rs
git commit -m "refactor: replace InboundMessage/OutboundMessage with unified Message enum"
```

---

### Task 2: Update Connection Trait

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/connection.rs`

- [ ] **Step 1: Update Connection trait**

In `crates/vol-llm-agent-channel/src/connection.rs`, update the trait and tests.

Change the import from:
```rust
use crate::protocol::InboundMessage;
```

To:
```rust
use crate::protocol::Message;
```

Replace the Connection trait definition:

```rust
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    /// Protocol identifier (e.g., "ws", "memory").
    fn protocol(&self) -> &str;

    /// Receive the next incoming message.
    async fn recv(&mut self) -> Option<Result<Message, ConnectionError>>;

    /// Send a message.
    async fn send(&self, msg: Message) -> Result<(), ConnectionError>;
}
```

Update the MockConnection in tests (around line 95-105):

```rust
struct MockConnection {
    protocol: String,
}

#[async_trait]
impl Connection for MockConnection {
    fn protocol(&self) -> &str { &self.protocol }
    async fn recv(&mut self) -> Option<Result<Message, ConnectionError>> { None }
    async fn send(&self, _msg: Message) -> Result<(), ConnectionError> { Ok(()) }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent-channel/src/connection.rs
git commit -m "refactor: update Connection trait to use unified Message type"
```

---

### Task 3: Update WebSocket Transport

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/transport/ws.rs`

- [ ] **Step 1: Rewrite ws.rs to use Message type**

Update `crates/vol-llm-agent-channel/src/transport/ws.rs`. This is a substantial rewrite. Key changes:

1. Change import from `use crate::protocol::{InboundMessage, OutboundMessage};` to `use crate::protocol::Message;`

2. Replace `serialize_outbound` with a single `serialize_message`:

```rust
fn serialize_message(msg: &Message) -> Result<String, ConnectionError> {
    serde_json::to_string(msg).map_err(|e| ConnectionError::WsSendError(e.to_string()))
}
```

3. Replace `send_error`, `send_connected`, `send_event`, `send_result` methods with a unified `send`:

Remove these methods entirely: `send_error`, `send_connected`, `send_text`, `handle_inbound`, `send_connected`, `send_event`, `send_result`.

The Connection impl becomes:

```rust
#[async_trait]
impl Connection for WsConnection {
    fn protocol(&self) -> &str {
        "ws"
    }

    async fn recv(&mut self) -> Option<Result<Message, ConnectionError>> {
        let msg = self.rx.next().await?;
        match msg {
            Ok(WsMessage::Text(text)) => {
                match serde_json::from_str::<Message>(&text) {
                    Ok(msg) => Some(Ok(msg)),
                    Err(e) => Some(Err(ConnectionError::ParseError(e.to_string()))),
                }
            }
            Ok(WsMessage::Close(_)) => None,
            Ok(WsMessage::Binary(_)) => {
                Some(Err(ConnectionError::ParseError("binary messages not supported".to_string())))
            }
            Ok(WsMessage::Ping(data)) => {
                tracing::debug!("WebSocket ping: {} bytes", data.len());
                self.recv().await
            }
            Ok(WsMessage::Pong(_)) => self.recv().await,
            Err(e) => Some(Err(ConnectionError::WsReceiveError(e.to_string()))),
        }
    }

    async fn send(&self, msg: Message) -> Result<(), ConnectionError> {
        let text = serialize_message(&msg)?;
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text))
            .await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))
    }
}
```

4. The `run()` method and `handle_inbound` method need to be removed from WsConnection since the message handling is now externalized. The manager will call `recv()` and `send()` directly. However, we need to keep the `run()` method for the existing agent use case (ReActAgent with ConnectionHolder). Let's simplify it to use the new Connection trait:

```rust
pub async fn run(mut self) {
    // Send a connected message so the client knows the agent ID.
    let connected = Message::Connected {
        sender: self.agent_id.clone(),
        receiver: "client".to_string(),
    };
    let _ = self.send(connected).await;

    loop {
        match self.recv().await {
            Some(Ok(msg)) => {
                match self.handle_message(msg).await {
                    Ok(()) => {}
                    Err(e) => {
                        tracing::warn!(%e, "error handling inbound message");
                        let _ = self.send(Message::Error {
                            req_id: None,
                            sender: self.agent_id.clone(),
                            receiver: "client".to_string(),
                            message: format!("handler error: {e}"),
                        }).await;
                    }
                }
            }
            Some(Err(e)) => {
                tracing::warn!(%e, "receive error");
                break;
            }
            None => {
                tracing::info!("WebSocket connection closed");
                break;
            }
        }
    }

    self.holder.detach().await;
}

async fn handle_message(&self, msg: Message) -> Result<(), ConnectionError> {
    match msg {
        Message::Submit { req_id, input, metadata, .. } => {
            let mut request = crate::request::AgentRequest::with_id(&req_id, &self.agent_id, &input);
            if let Some(meta) = metadata {
                request.metadata = meta;
            }

            let rx = match self.dispatcher.submit(request) {
                Ok(rx) => rx,
                Err(e) => {
                    return self.send(Message::Error {
                        req_id: Some(req_id),
                        sender: self.agent_id.clone(),
                        receiver: "client".to_string(),
                        message: e.to_string(),
                    }).await;
                }
            };

            match rx.await {
                Ok(run_result) => {
                    let response_value = match &run_result.response {
                        Ok(resp) => serde_json::to_value(resp)
                            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?,
                        Err(err) => serde_json::json!({ "error": err.to_string() }),
                    };

                    let result = Message::Result {
                        req_id: run_result.req_id,
                        sender: self.agent_id.clone(),
                        receiver: "client".to_string(),
                        result: response_value,
                    };
                    self.send(result).await
                }
                Err(_) => {
                    self.send(Message::Error {
                        req_id: Some(req_id),
                        sender: self.agent_id.clone(),
                        receiver: "client".to_string(),
                        message: "dispatcher dropped while processing request".to_string(),
                    }).await
                }
            }
        }
        Message::Cancel { req_id, .. } => {
            let cancelled = self.dispatcher.cancel(&req_id).await;
            if cancelled {
                self.send(Message::Error {
                    req_id: Some(req_id),
                    sender: self.agent_id.clone(),
                    receiver: "client".to_string(),
                    message: "request cancelled".to_string(),
                }).await
            } else {
                self.send(Message::Error {
                    req_id: Some(req_id),
                    sender: self.agent_id.clone(),
                    receiver: "client".to_string(),
                    message: "request not found in queue (already executing or completed)".to_string(),
                }).await
            }
        }
        _ => {
            // Ignore other message types (Connected, Event, Result) in agent mode.
            Ok(())
        }
    }
}
```

5. Remove `serialize_stream_event` function and `send_event`/`send_result` Connection trait impls.

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/ws.rs
git commit -m "refactor: update WebSocket transport to use unified Message type"
```

---

### Task 4: Update Memory Transport

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/transport/memory.rs`

- [ ] **Step 1: Update memory.rs to use Message**

Read `crates/vol-llm-agent-channel/src/transport/memory.rs`. Update:

1. Change import from `use crate::protocol::InboundMessage;` to `use crate::protocol::Message;`

2. Replace all `InboundMessage` references with `Message`:
   - `rx: mpsc::UnboundedReceiver<InboundMessage>` → `rx: mpsc::UnboundedReceiver<Message>`
   - `let (in_tx, in_rx) = mpsc::unbounded_channel::<InboundMessage>()` → `mpsc::unbounded_channel::<Message>()`
   - `async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>>` → `Option<Result<Message, ConnectionError>>`
   - `tx: mpsc::UnboundedSender<InboundMessage>` → `mpsc::UnboundedSender<Message>`
   - `pub fn send(&self, msg: InboundMessage)` → `pub fn send(&self, msg: Message)`

3. Update the `Connection` impl for `MemoryConnection`:
   - `async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>>` → `Option<Result<Message, ConnectionError>>`
   - Remove `send_event` and `send_result`, replace with:

```rust
async fn send(&self, msg: Message) -> Result<(), ConnectionError> {
    self.tx
        .send(msg)
        .map_err(|e| ConnectionError::ChannelError(e.to_string()))
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/memory.rs
git commit -m "refactor: update memory transport to use unified Message type"
```

---

### Task 5: Update Dispatcher and Router

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/dispatcher.rs`
- Modify: `crates/vol-llm-agent-channel/src/router.rs`

- [ ] **Step 1: Review dispatcher.rs for Message references**

`dispatcher.rs` does not directly import `InboundMessage` or `OutboundMessage` — it uses `AgentRequest` and `RunResult` from `request.rs`. These types remain unchanged. No code changes needed.

- [ ] **Step 2: Review router.rs for Message references**

`router.rs` also does not directly import protocol types. No code changes needed.

- [ ] **Step 3: Run cargo check to verify**

```bash
cargo check -p vol-llm-agent-channel
```

Expected: Should pass if Tasks 1-4 are correct.

- [ ] **Step 4: Run tests**

```bash
cargo test -p vol-llm-agent-channel
```

Expected: All tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/dispatcher.rs crates/vol-llm-agent-channel/src/router.rs
git commit -m "chore: verify dispatcher and router compile with new Message type"
```

---

### Task 6: Update channel crate tests

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/connection.rs` (test section already updated in Task 2)
- Modify: `crates/vol-llm-agent-channel/src/transport/memory.rs` (test section)

- [ ] **Step 1: Run tests and fix any remaining test failures**

```bash
cargo test -p vol-llm-agent-channel -- --nocapture
```

If any tests fail, fix them by updating test code to use the new `Message` type. Ensure all inline tests in connection.rs, dispatcher.rs, router.rs, and memory.rs pass.

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent-channel/
git commit -m "test: fix all tests to use unified Message type"
```

---

### Task 7: Add vol-llm-agent-channel dependency to vol-agent-manager

**Files:**
- Modify: `crates/vol-agent-manager/Cargo.toml`

- [ ] **Step 1: Add dependency**

Add to the `[dependencies]` section of `crates/vol-agent-manager/Cargo.toml`:

```toml
vol-llm-agent-channel = { path = "../vol-llm-agent-channel" }
```

Also add `async-trait` if not already present:

```toml
async-trait = { workspace = true }
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-agent-manager
```

Expected: Will fail (handler.rs still uses old types). But the dependency should resolve correctly.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-manager/Cargo.toml Cargo.lock
git commit -m "chore: add vol-llm-agent-channel dependency to vol-agent-manager"
```

---

### Task 8: Delete ws/protocol.rs and update ws/mod.rs

**Files:**
- Delete: `crates/vol-agent-manager/src/ws/protocol.rs`
- Modify: `crates/vol-agent-manager/src/ws/mod.rs`

- [ ] **Step 1: Delete ws/protocol.rs**

```bash
git rm crates/vol-agent-manager/src/ws/protocol.rs
```

- [ ] **Step 2: Update ws/mod.rs**

Replace contents of `crates/vol-agent-manager/src/ws/mod.rs`:

```rust
pub mod handler;
pub mod server;
```

Remove the `pub mod protocol;` line.

- [ ] **Step 3: Verify build fails with expected errors**

```bash
cargo check -p vol-agent-manager 2>&1 | head -30
```

Expected: Errors about missing types in handler.rs (this is expected — we'll fix in Task 9).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-manager/src/ws/mod.rs
git commit -m "refactor: remove ws/protocol.rs and update module exports"
```

---

### Task 9: Rewrite ws/handler.rs to use Connection trait and Message types

**Files:**
- Modify: `crates/vol-agent-manager/src/ws/handler.rs`

- [ ] **Step 1: Rewrite handler.rs**

Replace the entire contents of `crates/vol-agent-manager/src/ws/handler.rs`:

```rust
use std::sync::Arc;

use axum::extract::ws::{Message as WsMessage, WebSocket};
use chrono::Utc;
use tracing::{error, info, warn};
use vol_llm_agent_channel::{Connection, Message};

use crate::events::{EventBus, ManagerEvent};
use crate::metrics::collector::MetricsCollector;
use crate::state::manager::AgentStateManager;
use crate::state::models::{AgentState, AgentStatus};
use crate::task::dispatcher::TaskDispatcher;

/// WebSocket connection adapter implementing the Connection trait.
struct ManagerConnection {
    tx: Arc<tokio::sync::Mutex<futures::stream::SplitSink<WebSocket, WsMessage>>>,
    rx: futures::stream::SplitStream<WebSocket>,
}

impl ManagerConnection {
    fn new(ws: WebSocket) -> Self {
        let (tx, rx) = ws.split();
        Self {
            tx: Arc::new(tokio::sync::Mutex::new(tx)),
            rx,
        }
    }

    async fn send_raw(&self, msg: Message) -> Result<(), anyhow::Error> {
        let text = serde_json::to_string(&msg)?;
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text)).await?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl Connection for ManagerConnection {
    fn protocol(&self) -> &str {
        "ws"
    }

    async fn recv(&mut self) -> Option<Result<Message, vol_llm_agent_channel::ConnectionError>> {
        let msg = self.rx.next().await?;
        match msg {
            Ok(WsMessage::Text(text)) => {
                match serde_json::from_str::<Message>(&text) {
                    Ok(msg) => Some(Ok(msg)),
                    Err(e) => Some(Err(vol_llm_agent_channel::ConnectionError::ParseError(e.to_string()))),
                }
            }
            Ok(WsMessage::Close(_)) => None,
            Ok(WsMessage::Binary(_) | WsMessage::Ping(_) | WsMessage::Pong(_)) => {
                self.recv().await
            }
            Err(e) => Some(Err(vol_llm_agent_channel::ConnectionError::WsReceiveError(e.to_string()))),
        }
    }

    async fn send(&self, msg: Message) -> Result<(), vol_llm_agent_channel::ConnectionError> {
        let text = serde_json::to_string(&msg)
            .map_err(|e| vol_llm_agent_channel::ConnectionError::WsSendError(e.to_string()))?;
        let mut tx = self.tx.lock().await;
        tx.send(WsMessage::Text(text))
            .await
            .map_err(|e| vol_llm_agent_channel::ConnectionError::WsSendError(e.to_string()))
    }
}

/// Handle a single WebSocket connection for an agent.
pub async fn handle_agent_connection(
    ws: WebSocket,
    token: Option<String>,
    state_manager: Arc<AgentStateManager>,
    metrics: Arc<MetricsCollector>,
    event_bus: Arc<EventBus>,
    task_dispatcher: Arc<TaskDispatcher>,
    expected_token: Option<String>,
) {
    // Auth check
    if let Some(expected) = &expected_token {
        if token.as_ref() != Some(expected) {
            let err = Message::Error {
                req_id: None,
                sender: "manager".to_string(),
                receiver: "client".to_string(),
                message: "invalid token".to_string(),
            };
            let text = serde_json::to_string(&err).unwrap();
            let _ = ws.send(WsMessage::Text(text)).await;
            return;
        }
    }

    let mut conn = ManagerConnection::new(ws);

    // Wait for register message (Submit with metadata type=register)
    let agent_id = match conn.recv().await {
        Some(Ok(Message::Submit { metadata, sender, input, .. })) => {
            let meta = metadata.as_ref();
            let is_register = meta.map_or(false, |m| {
                m.get("type").and_then(|v| v.as_str()) == Some("register")
            });
            if !is_register {
                warn!("First message was not register, closing connection");
                return;
            }

            let id = if sender != "client" { sender } else { input.clone() };

            // Parse register metadata from input field (JSON string)
            match serde_json::from_str::<serde_json::Value>(&input) {
                Ok(reg_data) => {
                    let name = reg_data.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let agent_type = reg_data.get("type").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                    let version = reg_data.get("version").and_then(|v| v.as_str()).unwrap_or("0.0.0").to_string();
                    let capabilities = reg_data.get("capabilities")
                        .and_then(|v| v.as_array())
                        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                        .unwrap_or_default();

                    let host_info = reg_data.get("host_info");
                    let hostname = host_info.and_then(|h| h.get("hostname").and_then(|v| v.as_str())).unwrap_or("unknown");
                    let os = host_info.and_then(|h| h.get("os").and_then(|v| v.as_str())).unwrap_or("unknown");
                    let arch = host_info.and_then(|h| h.get("arch").and_then(|v| v.as_str())).unwrap_or("unknown");
                    let ip = host_info.and_then(|h| h.get("ip").and_then(|v| v.as_str())).unwrap_or("0.0.0.0");

                    let state = AgentState {
                        agent_id: id.clone(),
                        name,
                        r#type: agent_type,
                        version,
                        capabilities,
                        host_info: crate::state::models::HostInfo {
                            hostname: hostname.to_string(),
                            os: os.to_string(),
                            arch: arch.to_string(),
                            ip: ip.to_string(),
                        },
                        status: AgentStatus::Idle,
                        connected_at: Utc::now(),
                        last_heartbeat: Utc::now(),
                    };
                    state_manager.register(state).await;
                    metrics.agent_registered_total.set(
                        state_manager.list_all().await.len() as f64,
                    );
                    event_bus.emit(ManagerEvent::agent_registered(&id));
                    id
                }
                Err(e) => {
                    warn!("Invalid register payload: {}", e);
                    return;
                }
            }
        }
        _ => {
            warn!("Connection closed before register");
            return;
        }
    };

    // Send Connected ack
    let _ = conn.send(Message::Connected {
        sender: "manager".to_string(),
        receiver: agent_id.clone(),
    }).await;

    metrics.agent_connections_current.inc();

    // Message loop
    loop {
        match conn.recv().await {
            Some(Ok(msg)) => {
                if let Err(e) = handle_message(
                    &msg, &agent_id, &state_manager, &metrics, &event_bus, &task_dispatcher, &conn,
                ).await {
                    let _ = conn.send(Message::Error {
                        req_id: None,
                        sender: "manager".to_string(),
                        receiver: agent_id.clone(),
                        message: e.to_string(),
                    }).await;
                }
            }
            Some(Err(e)) => {
                error!(agent_id = %agent_id, "WebSocket error: {}", e);
                break;
            }
            None => {
                info!(agent_id = %agent_id, "Agent disconnected");
                state_manager
                    .update_status(&agent_id, AgentStatus::Disconnected)
                    .await;
                metrics.agent_connections_current.dec();
                event_bus.emit(ManagerEvent::agent_disconnected(&agent_id));
                break;
            }
        }
    }
}

async fn handle_message(
    msg: &Message,
    agent_id: &str,
    state_manager: &AgentStateManager,
    metrics: &MetricsCollector,
    event_bus: &EventBus,
    task_dispatcher: &TaskDispatcher,
    _conn: &ManagerConnection,
) -> Result<(), anyhow::Error> {
    let agent_type = state_manager
        .get(agent_id)
        .await
        .map(|s| s.r#type.clone())
        .unwrap_or_else(|| "unknown".to_string());

    match msg {
        Message::Submit { metadata, input, .. } => {
            let meta_type = metadata.as_ref().and_then(|m| {
                m.get("type").and_then(|v| v.as_str())
            }).unwrap_or("unknown");

            match meta_type {
                "heartbeat" => {
                    state_manager.update_heartbeat(agent_id).await;
                    let status = serde_json::from_str::<serde_json::Value>(input)
                        .ok()
                        .and_then(|v| v.get("status").and_then(|s| s.as_str()))
                        .unwrap_or("Idle");
                    if status == "Busy" {
                        state_manager.update_status(agent_id, AgentStatus::Busy).await;
                    } else {
                        state_manager.update_status(agent_id, AgentStatus::Idle).await;
                    }
                    metrics.increment_messages("heartbeat", agent_id, &agent_type);
                }
                "metric" => {
                    metrics.increment_metric_samples(agent_id);
                    metrics.increment_messages("metric", agent_id, &agent_type);
                }
                "event" => {
                    let data = serde_json::from_str::<serde_json::Value>(input)
                        .unwrap_or(serde_json::Value::Null);
                    let event_name = data.get("event_name").and_then(|v| v.as_str()).unwrap_or("unknown");
                    event_bus.emit(ManagerEvent::agent_event(agent_id, event_name, data));
                    metrics.increment_messages("event", agent_id, &agent_type);
                }
                "task_result" => {
                    let data = serde_json::from_str::<serde_json::Value>(input)
                        .unwrap_or(serde_json::Value::Null);
                    let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("unknown");
                    let result = data.get("result").cloned();
                    let error = data.get("error").and_then(|v| v.as_str());

                    // Extract task_id from metadata
                    let task_id = metadata.as_ref()
                        .and_then(|m| m.get("task_id"))
                        .and_then(|v| v.as_str());

                    if let Some(task_id) = task_id {
                        match status {
                            "Completed" => {
                                task_dispatcher.complete_task(task_id, result, None).await;
                                event_bus.emit(ManagerEvent::task_completed(task_id, agent_id));
                            }
                            "Failed" => {
                                let error_msg = error.unwrap_or("unknown error");
                                task_dispatcher.fail_task(task_id, error_msg).await;
                                event_bus.emit(ManagerEvent::task_failed(task_id, agent_id, error_msg));
                            }
                            _ => {
                                warn!(task_id, agent_id, status = %status, "Unknown task result status");
                            }
                        }
                    }
                    metrics.increment_messages("task_result", agent_id, &agent_type);
                }
                unknown => {
                    return Err(anyhow::anyhow!("Unknown submit message type: {}", unknown));
                }
            }
        }
        Message::Cancel { req_id, .. } => {
            info!(agent_id, req_id, "Received cancel request");
        }
        _ => {
            // Ignore Connected, Event, Result, Error from agent
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_register_metadata() {
        let json = serde_json::json!({
            "name": "test-agent",
            "type": "react-agent",
            "version": "0.1.0",
            "capabilities": ["Read", "Bash"],
            "host_info": {
                "hostname": "host1",
                "os": "linux",
                "arch": "x86_64",
                "ip": "10.0.0.1"
            }
        });
        assert_eq!(json.get("name").and_then(|v| v.as_str()), Some("test-agent"));
    }

    #[test]
    fn test_parse_heartbeat_metadata() {
        let meta = serde_json::json!({
            "type": "heartbeat",
            "status": "Idle"
        });
        assert_eq!(meta.get("type").and_then(|v| v.as_str()), Some("heartbeat"));
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message::Submit {
            req_id: "req-1".to_string(),
            sender: "agent-a".to_string(),
            receiver: "manager".to_string(),
            input: "hello".to_string(),
            metadata: None,
        };
        let serialized = serde_json::to_string(&msg).unwrap();
        assert!(serialized.contains("submit"));
        assert!(serialized.contains("agent-a"));
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p vol-agent-manager
```

Fix any compilation errors. The most likely issues are:
- Missing imports
- Type mismatches in the Connection impl

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-manager/src/ws/handler.rs
git commit -m "refactor: rewrite ws handler to use Connection trait and Message types"
```

---

### Task 10: Update ws/server.rs

**Files:**
- Modify: `crates/vol-agent-manager/src/ws/server.rs`

- [ ] **Step 1: Update server.rs**

The server.rs currently calls `handle_agent_connection` with a raw WebSocket. It should remain the same since the handler accepts `WebSocket` and wraps it internally in `ManagerConnection`. No changes needed to server.rs.

Verify:

```bash
cargo check -p vol-agent-manager
```

- [ ] **Step 2: Commit (if no changes needed, skip commit)**

If changes were needed:
```bash
git add crates/vol-agent-manager/src/ws/server.rs
git commit -m "refactor: update ws server for new handler signature"
```

---

### Task 11: Final compilation and tests

**Files:**
- All modified files

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Full workspace test**

```bash
cargo test --workspace -- --nocapture
```

- [ ] **Step 3: Fix any remaining issues**

Fix all compilation and test failures until `cargo check --workspace` and `cargo test --workspace` both pass cleanly.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "fix: resolve all compilation and test failures for channel integration"
```

---
