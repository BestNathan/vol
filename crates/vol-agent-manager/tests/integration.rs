//! Integration tests for vol-agent-manager HTTP API.
//!
//! These tests verify the REST API endpoints work end-to-end by wiring real
//! components (AgentStateManager, TaskDispatcher, EventBus, MetricsCollector)
//! and sending synthetic HTTP requests through the axum router.

use std::sync::Arc;

use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::{Request, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post, delete};
use axum::Router;
use http_body_util::BodyExt;
use tower::ServiceExt;
use vol_agent_manager::config::ManagerConfig;
use vol_agent_manager::events::EventBus;
use vol_agent_manager::metrics::collector::MetricsCollector;
use vol_agent_manager::state::manager::AgentStateManager;
use vol_agent_manager::state::models::{AgentState, AgentStatus, HostInfo};
use vol_agent_manager::instance::AgentInstanceRegistry;
use vol_agent_manager::task::dispatcher::TaskDispatcher;
use vol_llm_agent_channel::Message;
use vol_agent_manager::AppRouterState;

// ---------------------------------------------------------------------------
// Test fixture helpers
// ---------------------------------------------------------------------------

fn make_app_state() -> (Arc<AgentStateManager>, Arc<MetricsCollector>, Arc<EventBus>, Arc<TaskDispatcher>, AppRouterState) {
    let state_manager = Arc::new(AgentStateManager::new());
    let metrics = Arc::new(MetricsCollector::new());
    let event_bus = Arc::new(EventBus::new());
    let task_dispatcher = Arc::new(TaskDispatcher::new());
    let instance_registry = Arc::new(AgentInstanceRegistry::new());
    let agent_loader = Arc::new(vol_llm_agent::AgentLoader::new_empty());
    let llm_config = vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );
    let config = ManagerConfig::default();

    let app_state = AppRouterState {
        state_manager: state_manager.clone(),
        metrics: metrics.clone(),
        event_bus: event_bus.clone(),
        task_dispatcher: task_dispatcher.clone(),
        config,
        instance_registry,
        agent_loader,
        llm_config,
    };
    (state_manager, metrics, event_bus, task_dispatcher, app_state)
}

fn make_router() -> (Arc<AgentStateManager>, Arc<TaskDispatcher>, Router) {
    let (sm, _metrics, _event_bus, td, app_state) = make_app_state();
    let router = vol_agent_manager::ws::server::create_ws_router()
        .route("/health", get(health_handler))
        .route("/metrics", get(metrics_handler))
        .route("/api/v1/agents", get(list_agents_handler))
        .route("/api/v1/agents/:id", get(agent_handler))
        .route("/api/v1/agents/:id/tasks", post(dispatch_task_handler))
        .route("/api/v1/tasks/:id", get(task_handler))
        .route("/api/v1/tasks", get(list_tasks_handler))
        .with_state(app_state);
    (sm, td, router)
}

// ---------------------------------------------------------------------------
// Inline route handlers (mirror the expected server API)
// ---------------------------------------------------------------------------

async fn health_handler() -> impl IntoResponse {
    axum::Json(serde_json::json!({"status": "ok"}))
}

async fn metrics_handler(
    State(state): State<AppRouterState>,
) -> impl IntoResponse {
    use prometheus::Encoder;
    let encoder = prometheus::TextEncoder::new();
    let mut buffer = Vec::new();
    let metric_families = state.metrics.gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8_lossy(&buffer).into_owned()
}

async fn list_agents_handler(
    State(state): State<AppRouterState>,
) -> impl IntoResponse {
    let agents = state.state_manager.list_all().await;
    axum::Json(serde_json::json!({"agents": agents}))
}

async fn agent_handler(
    State(state): State<AppRouterState>,
    Path(id): Path<String>,
) -> Response<Body> {
    match state.state_manager.get(&id).await {
        Some(agent) => axum::Json(serde_json::json!({"agent": agent})).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "agent not found"})),
        ).into_response(),
    }
}

async fn dispatch_task_handler(
    State(state): State<AppRouterState>,
    Path(id): Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let task_type = body.get("task_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let parameters = body.get("parameters").cloned().unwrap_or(serde_json::json!({}));
    let task = state.task_dispatcher.create_task(&id, task_type, parameters, None).await;
    (StatusCode::ACCEPTED, axum::Json(serde_json::json!({"task_id": task.id})))
}

async fn task_handler(
    State(state): State<AppRouterState>,
    Path(id): Path<String>,
) -> Response<Body> {
    match state.task_dispatcher.get_task(&id).await {
        Some(task) => axum::Json(serde_json::json!({"task": task})).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "task not found"})),
        ).into_response(),
    }
}

