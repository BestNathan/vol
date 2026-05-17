# Agent Transport Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `Connection` trait, `ConnectionHolder` (AgentPlugin), message protocol, and WebSocket/in-memory transport implementations to the `vol-llm-agent-channel` crate.

**Architecture:** Three layers — `Connection` trait abstracts communication channels, `ConnectionHolder` implements `AgentPlugin` to forward streaming events to the active connection, `WsServer`/`WsConnection` and `MemoryConnection` implement the transport protocols. Agent and connection have independent lifecycles.

**Tech Stack:** Rust, tokio, axum (with ws feature), tokio-tungstenite, serde, serde_json, async-trait

---

### Task 1: Add workspace dependencies

**Files:**
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Add axum to workspace.dependencies**

Check if `axum` is already in workspace dependencies (it is, at line 54 of root Cargo.toml). Ensure the `ws` feature is present:

```toml
# In root Cargo.toml [workspace.dependencies]
axum = { version = "0.7", features = ["ws"] }
```

Also check `tokio-tungstenite` exists (it does at line 51).

- [ ] **Step 2: Add new dependencies to vol-llm-agent-channel/Cargo.toml**

Read the current file, then overwrite:

```toml
[package]
name = "vol-llm-agent-channel"
version.workspace = true
edition.workspace = true

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
uuid = { version = "1.6", features = ["v4"] }
vol-llm-agent = { path = "../vol-llm-agent" }
async-trait = { workspace = true }
axum = { workspace = true }
futures = "0.3"
tokio-tungstenite = { workspace = true }
chrono = { workspace = true }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Add transport module to lib.rs**

Edit `crates/vol-llm-agent-channel/src/lib.rs`:

```rust
//! vol-llm-agent-channel: Channel-based communication layer for ReActAgent.
//!
//! Provides `AgentDispatcher` for single-agent request queueing and
//! `AgentRouter` for multi-agent request routing.

pub mod connection;
pub mod dispatcher;
pub mod error;
pub mod protocol;
pub mod request;
pub mod router;
pub mod transport;

pub use connection::{Connection, ConnectionHolder};
pub use dispatcher::AgentDispatcher;
pub use error::ChannelError;
pub use protocol::{InboundMessage, OutboundMessage};
pub use request::{AgentRequest, RunResult};
pub use router::AgentRouter;
pub use transport::{MemoryConnection, MemoryHandle, WsServer};
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors (will have "unused import" warnings for the new modules — that's fine, we'll implement them in subsequent tasks)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/
git commit -m "feat: add transport dependencies and module declarations"
```

---

### Task 2: Add ConnectionError type

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/error.rs`

- [ ] **Step 1: Add ConnectionError to error.rs**

Read the current file, then replace it with:

```rust
// crates/vol-llm-agent-channel/src/error.rs

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    /// Target agent not found in router.
    #[error("agent '{0}' not registered")]
    AgentNotFound(String),

    /// Request was cancelled before execution.
    #[error("request '{0}' was cancelled")]
    Cancelled(String),

    /// Dispatcher dropped while request was pending.
    #[error("dispatcher dropped")]
    DispatcherDropped,

    /// Internal agent error (from ReActAgent::run).
    #[error("agent execution error: {0}")]
    AgentError(String),
}

/// Error type for connection operations.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    /// WebSocket send failed.
    #[error("websocket send error: {0}")]
    WsSendError(String),

    /// WebSocket receive failed.
    #[error("websocket receive error: {0}")]
    WsReceiveError(String),

    /// Failed to parse message.
    #[error("parse error: {0}")]
    ParseError(String),

    /// Connection was closed.
    #[error("connection closed")]
    Closed,

    /// Channel send failed (in-memory transport).
    #[error("channel send error: {0}")]
    ChannelError(String),
}
```

- [ ] **Step 2: Re-export ConnectionError from lib.rs**

Edit `crates/vol-llm-agent-channel/src/lib.rs`, add to the pub use line for error:

