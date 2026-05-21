//! HTTP transport for agent channel communication.
//!
//! Provides `HttpTransport` which returns an axum `Router` that can be
//! mounted at any path. Supports blocking and SSE streaming modes via `?stream`.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::sse::{Event, Sse};
use axum::response::IntoResponse;
use axum::routing::post;
use axum::{Json, Router};
use serde::Deserialize;
use tokio::sync::broadcast;

use crate::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload,
};
use crate::connection::{Connection, ConnectionHolder};
use crate::dispatcher::AgentDispatcher;
use crate::error::ConnectionError;
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
    tx: broadcast::Sender<AgentServerMessage>,
}

impl HttpEventConnection {
    pub fn new(tx: broadcast::Sender<AgentServerMessage>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl Connection for HttpEventConnection {
    fn protocol(&self) -> &str {
        "http"
    }

    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> {
        // HTTP is request-response; no inbound messages after POST.
        None
    }

    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError> {
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
    /// ```rust,ignore
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
        Err(_) => error_response(
            500,
            "dispatcher dropped while processing request".to_string(),
        ),
    }
}

/// SSE mode: stream events via Server-Sent Events.
async fn handle_sse(
    transport: Arc<HttpTransport>,
    agent_id: String,
    body: HttpRequestBody,
) -> axum::response::Response {
    // Reject if a connection is already active (concurrent SSE requests).
    if transport.holder.is_connected().await {
        return error_response(409, "another SSE connection is already active".to_string());
    }

    let req = build_request(&agent_id, &body);

    // Create broadcast channel for event capture.
    let (event_tx, mut event_rx) = broadcast::channel::<AgentServerMessage>(100);

    // Create and attach HTTP event connection.
    let conn = HttpEventConnection::new(event_tx.clone());
    transport.holder.attach(Arc::new(conn)).await;

    // Submit request to dispatcher.
    let rx = match transport.dispatcher.submit(req) {
        Ok(rx) => rx,
        Err(e) => {
            transport.holder.detach().await;
            return error_response(500, e.to_string());
        }
    };

    // Use oneshot to signal completion with final result.
    let (done_tx, done_rx) = tokio::sync::oneshot::channel::<AgentServerMessage>();

    // Spawn task to await dispatcher result and send final event.
    tokio::spawn(async move {
        match rx.await {
            Ok(run_result) => {
                let result_value = match &run_result.response {
                    Ok(resp) => serde_json::to_value(resp).unwrap_or(serde_json::Value::Null),
                    Err(err) => serde_json::json!({ "error": err.to_string() }),
                };
                let msg = AgentServerMessage::new_result(
                    run_result.run_id.clone(),
                    Operation::Agent(AgentOperation::Submit),
                    Payload::Agent(AgentPayload::SubmitResult {
                        run_id: run_result.run_id,
                        response: result_value,
                    }),
                );
                let _ = done_tx.send(msg);
            }
            Err(_) => {
                let msg = AgentServerMessage::new_error(
                    String::new(),
                    Operation::Agent(AgentOperation::Submit),
                    crate::agent_server_protocol::ErrorPayload {
                        code: "dispatcher_dropped".to_string(),
                        message: "dispatcher dropped while processing request".to_string(),
                        detail: None,
                        terminal: true,
                    },
                );
                let _ = done_tx.send(msg);
            }
        }
    });

    let holder = transport.holder.clone();

    // Merge event stream with done signal.
    let merged_stream = async_stream::stream! {
        let mut event_tx = Some(event_tx);
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
                    // Drop event_tx so event_rx returns Closed after draining buffer.
                    event_tx.take();
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

        // Detach from ConnectionHolder after stream ends.
        holder.detach().await;
    };

    Sse::new(merged_stream).into_response()
}

fn build_request(agent_id: &str, body: &HttpRequestBody) -> AgentRequest {
    let mut request = AgentRequest::new(agent_id, &body.input);
    if let Some(req_id) = &body.req_id {
        request.run_id = req_id.clone();
    }
    if let Some(meta) = &body.metadata {
        request.metadata = meta.clone();
    }
    request
}

fn error_response(status: u16, message: String) -> axum::response::Response {
    let body = serde_json::json!({ "error": message });
    (
        StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
        Json(body),
    )
        .into_response()
}

fn run_result_response(run_result: RunResult) -> axum::response::Response {
    match run_result.response {
        Ok(resp) => {
            let body = serde_json::json!({
                "run_id": run_result.run_id,
                "success": true,
                "response": serde_json::to_value(resp).unwrap_or(serde_json::Value::Null),
            });
            (StatusCode::OK, Json(body)).into_response()
        }
        Err(e) => {
            let body = serde_json::json!({
                "run_id": run_result.run_id,
                "target_id": run_result.target_id,
                "success": false,
                "error": e.to_string(),
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(body)).into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::util::ServiceExt;
    use vol_llm_agent::agent_def::AgentDef;
    use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
    use vol_llm_context::ContextBuilderBuilder;
    use vol_llm_core::{
        ConversationRequest, ConversationResponse, FinishReason, LLMClient, LLMProvider, Message,
        StreamReceiver, SupportedParam, TokenUsage,
    };
    use vol_llm_tool::ToolRegistry;
    use vol_session::{InMemoryEntryStore, Session};

    struct MockLlm;
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "mock"
        }
        fn supported_params(&self) -> &[SupportedParam] {
            &[]
        }
        async fn converse(
            &self,
            _: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            Ok(ConversationResponse {
                message: Message::assistant("mock response".to_string()),
                model: "mock".to_string(),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }
        async fn converse_stream(
            &self,
            _: ConversationRequest,
        ) -> vol_llm_core::Result<StreamReceiver> {
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
            mcp_manager: None,
            agent_id: "test_agent".to_string(),
            working_dir: std::path::PathBuf::from("/tmp"),
        };
        let agent = ReActAgent::new(config);
        let dispatcher = Arc::new(AgentDispatcher::new(agent));
        let holder = Arc::new(ConnectionHolder::new(
            "test_agent".to_string(),
            "client".to_string(),
        ));
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
        let content_type = response
            .headers()
            .get("content-type")
            .unwrap()
            .to_str()
            .unwrap();
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

    #[tokio::test]
    async fn test_http_transport_concurrent_sse_returns_409() {
        use std::time::Duration;
        use tokio::net::TcpListener;

        // Create a transport with a slow LLM so the first SSE request stays active.
        let def = AgentDef::new("slow_agent", "You are slow.").with_type("slow_agent");
        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
        let tools = Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: Some(def),
            llm: Arc::new(slow_llm::SlowMockLlm),
            tools,
            session,
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
            mcp_manager: None,
            agent_id: "slow_agent".to_string(),
            working_dir: std::path::PathBuf::from("/tmp"),
        };
        let agent = ReActAgent::new(config);
        let dispatcher = Arc::new(AgentDispatcher::new(agent));
        let holder = Arc::new(ConnectionHolder::new(
            "slow_agent".to_string(),
            "client".to_string(),
        ));
        let transport = HttpTransport::new(dispatcher, holder, "slow_agent");
        let app = transport.into_axum_router();

        // Bind to a random port and serve concurrently.
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // Give the server time to start.
        tokio::time::sleep(Duration::from_millis(50)).await;

        let client = reqwest::Client::new();

        let body = serde_json::json!({ "input": "hello" });
        let url = format!("http://127.0.0.1:{}/?stream=true", port);

        let handle1 = tokio::spawn({
            let client = client.clone();
            let url = url.clone();
            let body = body.clone();
            async move { client.post(&url).json(&body).send().await }
        });

        // Give the first request time to attach its connection.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Second SSE request should get 409.
        let resp2 = client.post(&url).json(&body).send().await.unwrap();
        assert_eq!(resp2.status(), 409);

        // The first request may still be running (slow LLM takes 5s).
        // Even if it completed, the key assertion (409 for concurrent) is verified.
    }
}

/// A mock LLM that takes 5 seconds to respond, for testing concurrent SSE requests.
#[cfg(test)]
mod slow_llm {
    use super::*;
    use vol_llm_core::{
        ConversationRequest, ConversationResponse, FinishReason, LLMClient, LLMProvider, Message,
        StreamReceiver, SupportedParam, TokenUsage,
    };

    pub struct SlowMockLlm;
    #[async_trait::async_trait]
    impl LLMClient for SlowMockLlm {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "slow-mock"
        }
        fn supported_params(&self) -> &[SupportedParam] {
            &[]
        }
        async fn converse(
            &self,
            _: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            Ok(ConversationResponse {
                message: Message::assistant("slow response".to_string()),
                model: "slow-mock".to_string(),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }
        async fn converse_stream(
            &self,
            _: ConversationRequest,
        ) -> vol_llm_core::Result<StreamReceiver> {
            let (_tx, rx) = tokio::sync::mpsc::channel(10);
            Ok(StreamReceiver::new(rx))
        }
    }
}
