#![allow(clippy::expect_used)]
use clap::Parser;
use tokio_util::sync::CancellationToken;
use vol_mcp_servers::docs_rs::DocsRsServer;
use vol_mcp_servers::transport::{self, TransportArgs};

#[derive(Parser)]
#[command(name = "docs-rs-mcp", about = "docs.rs MCP server")]
struct Cli {
    #[command(flatten)]
    transport: TransportArgs,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let otel_config = vol_llm_observability::OtelConfig {
        enabled: std::env::var("OTEL_ENABLED")
            .map(|v| v == "true")
            .unwrap_or(false),
        endpoint: std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").unwrap_or_else(|_| {
            "http://otel-collector.observability.svc.cluster.local:4317".to_string()
        }),
        service_name: "docs-rs-mcp".to_string(),
        service_namespace: "vol-agent".to_string(),
        deployment_environment: std::env::var("OTEL_ENV")
            .unwrap_or_else(|_| "production".to_string()),
        sample_rate: 1.0,
        batch_max_export_timeout_millis: 5000,
    };
    let _otel_guards =
        vol_llm_observability::init(&otel_config, "info").expect("Failed to initialize tracing");
    let cli = Cli::parse();
    let ct = CancellationToken::new();
    let server = DocsRsServer::new();
    transport::run_server(cli.transport.mode(), server, ct).await
}