async fn list_tasks_handler(
    State(state): State<AppRouterState>,
) -> impl IntoResponse {
    let tasks = state.task_dispatcher.list_tasks().await;
    axum::Json(serde_json::json!({"tasks": tasks}))
}

// ---------------------------------------------------------------------------
// Helper: make a GET request
// ---------------------------------------------------------------------------

fn get_request(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .body(Body::empty())
        .unwrap()
}

// ---------------------------------------------------------------------------
// Tests: Health endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_health_endpoint() {
    let (_sm, _td, router) = make_router();
    let response = router.oneshot(get_request("/health")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["status"], "ok");
}

// ---------------------------------------------------------------------------
// Tests: Agents API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_list_agents_empty() {
    let (_sm, _td, router) = make_router();
    let response = router.oneshot(get_request("/api/v1/agents")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["agents"].as_array().unwrap().len(), 0);
}

fn make_agent_state(id: &str, name: &str) -> AgentState {
    AgentState {
        agent_id: id.to_string(),
        name: name.to_string(),
        r#type: "react-agent".to_string(),
        version: "0.1.0".to_string(),
        capabilities: vec!["Read".to_string()],
        host_info: HostInfo {
            hostname: "host1".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            ip: "127.0.0.1".to_string(),
        },
        status: AgentStatus::Idle,
        connected_at: chrono::Utc::now(),
        last_heartbeat: chrono::Utc::now(),
    }
}

#[tokio::test]
async fn test_agent_crud_via_api() {
    let (sm, _td, router) = make_router();

    // Register an agent directly via state manager
    sm.register(make_agent_state("test-agent", "test")).await;

    // GET /api/v1/agents should return 1 agent
    let response = router
        .clone()
        .oneshot(get_request("/api/v1/agents"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["agents"].as_array().unwrap().len(), 1);

    // GET /api/v1/agents/test-agent should return the agent
    let response = router
        .oneshot(get_request("/api/v1/agents/test-agent"))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["agent"]["agent_id"], "test-agent");
}

