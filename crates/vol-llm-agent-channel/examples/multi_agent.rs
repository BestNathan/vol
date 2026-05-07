//! Multi-agent service with per-agent WebSocket and HTTP endpoints.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example multi_agent -p vol-llm-agent-channel
//! ```
//!
//! Endpoints:
//! - `GET /ws/:agent_id` — WebSocket to a specific agent
//! - `POST /api/chat/:agent_id` — HTTP POST to a specific agent
//! - `GET /api/agents` — List registered agents
//! - `GET /health` — Health check

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::Path;
use axum::extract::WebSocketUpgrade;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, ReActAgent};
use vol_llm_agent_channel::{AgentDispatcher, AgentRouter, ConnectionHolder, WsConnection};
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

/// Shared application state.
#[derive(Clone)]
struct AppState {
    router: AgentRouter,
    dispatchers: Arc<RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
    holders: Arc<RwLock<HashMap<String, Arc<ConnectionHolder>>>>,
}

/// Build a ReActAgent with the given system prompt.
fn make_agent(llm: Arc<dyn vol_llm_core::LLMClient>, name: &str, prompt: &str) -> ReActAgent {
    let def = AgentDef::new(name, prompt).with_type(name);
    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let mut config = AgentConfig::new(llm, tools, session);
    config.def = Some(def);
    ReActAgent::new(config)
}

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create shared LLM provider
    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "claude-sonnet-4-6",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::from(llm);

    // Define agents
    let agents = [
        ("translator", "You are a translation assistant. Translate the input to English."),
        ("summarizer", "You are a summarization assistant. Provide a brief summary."),
        ("coder", "You are a coding assistant. Help with programming questions."),
    ];

    let router = AgentRouter::new();
    let mut dispatchers = HashMap::new();
    let mut holders = HashMap::new();

    for (id, prompt) in &agents {
        let agent = make_agent(llm.clone(), id, prompt);
        // ConnectionHolder: sender=agent id (outgoing events), receiver=client id (incoming)
        // NOTE: To forward agent stream events to WebSocket, register holder as a plugin
        // on the agent before creation. ConnectionHolder does not implement Clone.
        let holder = Arc::new(ConnectionHolder::new(id.to_string(), "client".to_string()));
        let dispatcher = Arc::new(AgentDispatcher::new(agent));

        router.register(id.to_string(), dispatcher.clone()).await;
        dispatchers.insert(id.to_string(), dispatcher);
        holders.insert(id.to_string(), holder);

        info!(agent_id = id, "Agent registered");
    }

    let state = AppState {
        router,
        dispatchers: Arc::new(RwLock::new(dispatchers)),
        holders: Arc::new(RwLock::new(holders)),
    };

    // Build router
    let app = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .route("/api/agents", get(list_agents_handler))
        .route("/ws/:agent_id", get(ws_handler))
        .route("/api/chat/:agent_id", post(chat_handler))
        .with_state(state);

    info!("Starting multi-agent server on 0.0.0.0:3000");
    info!("  GET   /api/agents");
    info!("  WS    /ws/:agent_id  (e.g. /ws/translator)");
    info!("  POST  /api/chat/:agent_id");

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to 0.0.0.0:3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}

/// GET /api/agents — list registered agents.
async fn list_agents_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let agents = state.router.list_agents().await;
    Json(serde_json::json!({ "agents": agents }))
}

/// GET /ws/:agent_id — WebSocket upgrade to a specific agent.
async fn ws_handler(
    Path(agent_id): Path<String>,
    ws: WebSocketUpgrade,
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let dispatchers = state.dispatchers.read().await;
    let holders = state.holders.read().await;

    match (dispatchers.get(&agent_id), holders.get(&agent_id)) {
        (Some(dispatcher), Some(holder)) => {
            let dispatcher = dispatcher.clone();
            let holder = holder.clone();
            let aid = agent_id.clone();
            ws.on_upgrade(move |socket| {
                let conn = WsConnection::new(socket, dispatcher, holder, aid);
                conn.run()
            })
            .into_response()
        }
        _ => (StatusCode::NOT_FOUND, "agent not found").into_response(),
    }
}

/// POST /api/chat/:agent_id — HTTP chat with a specific agent.
#[derive(Deserialize)]
struct ChatInput {
    input: String,
}

async fn chat_handler(
    Path(agent_id): Path<String>,
    axum::extract::State(state): axum::extract::State<AppState>,
    Json(body): Json<ChatInput>,
) -> impl IntoResponse {
    let request = vol_llm_agent_channel::AgentRequest::new(&agent_id, &body.input);

    match state.router.send(&agent_id, request).await {
        Ok(rx) => match rx.await {
            Ok(run_result) => match run_result.response {
                Ok(resp) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "req_id": run_result.req_id,
                        "success": true,
                        "response": serde_json::to_value(resp).unwrap_or(serde_json::Value::Null),
                    })),
                ).into_response(),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "success": false, "error": e.to_string() })),
                ).into_response(),
            },
            Err(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "success": false, "error": "dispatcher dropped" })),
            ).into_response(),
        },
        Err(e) => {
            let status = match e {
                vol_llm_agent_channel::ChannelError::AgentNotFound(_) => StatusCode::NOT_FOUND,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            };
            (status, Json(serde_json::json!({ "error": e.to_string() }))).into_response()
        }
    }
}
