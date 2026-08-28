//! JSON-RPC server providing a WebSocket endpoint.

use std::sync::Arc;

use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;

use crate::connection::Connection;
use crate::service::JsonRpcMessageService;

use super::connection::JsonRpcConnection;

/// JSON-RPC server providing a WebSocket endpoint.
pub struct JsonRpcServer<S> {
    service: Arc<S>,
    path: String,
}

impl<S> JsonRpcServer<S>
where
    S: JsonRpcMessageService,
{
    /// Create a new server wrapping the given service and mounting path.
    pub fn new(service: Arc<S>, path: impl Into<String>) -> Self {
        Self {
            service,
            path: path.into(),
        }
    }

    /// Build an axum `Router` with the JSON-RPC WebSocket endpoint.
    pub fn into_axum_router(self) -> Router {
        let path = self.path.clone();
        let server = Arc::new(self);

        Router::new().route(
            &path,
            get(move |ws: WebSocketUpgrade| {
                let server = server.clone();
                async move { ws.on_upgrade(move |socket| handle_ws(socket, server)) }
            }),
        )
    }
}

async fn handle_ws<S>(socket: WebSocket, server: Arc<JsonRpcServer<S>>)
where
    S: JsonRpcMessageService,
{
    let conn: Arc<dyn Connection> = Arc::new(JsonRpcConnection::new(socket));
    server.service.serve_connection(conn).await;
}

#[cfg(test)]
mod generic_service_tests {
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::JsonRpcServer;
    use crate::connection::Connection;
    use crate::service::JsonRpcMessageService;

    struct MockService;

    #[async_trait]
    impl JsonRpcMessageService for MockService {
        async fn serve_connection(&self, _conn: Arc<dyn Connection>) {}
    }

    #[test]
    fn jsonrpc_server_accepts_generic_service_and_path() {
        let service = Arc::new(MockService);
        let _server = JsonRpcServer::new(service, "/custom/ws");
    }
}
