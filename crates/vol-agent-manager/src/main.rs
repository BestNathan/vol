use std::sync::Arc;

use anyhow::Result;
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{get, post},
};
use axum::response::sse::{Event, Sse};
use futures::stream::Stream;
use prometheus::{Encoder, TextEncoder};
use tracing::info;
use vol_agent_manager::config::ManagerConfig;
use vol_agent_manager::events::{EventBus, ManagerEvent};
use vol_agent_manager::health::HealthChecker;
use vol_agent_manager::metrics::collector::MetricsCollector;
use vol_agent_manager::state::manager::AgentStateManager;
use vol_agent_manager::task::dispatcher::{TaskDispatcher, TaskStatus};
use vol_agent_manager::ws::server::create_ws_router;
use vol_agent_manager::AppRouterState;

fn parse_args() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--config" || args[i] == "-c" {
            if i + 1 < args.len() {
                return Some(args[i + 1].clone());
            }
            eprintln!("Error: --config requires a file path");
            std::process::exit(1);
        }
        i += 1;
    }
    None
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "vol_agent_manager=info,tower_http=info".into()),
        )
        .init();

    let config_path = parse_args().unwrap_or_else(|| "config.toml".to_string());
    let config = ManagerConfig::from_path(&config_path)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to load {}: {}, using defaults", config_path, e);
            ManagerConfig::default()
        });

    let state_manager = Arc::new(AgentStateManager::new());
    let metrics = Arc::new(MetricsCollector::new());
    let event_bus = Arc::new(EventBus::new());
    let task_dispatcher = Arc::new(TaskDispatcher::new());

    let agent_loader = Arc::new(vol_llm_agent::AgentLoader::new(None));
    if let Err(e) = agent_loader.discover_all().await {
        tracing::warn!(error = %e, "Failed to discover agent definitions");
    }
    let instance_registry = Arc::new(vol_agent_manager::instance::AgentInstanceRegistry::new());

    let app_state = AppRouterState {
        state_manager: state_manager.clone(),
        metrics: metrics.clone(),
        event_bus: event_bus.clone(),
        task_dispatcher: task_dispatcher.clone(),
        config: config.clone(),
        instance_registry: instance_registry.clone(),
        agent_loader: agent_loader.clone(),
    };

    // Build router
    let app = create_ws_router()
        .route("/health", get(health_check))
        .route("/metrics", get(metrics_handler))
        .route("/api/v1/agents", get(list_agents))
        .route("/api/v1/agents/:id", get(get_agent))
        .route("/api/v1/agents/:id/tasks", post(dispatch_task))
        .route("/api/v1/tasks/:id", get(get_task))
        .route("/api/v1/tasks", get(list_tasks))
        .route("/api/v1/events", get(events_handler))
        .with_state(app_state.clone());

    // Start health checker in background
    let checker = HealthChecker::new(
        state_manager.clone(),
        std::time::Duration::from_secs(config.health.check_interval_secs),
        std::time::Duration::from_secs(config.health.heartbeat_timeout_secs),
        Some(event_bus.clone()),
    );
    tokio::spawn(Arc::new(checker).run_loop());

    info!("vol-agent-manager listening on {}", config.server.listen_addr);

    let listener = tokio::net::TcpListener::bind(&config.server.listen_addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    axum::Json(serde_json::json!({"status": "ok"}))
}

async fn metrics_handler(State(state): State<AppRouterState>) -> impl IntoResponse {
    let encoder = TextEncoder::new();
    let metric_families = state.metrics.gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    (
        [(axum::http::header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        String::from_utf8(buffer).unwrap(),
    )
}

async fn list_agents(State(state): State<AppRouterState>) -> impl IntoResponse {
    let agents = state.state_manager.list_all().await;
    axum::Json(serde_json::json!({"agents": agents}))
}

async fn get_agent(
    State(state): State<AppRouterState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.state_manager.get(&id).await {
        Some(agent) => (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"agent": agent}))),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "agent not found"})),
        ),
    }
}

async fn events_handler(State(state): State<AppRouterState>) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.event_bus.subscribe();
    let stream = async_stream::stream! {
        let mut rx = rx;
        while let Ok(event) = rx.recv().await {
            yield Ok(Event::default().data(event.to_json_string()));
        }
    };
    Sse::new(stream)
}

async fn dispatch_task(
    State(state): State<AppRouterState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    axum::Json(body): axum::Json<serde_json::Value>,
) -> impl IntoResponse {
    let task_type = body.get("task_type").and_then(|v| v.as_str()).unwrap_or("unknown");
    let parameters = body.get("parameters").cloned().unwrap_or(serde_json::json!({}));
    let timeout_secs = body.get("timeout_seconds").and_then(|v| v.as_u64());
    let timeout = timeout_secs.map(std::time::Duration::from_secs);

    let task = state.task_dispatcher
        .create_task(&id, task_type, parameters, timeout)
        .await;
    let task_id = task.id.clone();
    state.task_dispatcher.update_status(&task_id, TaskStatus::Dispatched).await;
    state.event_bus.emit(ManagerEvent::task_dispatched(&task_id, &id));

    (axum::http::StatusCode::ACCEPTED, axum::Json(serde_json::json!({"task_id": task_id})))
}

async fn get_task(
    State(state): State<AppRouterState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.task_dispatcher.get_task(&id).await {
        Some(task) => (axum::http::StatusCode::OK, axum::Json(serde_json::json!({"task": task}))),
        None => (
            axum::http::StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error": "task not found"})),
        ),
    }
}

async fn list_tasks(State(state): State<AppRouterState>) -> impl IntoResponse {
    let tasks = state.task_dispatcher.list_tasks().await;
    axum::Json(serde_json::json!({"tasks": tasks}))
}
