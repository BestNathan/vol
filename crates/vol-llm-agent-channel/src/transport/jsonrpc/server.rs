//! JSON-RPC server providing a `/ws` WebSocket endpoint.

use std::sync::Arc;

use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::routing::get;
use axum::Router;

use crate::server_core::AgentServerCore;

use super::connection::JsonRpcConnection;

/// JSON-RPC server providing a `/ws` WebSocket endpoint.
pub struct JsonRpcServer {
    core: Arc<AgentServerCore>,
}

impl JsonRpcServer {
    /// Create a new server wrapping the given core.
    pub fn new(core: Arc<AgentServerCore>) -> Self {
        Self { core }
    }

    /// Build an axum `Router` with the JSON-RPC WebSocket endpoint at `/ws`.
    pub fn into_axum_router(self) -> Router {
        let server = Arc::new(self);

        Router::new().route(
            "/ws",
            get(move |ws: WebSocketUpgrade| {
                let server = server.clone();
                async move { ws.on_upgrade(move |socket| handle_ws(socket, server)) }
            }),
        )
    }
}

async fn handle_ws(socket: WebSocket, server: Arc<JsonRpcServer>) {
    let conn = JsonRpcConnection::new(socket);
    server.core.serve(conn).await;
}
