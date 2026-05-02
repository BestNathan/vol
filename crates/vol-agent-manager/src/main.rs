use anyhow::Result;
use tracing::info;

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
                .unwrap_or_else(|_| "vol_agent_manager=info".into()),
        )
        .init();

    let config_path = parse_args().unwrap_or_else(|| "config.toml".to_string());
    let config = vol_agent_manager::config::ManagerConfig::from_path(&config_path)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to load {}: {}, using defaults", config_path, e);
            vol_agent_manager::config::ManagerConfig::default()
        });

    info!("vol-agent-manager starting on {}", config.server.listen_addr);
    info!("Press Ctrl+C to stop");

    // TODO: start server
    tokio::signal::ctrl_c().await?;
    info!("Shutting down");
    Ok(())
}
