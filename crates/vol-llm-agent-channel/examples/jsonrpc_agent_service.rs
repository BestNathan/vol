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
use vol_llm_mcp::McpConfig;
use vol_llm_mcp::McpManager;
use vol_llm_provider::create_provider;
use vol_llm_skill::SkillLoader;

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

    // Create MCP manager and connect
    let mcp_manager = {
        let configs = McpConfig::load(Some(std::path::Path::new(".")))
            .map(|c| c.servers().to_vec())
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load MCP config: {}", e);
                vec![]
            });
        tracing::info!("Loaded {} MCP server configurations", configs.len());
        let manager = McpManager::new(configs);
        let manager_for_connect = manager.clone();
        tokio::spawn(async move {
            let _ = manager_for_connect.connect().await;
        });
        Arc::new(manager)
    };

    // Create skill loader and discover skills
    let skill_loader = {
        let loader = Arc::new(SkillLoader::new(Some(std::path::PathBuf::from("."))));
        let loader_for_discover = loader.clone();
        tokio::spawn(async move {
            if let Err(e) = loader_for_discover.discover_all().await {
                tracing::warn!("Failed to discover skills: {}", e);
            }
        });
        Some(loader)
    };

    // Build unified core
    let core = AgentServerCore::builder()
        .working_dir(".")
        .store_dir("~/.vol")
        .llm(Arc::from(llm))
        .mcp_manager(mcp_manager)
        .skill_loader(skill_loader.unwrap())
        .build()
        .await
        .expect("failed to build core");

    // Register agent
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
