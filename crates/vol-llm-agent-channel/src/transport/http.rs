//! HTTP transport for agent channel communication.
//!
//! Provides `HttpTransport` which returns an axum `Router` that can be
//! mounted at any path. Supports blocking and SSE streaming modes via `?stream`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::Query;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use tokio::sync::broadcast;

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
#[allow(dead_code)]
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

    // Use oneshot to signal completion with final result.
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

        loop {
            tokio::select! {
                biased;
                result = &mut done_fut => {
                    if let Ok(msg) = result {
                        if let Ok(json) = serde_json::to_string(&msg) {
                            yield Ok::<_, std::convert::Infallible>(Event::default().data(json));
                        }
                    }
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
