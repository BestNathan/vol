//! JSON-RPC server managing multiple agent connections.

use std::collections::HashMap;
use std::sync::Arc;

use axum::Router;
use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::routing::get;

use vol_llm_mcp::manager::McpManager;

use crate::connection::ConnectionHolder;
use crate::dispatcher::AgentDispatcher;
use crate::router::AgentRouter;

use super::connection::JsonRpcConnection;

/// Registration info for a single agent.
pub struct AgentRegistration {
    pub agent_id: String,
    pub dispatcher: Arc<AgentDispatcher>,
    pub holder: Arc<ConnectionHolder>,
}

/// JSON-RPC server managing multiple agents.
pub struct JsonRpcServer {
    router: AgentRouter,
    dispatchers: HashMap<String, Arc<AgentDispatcher>>,
    holders: HashMap<String, Arc<ConnectionHolder>>,
    working_dir: String,
    store_dir: String,
    mcp_manager: Option<Arc<McpManager>>,
}

impl JsonRpcServer {
    /// Create a new server with the given agent registrations.
    pub async fn new(
        agents: Vec<AgentRegistration>,
        working_dir: String,
        store_dir: String,
        mcp_manager: Option<Arc<McpManager>>,
    ) -> Self {
        let router = AgentRouter::new();
        let mut holders = HashMap::new();
        let mut dispatchers = HashMap::new();

        for reg in agents {
            router.register(reg.agent_id.clone(), reg.dispatcher.clone()).await;
            dispatchers.insert(reg.agent_id.clone(), reg.dispatcher);
            holders.insert(reg.agent_id, reg.holder);
        }

        Self { router, dispatchers, holders, working_dir, store_dir, mcp_manager }
    }

    /// Build axum Router with the JSON-RPC WebSocket endpoint at `/ws`.
    pub fn into_axum_router(self) -> Router {
        let server = Arc::new(self);

        Router::new()
            .route(
                "/ws",
                get(move |ws: WebSocketUpgrade| {
                    let server = server.clone();
                    async move { ws.on_upgrade(move |socket| handle_ws(socket, server)) }
                }),
            )
    }
}

async fn handle_ws(socket: WebSocket, server: Arc<JsonRpcServer>) {
    let session_store = Arc::new(vol_session::FileSessionEntryStore::new(&server.store_dir));
    let conn = JsonRpcConnection::new(
        socket,
        server.router.clone(),
        server.dispatchers.clone(),
        server.holders.clone(),
        server.working_dir.clone(),
        server.store_dir.clone(),
        session_store,
        server.mcp_manager.clone(),
    );
    let conn_arc = Arc::new(conn);
    conn_arc.run().await;
}