#[tokio::test]
async fn test_get_agent_not_found() {
    let (_sm, _td, router) = make_router();
    let response = router
        .oneshot(get_request("/api/v1/agents/nonexistent"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tests: Agent state transitions (direct manager calls)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_agent_state_transitions() {
    let mgr = AgentStateManager::new();

    let state = make_agent_state("test-agent", "test");
    mgr.register(state).await;

    let got = mgr.get("test-agent").await.unwrap();
    assert_eq!(got.status, AgentStatus::Idle);

    mgr.update_status("test-agent", AgentStatus::Busy).await;
    let got = mgr.get("test-agent").await.unwrap();
    assert_eq!(got.status, AgentStatus::Busy);

    let all = mgr.list_all().await;
    assert_eq!(all.len(), 1);
}

// ---------------------------------------------------------------------------
// Tests: Tasks API
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_dispatch_task() {
    let (sm, _td, router) = make_router();

    // Register the agent first
    sm.register(make_agent_state("agent-1", "worker")).await;

    // POST /api/v1/agents/agent-1/tasks
    let body_str = serde_json::json!({
        "task_type": "run-query",
        "parameters": {"query": "SELECT 1"}
    });
    let body = Body::from(serde_json::to_string(&body_str).unwrap());
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/agents/agent-1/tasks")
        .header("content-type", "application/json")
        .body(body)
        .unwrap();

    let response = router.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let task_id = parsed["task_id"].as_str().unwrap().to_string();
    assert!(!task_id.is_empty());

    // GET /api/v1/tasks/<task_id> should return the task
    let request = get_request(&format!("/api/v1/tasks/{}", task_id));
    let response = router.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["task"]["agent_id"], "agent-1");
    assert_eq!(parsed["task"]["task_type"], "run-query");
}

#[tokio::test]
async fn test_list_tasks_empty() {
    let (_sm, _td, router) = make_router();
    let response = router.oneshot(get_request("/api/v1/tasks")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["tasks"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn test_list_tasks_with_entries() {
    let (_sm, td, router) = make_router();

    td.create_task("a", "t1", serde_json::json!({}), None).await;
    td.create_task("b", "t2", serde_json::json!({}), None).await;

    let response = router.oneshot(get_request("/api/v1/tasks")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["tasks"].as_array().unwrap().len(), 2);
}

#[tokio::test]
async fn test_get_task_not_found() {
    let (_sm, _td, router) = make_router();
    let response = router
        .oneshot(get_request("/api/v1/tasks/nonexistent"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ---------------------------------------------------------------------------
// Tests: Metrics endpoint
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_metrics_endpoint_returns_prometheus_format() {
    let (_sm, _td, router) = make_router();
    let response = router.oneshot(get_request("/metrics")).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(body.to_vec()).unwrap();
    // Should contain agent_manager prefix metrics
    assert!(text.contains("agent_manager"));
}

// ---------------------------------------------------------------------------
// Tests: Protocol message roundtrip (via vol-llm-agent-channel Message)
// ---------------------------------------------------------------------------

#[test]
fn test_protocol_message_roundtrip() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("type".to_string(), serde_json::json!("heartbeat"));
    let msg = Message::Submit {
        req_id: "req-1".to_string(),
        sender: "agent-1".to_string(),
        receiver: "manager".to_string(),
        input: serde_json::json!({"status": "Idle"}).to_string(),
        metadata: Some(metadata),
    };
    let serialized = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&serialized).unwrap();
    match parsed {
        Message::Submit { sender, metadata, .. } => {
            assert_eq!(sender, "agent-1");
            let meta = metadata.unwrap();
            assert_eq!(meta.get("type").and_then(|v| v.as_str()), Some("heartbeat"));
        }
        _ => panic!("expected Submit variant"),
    }
}

#[test]
fn test_protocol_control_command() {
    let mut metadata = std::collections::HashMap::new();
    metadata.insert("type".to_string(), serde_json::json!("execute-task"));
    metadata.insert("task_id".to_string(), serde_json::json!("task-123"));
    let msg = Message::Submit {
        req_id: "req-2".to_string(),
        sender: "manager".to_string(),
        receiver: "agent-1".to_string(),
        input: serde_json::json!({"cmd": "run"}).to_string(),
        metadata: Some(metadata),
    };
    let serialized = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&serialized).unwrap();
    match parsed {
        Message::Submit { metadata, receiver, .. } => {
            assert_eq!(receiver, "agent-1");
            let meta = metadata.unwrap();
            assert_eq!(meta.get("type").and_then(|v| v.as_str()), Some("execute-task"));
            assert_eq!(meta.get("task_id").and_then(|v| v.as_str()), Some("task-123"));
        }
        _ => panic!("expected Submit variant"),
    }
}

#[test]
fn test_protocol_error_message() {
    let msg = Message::Error {
        req_id: Some("req-3".to_string()),
        sender: "manager".to_string(),
        receiver: "agent-1".to_string(),
        message: "unauthorized".to_string(),
    };
    let serialized = serde_json::to_string(&msg).unwrap();
    let parsed: Message = serde_json::from_str(&serialized).unwrap();
    match parsed {
        Message::Error { sender, receiver, message, .. } => {
            assert_eq!(sender, "manager");
            assert_eq!(receiver, "agent-1");
            assert_eq!(message, "unauthorized");
        }
        _ => panic!("expected Error variant"),
    }
}

// ---------------------------------------------------------------------------
// Tests: Event bus
// ---------------------------------------------------------------------------

#[test]
fn test_event_bus_drain() {
    use vol_agent_manager::events::ManagerEvent;

    let bus = EventBus::new();
    bus.emit(ManagerEvent::agent_registered("agent-1"));
    bus.emit(ManagerEvent::agent_dead("agent-1"));
    let events = bus.drain();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].event_type, "agent_registered");
    assert_eq!(events[1].event_type, "agent_dead");
}

#[test]
fn test_event_bus_task_events() {
    use vol_agent_manager::events::ManagerEvent;

    let bus = EventBus::new();
    bus.emit(ManagerEvent::task_dispatched("task-1", "agent-1"));
    bus.emit(ManagerEvent::task_completed("task-1", "agent-1"));
    bus.emit(ManagerEvent::task_failed("task-2", "agent-2", "boom"));
    bus.emit(ManagerEvent::task_timeout("task-3", "agent-3"));
    let events = bus.drain();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].event_type, "task_dispatched");
    assert!(events[2].data.is_some()); // task_failed has data
}

// ---------------------------------------------------------------------------
// Tests: Task dispatcher lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_task_lifecycle() {
    use vol_agent_manager::task::dispatcher::TaskStatus;

    let td = TaskDispatcher::new();
    let task = td
        .create_task(
            "agent-1",
            "run-query",
            serde_json::json!({"query": "SELECT 1"}),
            None,
        )
        .await;
    let id = task.id.clone();

    // Initially pending
    assert_eq!(task.status, TaskStatus::Pending);

    // Dispatch
    td.update_status(&id, TaskStatus::Dispatched).await;
    let got = td.get_task(&id).await.unwrap();
    assert_eq!(got.status, TaskStatus::Dispatched);
    assert!(got.dispatched_at.is_some());

    // Complete
    td.complete_task(&id, Some(serde_json::json!({"rows": 1})), None).await;
    let got = td.get_task(&id).await.unwrap();
    assert_eq!(got.status, TaskStatus::Completed);
    assert!(got.completed_at.is_some());
    assert!(got.result.is_some());
}

