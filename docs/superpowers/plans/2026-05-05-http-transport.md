# HTTP Transport Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add HTTP transport to `vol-llm-agent-channel` — a reusable axum handler that supports blocking and SSE streaming modes, mountable at any path.

**Architecture:** `HttpTransport` struct holds `dispatcher`, `holder`, and `agent_id`. Its `into_axum_router()` returns an axum Router with a POST handler. Blocking mode awaits `dispatcher.submit()` result; SSE mode creates a temporary `HttpEventConnection` attached to `ConnectionHolder` to capture events, streams them as SSE, then sends final result.

**Tech Stack:** Rust, axum 0.7 (with `axum::response::sse`), tokio, serde_json, futures

---

### Task 1: Add `HttpEventConnection` and `HttpTransport` struct

**Files:**
- Create: `crates/vol-llm-agent-channel/src/transport/http.rs`

- [ ] **Step 1: Create `transport/http.rs` with imports and structs**

Create the file with this content:

```rust
//! HTTP transport for agent channel communication.
//!
//! Provides `HttpTransport` which returns an axum `Router` that can be
//! mounted at any path. Supports blocking and SSE streaming modes via `?stream`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::Query;
use axum::response::sse::{Event, Sse};
use axum::routing::post;
use axum::{Json, Router};
use futures::stream::{Stream, StreamExt};
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};

use crate::connection::{Connection, ConnectionHolder};
use crate::dispatcher::AgentDispatcher;
use crate::error::ConnectionError;
use crate::protocol::Message;
use crate::request::{AgentRequest, RunResult};

/// Query parameter for SSE streaming mode.
#[derive(Deserialize)]
struct StreamQuery {
    stream: Option<bool>,
}

/// A connection that forwards events to a broadcast channel.
///
/// This implements the `Connection` trait purely for `ConnectionHolder`
/// integration. `recv()` always returns `None` since HTTP is request-response.
pub struct HttpEventConnection {
    tx: broadcast::Sender<Message>,
    sender: String,
    receiver: String,
}

impl HttpEventConnection {
    pub fn new(tx: broadcast::Sender<Message>, sender: String, receiver: String) -> Self {
        Self { tx, sender, receiver }
    }
}

#[async_trait]
impl Connection for HttpEventConnection {
    fn protocol(&self) -> &str {
        "http"
    }

    async fn recv(&mut self) -> Option<Result<Message, ConnectionError>> {
        // HTTP is request-response; no inbound messages after POST.
        None
    }

    async fn send(&self, msg: Message) -> Result<(), ConnectionError> {
        // Only forward Event messages to the broadcast channel.
        // Ignore other message types (Connected, Result, Error) — those
        // are handled by the dispatcher result path.
        self.tx
            .send(msg)
            .map_err(|e| ConnectionError::ChannelError(e.to_string()))?;
        Ok(())
    }
}

/// HTTP transport that provides reusable axum handlers.
///
/// Unlike `WsServer` which creates a fixed `/ws` route, `HttpTransport`
/// returns a Router that users can `.merge()` at any path of their own service.
pub struct HttpTransport {
    dispatcher: Arc<AgentDispatcher>,
    holder: Arc<ConnectionHolder>,
    agent_id: String,
}

impl HttpTransport {
    /// Create a new HTTP transport.
    pub fn new(
        dispatcher: Arc<AgentDispatcher>,
        holder: Arc<ConnectionHolder>,
        agent_id: impl Into<String>,
    ) -> Self {
        Self {
            dispatcher,
            holder,
            agent_id: agent_id.into(),
        }
    }

    /// Build an axum `Router` with a POST endpoint.
    ///
    /// Users can merge this router at any path:
    /// ```rust
    /// let app = Router::new()
    ///     .nest("/api/chat", http_transport.into_axum_router());
    /// ```
    pub fn into_axum_router(self) -> Router {
        let agent_id = self.agent_id.clone();
        let transport = Arc::new(self);

        Router::new().route(
            "/",
            post({
                let transport = transport.clone();
                let agent_id = agent_id.clone();
                move |query, body| handle_post(transport, agent_id, query, body)
            }),
        )
    }
}

/// Request body for HTTP POST.
#[derive(Debug, Deserialize)]
struct HttpRequestBody {
    input: String,
    #[serde(default)]
    req_id: Option<String>,
    #[serde(default)]
    metadata: Option<HashMap<String, serde_json::Value>>,
}

/// Handle a POST request — dispatch to blocking or SSE mode.
async fn handle_post(
    transport: Arc<HttpTransport>,
    agent_id: String,
    Query(query): Query<StreamQuery>,
    Json(body): Json<HttpRequestBody>,
) -> axum::response::Response {
    if query.stream.unwrap_or(false) {
        handle_sse(transport, agent_id, body).await
    } else {
        handle_blocking(transport, agent_id, body).await
    }
}

