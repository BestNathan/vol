//! Per-node data-plane WebSocket connection pool.
//!
//! Created lazily when the user selects an agent on a node. Reused across
//! agents on the same node. Each connection has its own event stream and
//! auto-subscribes on open.

use crate::web::client::JsonRpcClient;
use std::collections::HashMap;

/// A data-plane connection for one node.
#[derive(Clone)]
pub struct DpConnection {
    pub client: JsonRpcClient,
    pub node_id: String,
    pub ws_url: String,
    pub agent_ids: Vec<String>,
}

/// Manages a pool of per-node data-plane connections.
#[derive(Clone, Default)]
pub struct DpConnectionPool {
    connections: HashMap<String, DpConnection>,
}

impl DpConnectionPool {
    pub fn new() -> Self {
        Self {
            connections: HashMap::new(),
        }
    }

    pub fn get_or_create(
        &mut self,
        node_id: &str,
        ws_url: &str,
        agent_ids: Vec<String>,
    ) -> &DpConnection {
        if !self.connections.contains_key(node_id) {
            let client = JsonRpcClient::new(ws_url);
            let conn = DpConnection {
                client,
                node_id: node_id.to_string(),
                ws_url: ws_url.to_string(),
                agent_ids,
            };
            self.connections.insert(node_id.to_string(), conn);
        }
        self.connections.get(node_id).unwrap()
    }

    pub fn get(&self, node_id: &str) -> Option<&DpConnection> {
        self.connections.get(node_id)
    }

    pub fn contains(&self, node_id: &str) -> bool {
        self.connections.contains_key(node_id)
    }

    pub fn connections(&self) -> impl Iterator<Item = &DpConnection> {
        self.connections.values()
    }
}