#[tokio::test]
async fn test_task_failure_and_timeout() {
    use vol_agent_manager::task::dispatcher::TaskStatus;

    let td = TaskDispatcher::new();

    // Test failure
    let task1 = td.create_task("agent-1", "test", serde_json::json!({}), None).await;
    td.fail_task(&task1.id, "connection refused").await;
    let got = td.get_task(&task1.id).await.unwrap();
    assert_eq!(got.status, TaskStatus::Failed);
    assert!(got.error.is_some());

    // Test timeout
    let task2 = td.create_task("agent-1", "test", serde_json::json!({}), None).await;
    td.timeout_task(&task2.id).await;
    let got = td.get_task(&task2.id).await.unwrap();
    assert_eq!(got.status, TaskStatus::Timeout);
}

// ---------------------------------------------------------------------------
// Tests: Config defaults
// ---------------------------------------------------------------------------

#[test]
fn test_config_defaults() {
    let config = ManagerConfig::default();
    assert_eq!(config.server.listen_addr, "0.0.0.0:8080");
    assert_eq!(config.health.check_interval_secs, 15);
    assert_eq!(config.health.heartbeat_timeout_secs, 90);
    assert_eq!(config.health.disconnect_retention_secs, 300);
    assert!(config.security.token.is_none());
}

// ---------------------------------------------------------------------------
// Tests: Multiple agents
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_multiple_agents_list() {
    let (sm, _td, router) = make_router();

    sm.register(make_agent_state("agent-a", "Alpha")).await;
    sm.register(make_agent_state("agent-b", "Beta")).await;
    sm.register(make_agent_state("agent-c", "Gamma")).await;

    let response = router.oneshot(get_request("/api/v1/agents")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let parsed: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(parsed["agents"].as_array().unwrap().len(), 3);
}

// ---------------------------------------------------------------------------
// Tests: Agent WS routing and new REST API (agent-loader integration)
// ---------------------------------------------------------------------------

/// Build a test app with agent-loader-backed routes.
async fn build_test_app() -> Router {
    use axum::extract::State;

    let state_manager = Arc::new(AgentStateManager::new());
    let metrics = Arc::new(MetricsCollector::new());
    let event_bus = Arc::new(EventBus::new());
    let task_dispatcher = Arc::new(TaskDispatcher::new());
    let instance_registry = Arc::new(AgentInstanceRegistry::new());
    let agent_loader = Arc::new(vol_llm_agent::AgentLoader::new_empty());
    let llm_config = vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );
    let config = ManagerConfig::default();

    let app_state = AppRouterState {
        state_manager,
        metrics,
        event_bus,
        task_dispatcher,
        config,
        instance_registry,
        agent_loader,
        llm_config,
    };

    // Only include routes we can test (lib-based handlers)
    vol_agent_manager::ws::server::create_ws_router()
        .route("/api/v1/agent-types", get(|State(s): State<AppRouterState>| async move {
            let metadata = s.agent_loader.list_metadata().await;
            axum::Json(serde_json::json!({
                "agent_types": metadata.iter().map(|m| serde_json::json!({
                    "name": m.name,
                    "type": m.r#type,
                    "description": m.description,
                    "scope": format!("{:?}", m.scope),
                })).collect::<Vec<_>>()
            }))
        }))
        .route("/api/v1/agent-instances", get(|State(s): State<AppRouterState>| async move {
            let instances = s.instance_registry.list_instances().await;
            axum::Json(serde_json::json!({ "instances": instances }))
        }))
        .route("/api/v1/agent-instances/:type/:session_id", delete(|State(s): State<AppRouterState>, axum::extract::Path((t, s_id)): axum::extract::Path<(String, String)>| async move {
            s.instance_registry.destroy(&t, &s_id).await;
            (axum::http::StatusCode::NO_CONTENT, ())
        }))
        .with_state(app_state)
}

#[tokio::test]
async fn test_agent_ws_router_rejects_non_ws_request() {
    let app = build_test_app().await;

    // Non-WebSocket HTTP requests to the WS endpoint get 400 (Bad Request)
    // because axum rejects non-upgrade requests on WS routes.
    // Unknown agent types would get 404 after a successful WS upgrade.
    let response = app
        .oneshot(
            Request::builder()
                .uri("/ws/agents/unknown-type/session/test-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn test_list_agent_types_empty() {
    let app = build_test_app().await;

    let response = app
        .oneshot(get_request("/api/v1/agent-types"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["agent_types"], serde_json::json!([]));
}

#[tokio::test]
async fn test_list_agent_instances_empty() {
    let app = build_test_app().await;

    let response = app
        .oneshot(get_request("/api/v1/agent-instances"))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["instances"], serde_json::json!([]));
}