```rust
pub use error::{ChannelError, ConnectionError};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/error.rs crates/vol-llm-agent-channel/src/lib.rs
git commit -m "feat: add ConnectionError type"
```

---

### Task 3: Implement message protocol

**Files:**
- Create: `crates/vol-llm-agent-channel/src/protocol.rs`

- [ ] **Step 1: Write protocol.rs**

```rust
// crates/vol-llm-agent-channel/src/protocol.rs

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use vol_llm_agent::AgentStreamEvent;

use crate::request::RunResult;

/// Messages received from client (inbound).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InboundMessage {
    Submit {
        req_id: String,
        target_id: String,
        input: String,
        #[serde(default)]
        metadata: Option<HashMap<String, serde_json::Value>>,
    },
    Cancel {
        req_id: String,
    },
}

/// Messages sent to client (outbound).
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundMessage {
    Connected {
        agent_id: String,
    },
    Event {
        event: AgentStreamEvent,
    },
    Result {
        result: RunResultWrapper,
    },
    Error {
        req_id: Option<String>,
        message: String,
    },
}

/// Wrapper for RunResult that makes it serializable for transport.
/// RunResult contains AgentResponse which has complex types.
#[derive(Debug, Serialize)]
pub struct RunResultWrapper {
    pub req_id: String,
    pub target_id: String,
    pub run_id: Option<String>,
    pub response: Result<AgentResponseWrapper, String>,
}

/// Simplified serializable version of AgentResponse for transport.
#[derive(Debug, Serialize)]
pub struct AgentResponseWrapper {
    pub content: String,
    pub run_id: String,
    pub session_id: String,
    pub iterations: u32,
    pub tool_calls: Vec<ToolCallSummary>,
    pub error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ToolCallSummary {
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub iteration: u32,
    pub success: bool,
}

impl RunResultWrapper {
    pub fn from_run_result(result: crate::request::RunResult) -> Self {
        Self {
            req_id: result.req_id,
            target_id: result.target_id,
            run_id: result.run_id,
            response: match result.response {
                Ok(resp) => Ok(AgentResponseWrapper {
                    content: resp.content.clone(),
                    run_id: resp.run_id.clone(),
                    session_id: resp.session_id.clone(),
                    iterations: resp.iterations,
                    tool_calls: resp.tool_calls.iter().map(|t| ToolCallSummary {
                        tool_name: t.tool_name.clone(),
                        arguments: t.arguments.clone(),
                        result: t.result.clone(),
                        iteration: t.iteration,
                        success: t.success,
                    }).collect(),
                    error: resp.error.clone(),
                }),
                Err(e) => Err(e.to_string()),
            },
        }
    }
}
```

- [ ] **Step 2: Add Serialize/Deserialize to AgentStreamEvent**

The `AgentStreamEvent` from vol-llm-core may not derive Serialize/Deserialize. Check:

```bash
grep -n 'derive.*Serialize' /root/nq-deribit/crates/vol-llm-core/src/stream.rs
```

If `AgentStreamEvent` doesn't derive Serialize, we need to add a serializable wrapper. In that case, modify `protocol.rs` to use a helper:

