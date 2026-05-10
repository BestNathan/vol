use std::net::SocketAddr;

use clap::Parser;
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use tokio_util::sync::CancellationToken;

mod http_sse;

#[derive(Parser, Debug)]
pub struct TransportArgs {
    /// Listen address for HTTP/SSE transport (e.g. 0.0.0.0:8080)
    #[arg(long)]
    pub http: Option<SocketAddr>,
}

pub enum TransportMode {
    Stdio,
    HttpSse(SocketAddr),
}

impl TransportArgs {
    pub fn mode(&self) -> TransportMode {
        if let Some(addr) = self.http {
            TransportMode::HttpSse(addr)
        } else {
            TransportMode::Stdio
        }
    }
}

pub async fn run_server<S>(mode: TransportMode, server: S, ct: CancellationToken) -> anyhow::Result<()>
where
    S: rmcp::Service<rmcp::RoleServer> + Clone + Send + Sync + 'static,
{
    match mode {
        TransportMode::Stdio => {
            tracing::info!("docs-rs-mcp running on stdio");
            let service = server.serve(stdio()).await?;
            tokio::select! {
                _ = ct.cancelled() => {}
                _ = service.waiting() => {}
            }
        }
        TransportMode::HttpSse(addr) => {
            http_sse::serve_http_sse(server, addr, ct).await?;
        }
    }
    Ok(())
}
