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
use vol_llm_core::AgentDef;
use vol_llm_agent_channel::{AgentServerCore, HttpTransport, WsServer};

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    info!(model = "claude-sonnet-4-6", "LLM provider configured through AgentServerCore");

    // Build agent
    let def = AgentDef::new(
        "general-assistant",
        "You are a helpful AI assistant. Answer questions concisely.",
    )
    .with_type("general-assistant");

    let def_for_core = def.clone();

    let core = Arc::new(
        AgentServerCore::new(std::env::current_dir().unwrap(), "~/.vol-llm-agent-channel")
            .await
            .expect("failed to create agent server core"),
    );
    core.register_agent("my-agent", def_for_core)
        .await
        .expect("failed to register my-agent");

    // Build routers
    // SSE streaming (?stream=true) is handled internally by HttpTransport.
    let ws_router = WsServer::new(core.clone()).into_axum_router();
    let http_router = HttpTransport::new(core.clone()).into_axum_router();

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
