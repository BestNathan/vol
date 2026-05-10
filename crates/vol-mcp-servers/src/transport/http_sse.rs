use std::net::SocketAddr;
use std::sync::Arc;

use axum::Router;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager,
    tower::{StreamableHttpServerConfig, StreamableHttpService},
};
use rmcp::Service;
use tokio_util::sync::CancellationToken;

pub async fn serve_http_sse<S>(
    server: S,
    addr: SocketAddr,
    ct: CancellationToken,
) -> anyhow::Result<()>
where
    S: Service<rmcp::RoleServer> + Clone + Send + Sync + 'static,
{
    let session_manager = Arc::new(LocalSessionManager::default());
    let config = StreamableHttpServerConfig::default().with_cancellation_token(ct.clone());
    let service = StreamableHttpService::new(move || Ok(server.clone()), session_manager, config);
    let app = Router::new().nest_service("/", service);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("docs-rs-mcp listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { ct.cancelled().await })
        .await?;
    Ok(())
}
