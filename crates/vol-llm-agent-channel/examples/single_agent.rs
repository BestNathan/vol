//! Single-agent service with WebSocket and HTTP endpoints.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example single_agent -p vol-llm-agent-channel
//! ```
//!
//! Endpoints:
//! - `GET /ws` — WebSocket upgrade for bidirectional chat
//! - `POST /api/chat` — HTTP POST with `{"input": "..."}`, returns JSON result
//! - `POST /api/chat?stream=true` — Same as POST but with SSE event streaming
//! - `GET /health` — Health check

use std::sync::Arc;

use axum::routing::get;
use axum::{Json, Router};
use tokio::net::TcpListener;
use tracing::info;
use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::react::{AgentConfig, ReActAgent};
use vol_llm_agent_channel::{AgentDispatcher, ConnectionHolder, HttpTransport, WsServer};
use vol_llm_provider::create_provider;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create LLM provider from env
    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "claude-sonnet-4-6",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    info!(model = "claude-sonnet-4-6", "LLM provider created");

    // Build agent
    let def = AgentDef::new(
        "general-assistant",
        "You are a helpful AI assistant. Answer questions concisely.",
    )
    .with_type("general-assistant");

    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let mut config = AgentConfig::new(Arc::from(llm), tools, session);
    config.def = Some(def);
    let agent = ReActAgent::new(config);

    // Shared primitives
    // ConnectionHolder: sender=agent id (outgoing events), receiver=client id (incoming messages)
    // NOTE: To forward agent stream events (tool calls, thinking, content) to the WebSocket
    // connection, register the holder as a plugin on the agent before creation.
    // ConnectionHolder does not implement Clone; see the channel crate tests for usage patterns.
    let holder = Arc::new(ConnectionHolder::new("my-agent".to_string(), "client".to_string()));

    let dispatcher = Arc::new(AgentDispatcher::new(agent));

    // Build routers
    // SSE streaming (?stream=true) is handled internally by HttpTransport.
    let ws_router = WsServer::new(dispatcher.clone(), holder.clone(), "my-agent").into_axum_router();
    let http_router = HttpTransport::new(dispatcher, holder, "my-agent").into_axum_router();

    // Combine
    let app = Router::new()
        .route("/health", get(|| async { Json(serde_json::json!({"status": "ok"})) }))
        .merge(ws_router)
        .merge(http_router);

    info!("Starting server on 0.0.0.0:3000");
    info!("  WS:   ws://localhost:3000/ws");
    info!("  HTTP: POST http://localhost:3000/api/chat");
    info!("  SSE:  POST http://localhost:3000/api/chat?stream=true");

    let listener = TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to 0.0.0.0:3000");

    axum::serve(listener, app)
        .await
        .expect("server failed");
}