```rust
// If AgentStreamEvent doesn't implement Serialize, wrap it:
use vol_llm_agent::AgentStreamEvent;

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SerializableEvent {
    AgentStart { input: String },
    AgentComplete { response: Option<serde_json::Value> },
    AgentAborted { reason: String },
    // ... other variants
}

impl From<&AgentStreamEvent> for SerializableEvent {
    fn from(event: &AgentStreamEvent) -> Self {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => Self::AgentStart { input: input.clone() },
            AgentStreamEvent::AgentComplete { response, .. } => Self::AgentComplete { response: response.clone() },
            AgentStreamEvent::AgentAborted { reason, .. } => Self::AgentAborted { reason: reason.clone() },
            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                Self::MaxIterationsReached {
                    current_iteration: *current_iteration,
                    max_iterations: *max_iterations,
                }
            }
            AgentStreamEvent::ThinkingStart { .. } => Self::ThinkingStart {},
            AgentStreamEvent::ThinkingDelta { delta, .. } => Self::ThinkingDelta { delta: delta.clone() },
            AgentStreamEvent::ThinkingComplete { thinking, .. } => Self::ThinkingComplete { thinking: thinking.clone() },
            AgentStreamEvent::ContentStart { .. } => Self::ContentStart {},
            AgentStreamEvent::ContentDelta { delta, .. } => Self::ContentDelta { delta: delta.clone() },
            AgentStreamEvent::ContentComplete { content, .. } => Self::ContentComplete { content: content.clone() },
            AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, arguments, .. } => {
                Self::ToolCallBegin {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    arguments: arguments.clone(),
                }
            }
            AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } => {
                Self::ToolCallComplete {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    result: result.clone(),
                    duration_ms: *duration_ms,
                }
            }
            AgentStreamEvent::ToolCallError { tool_call_id, tool_name, error, duration_ms, .. } => {
                Self::ToolCallError {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    error: error.clone(),
                    duration_ms: *duration_ms,
                }
            }
            AgentStreamEvent::ToolCallSkipped { tool_call_id, tool_name, reason, duration_ms, .. } => {
                Self::ToolCallSkipped {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    reason: reason.clone(),
                    duration_ms: *duration_ms,
                }
            }
            AgentStreamEvent::ToolCallArgumentDelta { tool_call_id, tool_name, delta, .. } => {
                Self::ToolCallArgumentDelta {
                    tool_call_id: tool_call_id.clone(),
                    tool_name: tool_name.clone(),
                    delta: delta.clone(),
                }
            }
            AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer, .. } => {
                Self::IterationComplete {
                    iteration: *iteration,
                    tool_calls: tool_calls.iter().map(|tc| serde_json::json!({
                        "id": tc.id,
                        "name": tc.name,
                        "arguments": tc.arguments,
                    })).collect(),
                    final_answer: final_answer.clone(),
                }
            }
            AgentStreamEvent::LLMCallStart { iteration, .. } => Self::LLMCallStart { iteration: *iteration },
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => Self::LLMCallComplete {
                model: model.clone(),
                usage: usage.as_ref().map(|u| serde_json::json!(u)),
            },
            AgentStreamEvent::LLMCallError { error, .. } => Self::LLMCallError { error: error.clone() },
            AgentStreamEvent::IterationContinued { from_iteration, .. } => Self::IterationContinued {
                from_iteration: *from_iteration,
            },
            AgentStreamEvent::PluginEvent { name, data, .. } => Self::PluginEvent {
                name: name.clone(),
                data: data.clone(),
            },
        }
    }
}
```

If `AgentStreamEvent` already derives Serialize, skip this helper and use it directly in `OutboundMessage::Event`.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/protocol.rs crates/vol-llm-agent-channel/src/lib.rs
git commit -m "feat: add message protocol types"
```

---

### Task 4: Implement Connection trait

**Files:**
- Create: `crates/vol-llm-agent-channel/src/connection.rs`

- [ ] **Step 1: Write connection.rs**

```rust
// crates/vol-llm-agent-channel/src/connection.rs

use std::sync::Arc;

use tokio::sync::RwLock;
use vol_llm_agent::plugins::{AgentPlugin, PluginId, AgentStreamEvent, RunContext};
use async_trait::async_trait;

use crate::error::ConnectionError;
use crate::protocol::InboundMessage;
use crate::request::RunResult;

/// Abstract connection for agent communication.
/// Implement for each transport protocol.
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    /// Protocol identifier (e.g., "ws", "memory").
    fn protocol(&self) -> &str;

    /// Receive the next incoming message from the client.
    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>>;

    /// Send an agent streaming event to the client.
    async fn send_event(&self, event: &AgentStreamEvent) -> Result<(), ConnectionError>;

    /// Send the final run result to the client.
    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError>;
}

