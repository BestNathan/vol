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
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();
    let cli = Cli::parse();
    let ct = CancellationToken::new();
    let server = DocsRsServer::new();
    transport::run_server(cli.transport.mode(), server, ct).await
}
