use std::path::PathBuf;

use clap::Parser;
use tokio_util::sync::CancellationToken;
use vol_llm_sandbox::registry::SandboxRegistry;
use vol_mcp_servers::cli_tools::CliToolsMcpServer;
use vol_mcp_servers::transport::{self, TransportArgs};

#[derive(Parser)]
#[command(name = "cli-tools-mcp", about = "CLI-as-Tool MCP server")]
struct Cli {
    #[command(flatten)]
    transport: TransportArgs,

    /// Directory containing .agents/cli-tools/*.toml configs.
    #[arg(long, default_value = ".agents/cli-tools")]
    cli_tools_dir: PathBuf,

    /// Directory containing .agents/sandboxes/*.toml configs (for sandbox_ref).
    #[arg(long, default_value = ".agents/sandboxes")]
    sandboxes_dir: PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let otel_config = vol_llm_observability::OtelConfig {
        enabled: std::env::var("OTEL_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(false),
        endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
            .unwrap_or_else(|_| "http://otel-collector.observability.svc.cluster.local:4317".to_string()),
        service_name: "cli-tools-mcp".to_string(),
        service_namespace: "vol-agent".to_string(),
        deployment_environment: std::env::var("OTEL_ENV").unwrap_or_else(|_| "production".to_string()),
        sample_rate: 1.0,
        batch_max_export_timeout_millis: 5000,
    };
    let _otel_guards = vol_llm_observability::init(&otel_config, "info")
        .expect("Failed to initialize tracing");

    let cli = Cli::parse();

    let sandbox_registry = SandboxRegistry::load(&cli.sandboxes_dir)
        .await
        .map_err(|e| anyhow::anyhow!("sandbox registry: {e}"))?;

    let server = CliToolsMcpServer::load(&cli.cli_tools_dir, &sandbox_registry)
        .await
        .map_err(|e| anyhow::anyhow!("cli-tools load: {e}"))?;

    let ct = CancellationToken::new();
    transport::run_server(cli.transport.mode(), server, ct).await
}
