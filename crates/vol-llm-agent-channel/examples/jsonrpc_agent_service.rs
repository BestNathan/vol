//! JSON-RPC agent service over WebSocket.
//!
//! Exposes agent operations via JSON-RPC 2.0 over WebSocket.
//!
//! Run with:
//! ```bash
//! ANTHROPIC_AUTH_TOKEN=your_key RUST_LOG=info \
//!   cargo run --example jsonrpc_agent_service -p vol-llm-agent-channel
//! ```

use std::sync::Arc;

use vol_llm_agent::agent_def::AgentDef;
use vol_llm_provider::create_provider;

use vol_llm_agent_channel::AgentServerCore;
use vol_llm_agent_channel::JsonRpcServer;

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Create LLM provider
    let llm = create_provider(&vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "coding",
        "ANTHROPIC_AUTH_TOKEN",
        "http://192.168.2.162:31693",
    ))
    .expect("failed to create LLM provider — set ANTHROPIC_AUTH_TOKEN");

    // Build core from working_dir + store_dir only.
    // MCP config is auto-discovered from .mcp.json, skills from .agents/skills/.
    let core = AgentServerCore::new(".", "~/.vol", Arc::from(llm))
        .build()
        .await
        .expect("failed to build core");

    // Register default agent.
    let def = AgentDef::new(
        "general-assistant",
        "You are a helpful AI assistant. Answer questions concisely.",
    )
    .with_type("general-assistant");

    core.register_agent("general-assistant", def)
        .await
        .expect("failed to register agent");

    // Create JSON-RPC server
    let server = JsonRpcServer::new(Arc::new(core));

    let app = server.into_axum_router();

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3001")
        .await
        .expect("failed to bind");

    tracing::info!("JSON-RPC server started on ws://localhost:3001");
    tracing::info!("  Methods: agent.submit, agent.cancel, agent.approve");
    tracing::info!("           agent.subscribe, agent.unsubscribe");
    tracing::info!("           file.list, file.read");
    tracing::info!("           log.list, log.read");
    tracing::info!("           session.list, session.resume");
    tracing::info!("           mcp.* (list_servers, list_tools, call_tool, etc.)");
    tracing::info!("           skill.list, skill.get");

    axum::serve(listener, app)
        .await
        .expect("server error");
}
