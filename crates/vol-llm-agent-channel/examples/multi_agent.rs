//! Multi-agent service using Agent Server Protocol transports.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example multi_agent -p vol-llm-agent-channel
//! ```
//!
//! Endpoints:
//! - `GET /ws` — WebSocket using Agent Server Protocol
//! - `POST /api/chat/:agent_id` — HTTP POST to a specific agent
//! - `GET /api/agents` — List registered agents
//! - `GET /health` — Health check

use std::sync::Arc;

use axum::extract::Path;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tokio::net::TcpListener;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::AgentInput;
use vol_llm_agent_channel::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, Operation, Payload,
};
use vol_llm_agent_channel::{AgentServerCore, HttpTransport, WsServer};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let agents = [
        ("translator", "You are a translation assistant. Translate the input to English."),
        ("summarizer", "You are a summarization assistant. Provide a brief summary."),
        ("coder", "You are a coding assistant. Help with programming questions."),
    ];

    let core = Arc::new(
        AgentServerCore::new(std::env::current_dir().unwrap(), "~/.vol-llm-agent-channel")
            .await
            .expect("failed to create agent server core"),
    );

    for (id, prompt) in &agents {
        let def = AgentDef::new(*id, *prompt).with_type(*id);
        core.register_agent(*id, def)
            .await
            .unwrap_or_else(|e| panic!("failed to register {id}: {e}"));
        info!(agent_id = id, "Agent registered");
    }

    let ws_router = WsServer::new(core.clone()).into_axum_router();
    let http_router = HttpTransport::new(core.clone()).into_axum_router();
    let app = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .route("/api/agents", get({
            let core = core.clone();
            move || list_agents_handler(core.clone())
        }))
        .route("/api/chat/:agent_id", post({
            let core = core.clone();
            move |agent_id, body| chat_handler(core.clone(), agent_id, body)
        }))
        .merge(ws_router)
        .merge(http_router);

    info!("Starting multi-agent server on 0.0.0.0:3001");
    info!("  GET   /api/agents");
    info!("  WS    /ws");
    info!("  POST  /api/chat/:agent_id");

    let listener = TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("failed to bind to 0.0.0.0:3001");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

async fn list_agents_handler(core: Arc<AgentServerCore>) -> impl IntoResponse {
    let agents = core.list_agent_ids().await;
    Json(serde_json::json!({ "agents": agents }))
}

#[derive(Deserialize)]
struct ChatInput {
    input: String,
}

async fn chat_handler(
    core: Arc<AgentServerCore>,
    Path(agent_id): Path<String>,
    Json(body): Json<ChatInput>,
) -> impl IntoResponse {
    if !core.router().has_agent(&agent_id).await {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({ "error": "agent not found" })))
            .into_response();
    }

    let request = AgentServerMessage::new_command(
        uuid::Uuid::new_v4().to_string(),
        Operation::Agent(AgentOperation::Submit),
        Payload::Agent(AgentPayload::Submit {
            input: AgentInput::text(body.input),
            target: Some(agent_id),
        }),
    );

    match core.handle(request).await {
        Ok(messages) => (StatusCode::OK, Json(messages)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
