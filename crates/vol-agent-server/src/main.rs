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

use vol_agent_server::{app, config::ServerConfig};

#[tokio::main]
async fn main() {
    // --- Parse --config flag ---
    let explicit_config = std::env::args().nth(1).and_then(|arg| {
        if arg == "--config" {
            std::env::args().nth(2)
        } else if arg.starts_with("--config=") {
            Some(arg.trim_start_matches("--config=").to_string())
        } else {
            None
        }
    });

    // --- Load config ---
    let (config, config_path) = ServerConfig::load_or_default(explicit_config.as_deref())
        .unwrap_or_else(|e| {
            eprintln!("Config error: {}", e);
            std::process::exit(1);
        });

    // --- Init tracing ---
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.tracing.level));

    match config.tracing.format.as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .json()
                .init();
        }
        _ => {
            tracing_subscriber::fmt().with_env_filter(env_filter).init();
        }
    }

    if let Some(ref path) = config_path {
        tracing::info!("Config loaded from {:?}", path);
    } else {
        tracing::info!("Using built-in defaults (no config file found)");
    }

    tracing::info!(
        control_plane = config.server.roles.control_plane,
        data_plane = config.server.roles.data_plane,
        "Configured server roles"
    );

    if let Some(task_store) = &config.runtime.task_store {
        tracing::info!(task_store_type = ?task_store.store_type, "Using configured task store");
    } else {
        tracing::info!("Using default file task store");
    }

    if let Some(session_store) = &config.runtime.session_store {
        tracing::info!(session_store_type = ?session_store.store_type, "Using configured session store");
    } else {
        tracing::info!("Using default file session store");
    }

    if let Err(err) = app::run(config).await {
        tracing::error!("Server error: {}", err);
        std::process::exit(1);
    }
}
