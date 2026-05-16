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
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent};
use vol_llm_agent_channel::{AgentDispatcher, AgentRegistration, ConnectionHolder, JsonRpcServer};
use vol_llm_mcp::McpConfig;
use vol_llm_mcp::McpManager;
use vol_llm_provider::create_provider;
use vol_llm_skill::SkillLoader;
use vol_llm_tool::ToolRegistry;
use vol_session::file_store::FileSessionEntryStore;
use vol_session::Session;

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
        "qwen3.6-plus",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    ))
    .expect("failed to create LLM provider \u{2014} set ANTHROPIC_AUTH_TOKEN");

    // Build agent
    let def = AgentDef::new(
        "general-assistant",
        "You are a helpful AI assistant. Answer questions concisely.",
    )
    .with_type("general-assistant");

    let entry_store = Arc::new(FileSessionEntryStore::new("/tmp/vol-llm-store"));
    let session = Arc::new(Session::new(entry_store));
    let tools = Arc::new(ToolRegistry::new());
    let mut config = AgentConfig::new(Arc::from(llm), tools, session);
    config.def = Some(def);

    // Create ConnectionHolder as the event bridge plugin.
    // Pass the holder by value to register() (it wraps in Arc internally),
    // then clone it (cheap - internal state is Arc-wrapped) for AgentRegistration.
    let holder = ConnectionHolder::new("agent".to_string(), "client".to_string());
    let mut plugin_registry = PluginRegistry::new();
    plugin_registry.register(holder.clone());

    let mut config_with_plugin = config;
    config_with_plugin.plugin_registry = plugin_registry;
    let agent = ReActAgent::new(config_with_plugin);

    // Create dispatcher
    let dispatcher = Arc::new(AgentDispatcher::new(agent));

    // Wrap holder in Arc for the server
    let holder = Arc::new(holder);

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

    // Create JSON-RPC server
    let server = JsonRpcServer::new(
        vec![AgentRegistration {
            agent_id: "general-assistant".to_string(),
            dispatcher,
            holder,
        }],
        ".".to_string(),
        "/tmp/vol-llm-store".to_string(),
        Some(mcp_manager),
        skill_loader,
    ).await;

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
