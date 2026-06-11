use std::sync::Arc;

use async_trait::async_trait;

use crate::connection::Connection;

/// Generic service abstraction consumed by JSON-RPC WebSocket transport.
///
/// Implementations own connection lifecycle behavior. Concrete services live
/// outside the transport layer, e.g. data-plane and control-plane server cores.
#[async_trait]
pub trait JsonRpcMessageService: Send + Sync + 'static {
    async fn serve_connection(&self, conn: Arc<dyn Connection>);
}