/// Blocking mode: submit request, await result, return JSON.
async fn handle_blocking(
    transport: Arc<HttpTransport>,
    agent_id: String,
    body: HttpRequestBody,
) -> axum::response::Response {
    let req = build_request(&agent_id, &body);

    let rx = match transport.dispatcher.submit(req) {
        Ok(rx) => rx,
        Err(e) => {
            return error_response(500, e.to_string());
        }
    };

    match rx.await {
        Ok(run_result) => run_result_response(run_result),
        Err(_) => error_response(500, "dispatcher dropped while processing request".to_string()),
    }
}

/// SSE mode: stream events via Server-Sent Events.
async fn handle_sse(
    transport: Arc<HttpTransport>,
    agent_id: String,
    body: HttpRequestBody,
) -> axum::response::Response {
    let req = build_request(&agent_id, &body);

    // Create broadcast channel for event capture.
    let (event_tx, mut event_rx) = broadcast::channel::<Message>(100);

    // Create and attach HTTP event connection.
    let conn = HttpEventConnection::new(
        event_tx,
        agent_id.clone(),
        "client".to_string(),
    );
    transport.holder.attach(Arc::new(conn)).await;

    // Submit request to dispatcher.
    let rx = match transport.dispatcher.submit(req) {
        Ok(rx) => rx,
        Err(e) => {
            return error_response(500, e.to_string());
        }
    };

    // Convert broadcast receiver to a stream.
    let event_stream = async_stream::stream! {
        loop {
            match event_rx.recv().await {
                Ok(msg) => {
                    if let Ok(json) = serde_json::to_string(&msg) {
                        yield Ok(Event::default().data(json));
                    }
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    yield Ok(Event::default().data(format!("{{\"warning\":\"lagged {} events\"}}", n)));
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
                }
            }
        }
    };

    // After the agent run completes, send the final result event.
    // We need to combine the event stream with the final result.
    // Use a oneshot to signal completion.
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<Message>();

    // Spawn task to await dispatcher result and send final event.
    tokio::spawn(async move {
        match rx.await {
            Ok(run_result) => {
                let result_value = match &run_result.response {
                    Ok(resp) => serde_json::to_value(resp).unwrap_or(serde_json::Value::Null),
                    Err(err) => serde_json::json!({ "error": err.to_string() }),
                };
                let msg = Message::Result {
                    req_id: run_result.req_id,
                    sender: agent_id,
                    receiver: "client".to_string(),
                    result: result_value,
                };
                let _ = done_tx.send(msg);
            }
            Err(_) => {
                let msg = Message::Error {
                    req_id: None,
                    sender: agent_id,
                    receiver: "client".to_string(),
                    message: "dispatcher dropped while processing request".to_string(),
                };
                let _ = done_tx.send(msg);
            }
        }
    });

    // Merge event stream with done signal.
    let merged_stream = async_stream::stream! {
        let mut done_fut = done_rx;
        let mut done_received = false;

        loop {
            tokio::select! {
                biased;
                result = &mut done_fut => {
                    if let Ok(msg) = result {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            yield Ok::<_, std::convert::Infallible>(Event::default().data(json));
                        }
                    }
                    done_received = true;
                    // Continue draining remaining events briefly, then break.
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    break;
                }
                result = event_rx.recv() => {
                    match result {
                        Ok(msg) => {
                            if let Ok(json) = serde_json::to_string(&msg) {
                                yield Ok(Event::default().data(json));
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            yield Ok(Event::default().data(format!("{{\"warning\":\"lagged {} events\"}}", n)));
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
    };

    Sse::new(merged_stream).into_response()
}

fn build_request(agent_id: &str, body: &HttpRequestBody) -> AgentRequest {
    let mut request = AgentRequest::new(agent_id, &body.input);
    if let Some(req_id) = &body.req_id {
        request.req_id = req_id.clone();
    }
    if let Some(meta) = &body.metadata {
        request.metadata = meta.clone();
    }
    request
}

fn error_response(status: u16, message: String) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    let body = serde_json::json!({ "error": message });
    (StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR), Json(body)).into_response()
}

fn run_result_response(run_result: RunResult) -> axum::response::Response {
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    match run_result.response {
        Ok(resp) => {
            let body = serde_json::json!({
                "req_id": run_result.req_id,
                "target_id": run_result.target_id,
                "run_id": run_result.run_id,
                "success": true,
                "response": serde_json::to_value(resp).unwrap_or(serde_json::Value::Null),
            });
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            let body = serde_json::json!({
                "req_id": run_result.req_id,
                "target_id": run_result.target_id,
                "success": false,
                "error": e.to_string(),
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
        }
    }
}
```

- [ ] **Step 2: Add `async-stream` dependency**

Add to `crates/vol-llm-agent-channel/Cargo.toml` under `[dependencies]`:

```toml
async-stream = "0.3"
```

Also update workspace `Cargo.toml` axum entry to include the SSE feature:

Change:
```toml
axum = { version = "0.7", features = ["ws"] }
```
to:
```toml
axum = { version = "0.7", features = ["ws", "query"] }
```

- [ ] **Step 3: Verify compile**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | grep -v "^warning"`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/http.rs
git add crates/vol-llm-agent-channel/Cargo.toml
git commit -m "feat: add HTTP transport with blocking and SSE modes"
```

---

### Task 2: Export `HttpTransport` from module and crate root

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/transport/mod.rs`
- Modify: `crates/vol-llm-agent-channel/src/lib.rs`

- [ ] **Step 1: Update `transport/mod.rs`**

Change the file to:

```rust
mod http;
mod memory;
mod ws;

pub use http::HttpTransport;
pub use memory::{MemoryConnection, MemoryHandle};
pub use ws::{WsConnection, WsServer};
```

- [ ] **Step 2: Update `lib.rs`**

Change the last line from:
```rust
pub use transport::{MemoryConnection, MemoryHandle, WsConnection, WsServer};
```
to:
```rust
pub use transport::{HttpTransport, MemoryConnection, MemoryHandle, WsConnection, WsServer};
```

- [ ] **Step 3: Verify compile**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | grep -v "^warning"`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/mod.rs crates/vol-llm-agent-channel/src/lib.rs
git commit -m "feat: export HttpTransport from crate root"
```

---

### Task 3: Add tests for HTTP transport

**Files:**
- Create: `crates/vol-llm-agent-channel/src/transport/http.rs` (add `#[cfg(test)]` module at bottom)

- [ ] **Step 1: Add test module to `http.rs`**

Append this at the bottom of `crates/vol-llm-agent-channel/src/transport/http.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use crate::connection::ConnectionHolder;
    use crate::dispatcher::AgentDispatcher;
    use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
    use vol_llm_agent::agent_def::AgentDef;
    use vol_llm_context::ContextBuilderBuilder;
    use vol_session::{InMemoryEntryStore, Session};
    use vol_llm_tool::ToolRegistry;
    use vol_llm_core::{ConversationRequest, ConversationResponse, LLMClient, LLMProvider, StreamReceiver, SupportedParam};

    struct MockLlm;
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        fn supported_params(&self) -> &[SupportedParam] { &[] }
        async fn converse(&self, _: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
            Ok(ConversationResponse {
                content: "mock response".to_string(),
                usage: None,
                finish_reason: vol_llm_core::FinishReason::Stop,
            })
        }
        async fn converse_stream(&self, _: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
            let (_tx, rx) = tokio::sync::mpsc::channel(10);
            Ok(StreamReceiver::new(rx))
        }
    }

    fn make_test_transport() -> HttpTransport {
        let def = AgentDef::new("test_agent", "You are a test agent.").with_type("test_agent");
        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
        let tools = Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: Some(def),
            llm: Arc::new(MockLlm),
            tools,
            session,
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
        };
        let agent = ReActAgent::new(config);
        let dispatcher = Arc::new(AgentDispatcher::new(agent));
        let holder = Arc::new(ConnectionHolder::new("test_agent".to_string(), "client".to_string()));
        HttpTransport::new(dispatcher, holder, "test_agent")
    }

    #[tokio::test]
    async fn test_http_transport_blocking_returns_result() {
        let transport = make_test_transport();
        let app = transport.into_axum_router();

        let body = serde_json::json!({ "input": "hello" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert!(json.get("success").and_then(|v| v.as_bool()).unwrap());
        assert!(json.get("response").is_some());
    }

    #[tokio::test]
    async fn test_http_transport_sse_returns_events() {
        let transport = make_test_transport();
        let app = transport.into_axum_router();

        let body = serde_json::json!({ "input": "hello" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/?stream=true")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        // Verify response is SSE (content-type should contain text/event-stream)
        let content_type = response.headers().get("content-type").unwrap().to_str().unwrap();
        assert!(content_type.contains("text/event-stream"));
    }

    #[tokio::test]
    async fn test_http_transport_invalid_body_returns_400() {
        let transport = make_test_transport();
        let app = transport.into_axum_router();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header("content-type", "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_http_transport_empty_input_succeeds() {
        let transport = make_test_transport();
        let app = transport.into_axum_router();

        let body = serde_json::json!({ "input": "" });
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/")
                    .header("content-type", "application/json")
                    .body(Body::from(serde_json::to_string(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
        assert!(json.get("success").and_then(|v| v.as_bool()).unwrap());
    }
}
```

- [ ] **Step 2: Add test dependencies**

Add to `crates/vol-llm-agent-channel/Cargo.toml` under `[dev-dependencies]`:

```toml
tower = "0.4"
http-body-util = "0.1"
vol-session = { path = "../vol-session" }
vol-llm-context = { path = "../vol-llm-context" }
vol-llm-tool = { path = "../vol-llm-tool" }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agent-channel http -- --nocapture`
Expected: 4 tests pass

- [ ] **Step 4: Run full test suite**

Run: `cargo test -p vol-llm-agent-channel 2>&1 | tail -5`
Expected: all tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/transport/http.rs crates/vol-llm-agent-channel/Cargo.toml
git commit -m "test: add HTTP transport tests for blocking, SSE, error, and empty input"
```