/// Registered as AgentPlugin on agent creation.
/// Holds at most one active connection at a time.
/// Agent and connection have independent lifecycles.
pub struct ConnectionHolder {
    connection: Arc<RwLock<Option<Arc<dyn Connection>>>>,
}

impl ConnectionHolder {
    /// Create a new empty holder.
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
        }
    }

    /// Attach a connection. Detaches existing one first.
    pub async fn attach(&self, conn: Arc<dyn Connection>) {
        self.detach().await;
        *self.connection.write().await = Some(conn);
    }

    /// Detach current connection (if any).
    pub async fn detach(&self) {
        *self.connection.write().await = None;
    }

    /// Whether a connection is currently active.
    pub async fn is_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Get the current connection (for testing).
    pub async fn connection(&self) -> Option<Arc<dyn Connection>> {
        self.connection.read().await.clone()
    }
}

impl Default for ConnectionHolder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentPlugin for ConnectionHolder {
    fn id(&self) -> PluginId {
        "connection_holder".to_string()
    }

    fn priority(&self) -> u32 {
        50
    }

    async fn listen(&self, event: &AgentStreamEvent, _ctx: &RunContext) {
        if let Some(conn) = self.connection.read().await.as_ref() {
            let _ = conn.send_event(event).await;
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/connection.rs crates/vol-llm-agent-channel/src/lib.rs
git commit -m "feat: add Connection trait and ConnectionHolder plugin"
```

---

### Task 5: Add ConnectionHolder unit tests

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/connection.rs` (append tests)

- [ ] **Step 1: Add tests**

Append to `connection.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // A no-op Connection implementation for testing.
    struct MockConnection {
        protocol: String,
    }

    #[async_trait]
    impl Connection for MockConnection {
        fn protocol(&self) -> &str { &self.protocol }
        async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>> { None }
        async fn send_event(&self, _event: &AgentStreamEvent) -> Result<(), ConnectionError> { Ok(()) }
        async fn send_result(&self, _result: &RunResult) -> Result<(), ConnectionError> { Ok(()) }
    }

    #[tokio::test]
    async fn test_holder_new_is_empty() {
        let holder = ConnectionHolder::new();
        assert!(!holder.is_connected().await);
    }

    #[tokio::test]
    async fn test_holder_attach() {
        let holder = ConnectionHolder::new();
        let conn = Arc::new(MockConnection { protocol: "test".to_string() });

        holder.attach(conn.clone()).await;
        assert!(holder.is_connected().await);
        assert_eq!(holder.connection().await.unwrap().protocol(), "test");
    }

    #[tokio::test]
    async fn test_holder_detach_replaces_connection() {
        let holder = ConnectionHolder::new();
        let conn1 = Arc::new(MockConnection { protocol: "test1".to_string() });
        let conn2 = Arc::new(MockConnection { protocol: "test2".to_string() });

        holder.attach(conn1).await;
        assert_eq!(holder.connection().await.unwrap().protocol(), "test1");

        holder.attach(conn2).await;
        assert_eq!(holder.connection().await.unwrap().protocol(), "test2");
    }

    #[tokio::test]
    async fn test_holder_detach_clears() {
        let holder = ConnectionHolder::new();
        let conn = Arc::new(MockConnection { protocol: "test".to_string() });

        holder.attach(conn).await;
        holder.detach().await;
        assert!(!holder.is_connected().await);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: all tests pass (existing 7 + 4 new = 11 tests)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/connection.rs
git commit -m "test: add ConnectionHolder unit tests"
```

---

### Task 6: Implement WebSocket transport

**Files:**
- Create: `crates/vol-llm-agent-channel/src/transport/mod.rs`
- Create: `crates/vol-llm-agent-channel/src/transport/ws.rs`

- [ ] **Step 1: Create transport/mod.rs**

```rust
// crates/vol-llm-agent-channel/src/transport/mod.rs

mod ws;

pub use ws::{WsConnection, WsServer};
```

- [ ] **Step 2: Create transport/ws.rs**

```rust
// crates/vol-llm-agent-channel/src/transport/ws.rs

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::{
    Router,
    extract::WebSocketUpgrade,
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use serde_json;
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;
use tracing;

use crate::connection::Connection;
use crate::dispatcher::AgentDispatcher;
use crate::error::ConnectionError;
use crate::protocol::{InboundMessage, OutboundMessage};
use crate::request::{AgentRequest, RunResult};

/// Serialize an AgentStreamEvent to JSON value.
/// AgentStreamEvent doesn't derive Serialize, so we construct the Value manually.
pub(crate) fn serialize_stream_event(event: &vol_llm_agent::AgentStreamEvent) -> serde_json::Value {
    use vol_llm_agent::AgentStreamEvent;
    match event {
        AgentStreamEvent::AgentStart { input, .. } => {
            serde_json::json!({ "type": "agent_start", "input": input })
        }
        AgentStreamEvent::AgentComplete { response, .. } => {
            serde_json::json!({ "type": "agent_complete", "response": response })
        }
        AgentStreamEvent::AgentAborted { reason, .. } => {
            serde_json::json!({ "type": "agent_aborted", "reason": reason })
        }
        AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
            serde_json::json!({
                "type": "max_iterations_reached",
                "current_iteration": current_iteration,
                "max_iterations": max_iterations,
            })
        }
        AgentStreamEvent::IterationContinued { from_iteration, .. } => {
            serde_json::json!({
                "type": "iteration_continued",
                "from_iteration": from_iteration,
            })
        }
        AgentStreamEvent::LLMCallStart { iteration, .. } => {
            serde_json::json!({ "type": "llm_call_start", "iteration": iteration })
        }
        AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
            serde_json::json!({
                "type": "llm_call_complete",
                "model": model,
                "usage": usage.as_ref().map(|u| serde_json::json!({
                    "prompt_tokens": u.prompt_tokens,
                    "completion_tokens": u.completion_tokens,
                    "total_tokens": u.total_tokens,
                    "cached_tokens": u.cached_tokens,
                })),
            })
        }
        AgentStreamEvent::LLMCallError { error, .. } => {
            serde_json::json!({ "type": "llm_call_error", "error": error })
        }
        AgentStreamEvent::ThinkingStart { .. } => {
            serde_json::json!({ "type": "thinking_start" })
        }
        AgentStreamEvent::ThinkingDelta { delta, .. } => {
            serde_json::json!({ "type": "thinking_delta", "delta": delta })
        }
        AgentStreamEvent::ThinkingComplete { thinking, .. } => {
            serde_json::json!({ "type": "thinking_complete", "thinking": thinking })
        }
        AgentStreamEvent::ContentStart { .. } => {
            serde_json::json!({ "type": "content_start" })
        }
        AgentStreamEvent::ContentDelta { delta, .. } => {
            serde_json::json!({ "type": "content_delta", "delta": delta })
        }
        AgentStreamEvent::ContentComplete { content, .. } => {
            serde_json::json!({ "type": "content_complete", "content": content })
        }
        AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, arguments, .. } => {
            serde_json::json!({
                "type": "tool_call_begin",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "arguments": arguments,
            })
        }
        AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } => {
            serde_json::json!({
                "type": "tool_call_complete",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "result": result,
                "duration_ms": duration_ms,
            })
        }
        AgentStreamEvent::ToolCallError { tool_call_id, tool_name, error, duration_ms, .. } => {
            serde_json::json!({
                "type": "tool_call_error",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "error": error,
                "duration_ms": duration_ms,
            })
        }
        AgentStreamEvent::ToolCallSkipped { tool_call_id, tool_name, reason, duration_ms, .. } => {
            serde_json::json!({
                "type": "tool_call_skipped",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "reason": reason,
                "duration_ms": duration_ms,
            })
        }
        AgentStreamEvent::ToolCallArgumentDelta { tool_call_id, tool_name, delta, .. } => {
            serde_json::json!({
                "type": "tool_call_argument_delta",
                "tool_call_id": tool_call_id,
                "tool_name": tool_name,
                "delta": delta,
            })
        }
        AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer, .. } => {
            serde_json::json!({
                "type": "iteration_complete",
                "iteration": iteration,
                "tool_calls": tool_calls.iter().map(|tc| serde_json::json!({
                    "id": tc.id,
                    "name": tc.name,
                    "arguments": tc.arguments,
                })).collect::<Vec<_>>(),
                "final_answer": final_answer,
            })
        }
        AgentStreamEvent::PluginEvent { name, data, .. } => {
            serde_json::json!({
                "type": "plugin_event",
                "name": name,
                "data": data,
            })
        }
    }
}

/// WebSocket connection implementing Connection trait.
pub struct WsConnection {
    /// Sender side — cloned for send operations.
    tx: tokio::sync::Mutex<axum::extract::ws::WebSocket>,
    /// Agent dispatcher for this connection.
    dispatcher: Arc<AgentDispatcher>,
}

impl WsConnection {
    pub fn new(socket: axum::extract::ws::WebSocket, dispatcher: Arc<AgentDispatcher>) -> Self {
        Self {
            tx: tokio::sync::Mutex::new(socket),
            dispatcher,
        }
    }

    /// Run the connection loop: recv messages, dispatch to agent, send results.
    pub async fn run(mut self) -> Result<(), ConnectionError> {
        while let Some(msg) = self.recv().await {
            match msg {
                Ok(InboundMessage::Submit { req_id, target_id, input, metadata }) => {
                    let mut request = AgentRequest::with_id(&req_id, &target_id, &input);
                    if let Some(meta) = metadata {
                        request.metadata = meta;
                    }
                    match self.dispatcher.submit(request) {
                        Ok(rx) => {
                            if let Ok(result) = rx.await {
                                self.send_result(&result).await?;
                            }
                        }
                        Err(e) => {
                            self.send_error(Some(req_id), e.to_string()).await?;
                        }
                    }
                }
                Ok(InboundMessage::Cancel { req_id }) => {
                    let cancelled = self.dispatcher.cancel(&req_id).await;
                    if !cancelled {
                        self.send_error(Some(req_id), "request not found or already executing".to_string()).await?;
                    }
                }
                Err(e) => {
                    self.send_error(None, format!("parse error: {}", e)).await?;
                }
            }
        }
        Ok(())
    }

    async fn send_error(&self, req_id: Option<String>, message: String) -> Result<(), ConnectionError> {
        let msg = OutboundMessage::Error { req_id, message };
        let json = serde_json::to_string(&msg).map_err(|e| ConnectionError::ParseError(e.to_string()))?;
        let mut socket = self.tx.lock().await;
        socket.send(Message::Text(json)).await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
        Ok(())
    }
}

#[async_trait]
impl Connection for WsConnection {
    fn protocol(&self) -> &str { "ws" }

    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>> {
        let mut socket = self.tx.lock().await;
        match socket.recv().await {
            Some(Ok(Message::Text(text))) => {
                match serde_json::from_str::<InboundMessage>(&text) {
                    Ok(msg) => Some(Ok(msg)),
                    Err(e) => Some(Err(ConnectionError::ParseError(e.to_string()))),
                }
            }
            Some(Ok(Message::Close(_))) => None,
            Some(Ok(_)) => None, // Ignore binary/ping/pong
            Some(Err(e)) => Some(Err(ConnectionError::WsReceiveError(e.to_string()))),
            None => None,
        }
    }

    async fn send_event(&self, event: &vol_llm_agent::AgentStreamEvent) -> Result<(), ConnectionError> {
        // AgentStreamEvent doesn't implement Serialize, construct JSON manually.
        let event_json = serialize_stream_event(event);
        let outbound = serde_json::json!({
            "type": "event",
            "event": event_json,
        });
        let json = serde_json::to_string(&outbound)
            .map_err(|e| ConnectionError::ParseError(e.to_string()))?;
        let mut socket = self.tx.lock().await;
        socket.send(Message::Text(json)).await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
        Ok(())
    }

    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError> {
        // RunResult contains non-serializable types.
        let result_json = match &result.response {
            Ok(resp) => serde_json::json!({
                "req_id": result.req_id,
                "target_id": result.target_id,
                "run_id": result.run_id,
                "response": {
                    "content": resp.content,
                    "run_id": resp.run_id,
                    "session_id": resp.session_id,
                    "iterations": resp.iterations,
                    "tool_calls": resp.tool_calls.iter().map(|t| serde_json::json!({
                        "tool_name": t.tool_name,
                        "arguments": t.arguments,
                        "result": t.result,
                        "iteration": t.iteration,
                        "success": t.success,
                    })).collect::<Vec<_>>(),
                    "error": resp.error,
                }
            }),
            Err(e) => serde_json::json!({
                "req_id": result.req_id,
                "target_id": result.target_id,
                "run_id": result.run_id,
                "response": { "error": e.to_string() },
            }),
        };
        let outbound = serde_json::json!({
            "type": "result",
            "result": result_json,
        });
        let json = serde_json::to_string(&outbound)
            .map_err(|e| ConnectionError::ParseError(e.to_string()))?;
        let mut socket = self.tx.lock().await;
        socket.send(Message::Text(json)).await
            .map_err(|e| ConnectionError::WsSendError(e.to_string()))?;
        Ok(())
    }
}

/// WebSocket server that serves WS connections.
pub struct WsServer {
    dispatcher: Arc<AgentDispatcher>,
    holder: Arc<crate::connection::ConnectionHolder>,
}

impl WsServer {
    pub fn new(dispatcher: Arc<AgentDispatcher>, holder: Arc<crate::connection::ConnectionHolder>) -> Self {
        Self { dispatcher, holder }
    }

    /// Create an axum router with WS endpoint at "/ws".
    pub fn into_axum_router(self) -> Router {
        let server = Arc::new(self);
        Router::new()
            .route("/ws", get(Self::ws_handler))
            .with_state(server)
    }

    async fn ws_handler(
        ws: WebSocketUpgrade,
        axum::extract::State(state): axum::extract::State<Arc<Self>>,
    ) -> impl IntoResponse {
        let dispatcher = state.dispatcher.clone();
        let holder = state.holder.clone();
        ws.on_upgrade(move |socket| async move {
            let conn = WsConnection::new(socket, dispatcher);
            // Attach to holder so events stream to this connection
            holder.attach(Arc::new(conn)).await;
        })
    }
}
```

- [ ] **Step 3: Fix protocol.rs for dynamic event serialization**

The `OutboundMessage::Event` needs to handle `AgentStreamEvent` which may not implement `Serialize`. Update `protocol.rs` to use `serde_json::Value` for the event field:

```rust
/// Messages sent to client (outbound).
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutboundMessage {
    Connected {
        agent_id: String,
    },
    Event {
        #[serde(flatten)]
        event: serde_json::Value,
    },
    Result {
        result: serde_json::Value,
    },
    Error {
        req_id: Option<String>,
        message: String,
    },
}
```

And remove the `Deserialize` derive since `OutboundMessage` is only serialized (never deserialized).

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/ crates/vol-llm-agent-channel/src/protocol.rs crates/vol-llm-agent-channel/src/lib.rs
git commit -m "feat: implement WebSocket transport"
```

---

### Task 7: Implement in-memory transport

**Files:**
- Create: `crates/vol-llm-agent-channel/src/transport/memory.rs`
- Modify: `crates/vol-llm-agent-channel/src/transport/mod.rs` (add memory module)

- [ ] **Step 1: Create transport/memory.rs**

```rust
// crates/vol-llm-agent-channel/src/transport/memory.rs

use std::sync::Arc;

use async_trait::async_trait;
use serde_json;
use tokio::sync::mpsc;

use crate::connection::Connection;
use crate::error::ConnectionError;
use crate::protocol::InboundMessage;
use crate::request::RunResult;
use super::ws::serialize_stream_event;

/// In-memory connection for local testing.
pub struct MemoryConnection {
    rx: mpsc::UnboundedReceiver<InboundMessage>,
    tx: mpsc::UnboundedSender<OutboundEnvelope>,
}

/// Wraps outbound messages for serialization.
struct OutboundEnvelope {
    data: serde_json::Value,
}

impl MemoryConnection {
    /// Create a pair: the connection + a handle for the test to send/recv.
    pub fn new() -> (Self, MemoryHandle) {
        let (in_tx, in_rx) = mpsc::unbounded_channel::<InboundMessage>();
        let (out_tx, out_rx) = mpsc::unbounded_channel::<OutboundEnvelope>();
        (
            Self { rx: in_rx, tx: out_tx },
            MemoryHandle { tx: in_tx, rx: out_rx },
        )
    }
}

#[async_trait]
impl Connection for MemoryConnection {
    fn protocol(&self) -> &str { "memory" }

    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>> {
        self.rx.recv().await
    }

    async fn send_event(&self, event: &vol_llm_agent::AgentStreamEvent) -> Result<(), ConnectionError> {
        let event_json = serialize_stream_event(event);
        let envelope = OutboundEnvelope {
            data: serde_json::json!({ "type": "event", "event": event_json }),
        };
        self.tx.send(envelope)
            .map_err(|e| ConnectionError::ChannelError(e.to_string()))
    }

    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError> {
        let result_json = match &result.response {
            Ok(resp) => serde_json::json!({
                "req_id": result.req_id,
                "target_id": result.target_id,
                "run_id": result.run_id,
                "response": {
                    "content": resp.content,
                    "run_id": resp.run_id,
                    "session_id": resp.session_id,
                    "iterations": resp.iterations,
                    "tool_calls": resp.tool_calls.iter().map(|t| serde_json::json!({
                        "tool_name": t.tool_name,
                        "arguments": t.arguments,
                        "result": t.result,
                        "iteration": t.iteration,
                        "success": t.success,
                    })).collect::<Vec<_>>(),
                    "error": resp.error,
                }
            }),
            Err(e) => serde_json::json!({
                "req_id": result.req_id,
                "target_id": result.target_id,
                "run_id": result.run_id,
                "response": { "error": e.to_string() },
            }),
        };
        let envelope = OutboundEnvelope {
            data: serde_json::json!({ "type": "result", "result": result_json }),
        };
        self.tx.send(envelope)
            .map_err(|e| ConnectionError::ChannelError(e.to_string()))
    }
}

/// Test handle for controlling the connection from tests.
pub struct MemoryHandle {
    tx: mpsc::UnboundedSender<InboundMessage>,
    rx: mpsc::UnboundedReceiver<OutboundEnvelope>,
}

impl MemoryHandle {
    /// Send an inbound message to the connection.
    pub fn send(&self, msg: InboundMessage) -> Result<(), &'static str> {
        self.tx.send(msg).map_err(|_| "connection closed")
    }

    /// Receive the next outbound message (with timeout).
    pub async fn recv(&mut self) -> Option<serde_json::Value> {
        self.rx.recv().await.map(|e| e.data)
    }
}
```

- [ ] **Step 2: Update transport/mod.rs**

```rust
// crates/vol-llm-agent-channel/src/transport/mod.rs

mod memory;
mod ws;

pub use memory::{MemoryConnection, MemoryHandle};
pub use ws::{WsConnection, WsServer};
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/memory.rs crates/vol-llm-agent-channel/src/transport/mod.rs crates/vol-llm-agent-channel/src/lib.rs
git commit -m "feat: implement in-memory transport"
```

---

### Task 8: Full workspace check and test run

**Files:** No file changes — verification step.

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: no errors

- [ ] **Step 2: Run all crate tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: all tests pass (existing 7 dispatcher/router tests + 4 connection holder tests = 11 tests)
