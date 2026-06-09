//! vol-agent-server: JSON-RPC agent service binary.
//!
//! Serves agent operations via JSON-RPC 2.0 over WebSocket.
//!
//! ## Usage
//!
//! ```bash
//! # Default config (~/.vol/agent-server.toml or built-in defaults)
//! vol-agent-server
//!
//! # Explicit config
//! vol-agent-server --config ./my-config.toml
//! ```

use std::sync::Arc;

use vol_llm_agent_channel::{AgentServerCore, JsonRpcServer};

mod config;
use config::ServerConfig;

#[tokio::main]
async fn main() {
    // --- Parse --config flag ---
    let explicit_config = std::env::args()
        .nth(1)
        .and_then(|arg| {
            if arg == "--config" {
                std::env::args().nth(2)
            } else if arg.starts_with("--config=") {
                Some(arg.trim_start_matches("--config=").to_string())
            } else {
                None
            }
        });

    // --- Load config ---
    let (mut config, config_path) = ServerConfig::load_or_default(explicit_config.as_deref())
        .unwrap_or_else(|e| {
            eprintln!("Config error: {}", e);
            std::process::exit(1);
        });
    config.expand_tilde();

    // --- Init tracing ---
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new(&config.tracing.level)
        });

    match config.tracing.format.as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .json()
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .init();
        }
    }

    if let Some(ref path) = config_path {
        tracing::info!("Config loaded from {:?}", path);
    } else {
        tracing::info!("Using built-in defaults (no config file found)");
    }

    if let Some(task_store) = &config.runtime.task_store {
        tracing::info!(task_store_type = ?task_store.store_type, "Using configured task store");
    } else {
        tracing::info!("Using default file task store");
    }

    // --- Build core ---
    tracing::info!(
        working_dir = %config.runtime.working_dir,
        store_dir = %config.runtime.store_dir,
        "Building AgentServerCore"
    );

    let core = AgentServerCore::builder(&config.runtime.working_dir, &config.runtime.store_dir)
        .with_task_store_config(config.runtime.task_store.clone())
        .build()
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to build AgentServerCore: {}", e);
            std::process::exit(1);
        });

    // --- Discover agents ---
    core.discover_agents()
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to discover agents: {}", e);
            std::process::exit(1);
        });

    // --- Start server ---
    let server = JsonRpcServer::new(Arc::new(core));
    let app = server.into_axum_router();

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to bind {}: {}", addr, e);
            std::process::exit(1);
        });

    tracing::info!("JSON-RPC server started on ws://{}", addr);
    tracing::info!("  Methods: agent.submit, agent.cancel, agent.approve");
    tracing::info!("           agent.list, agent.subscribe, agent.unsubscribe");
    tracing::info!("           file.list, file.read");
    tracing::info!("           log.list, log.read");
    tracing::info!("           session.list, session.resume");
    tracing::info!("           mcp.* (list_servers, list_tools, call_tool, etc.)");
    tracing::info!("           skill.list, skill.get");
    tracing::info!("           task.list, task.output");

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Server error: {}", e);
            std::process::exit(1);
        });
}
