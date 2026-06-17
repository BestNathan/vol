use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use vol_llm_agent_protocol::Connection;

use super::capability::CapabilityIndex;
use super::event::EventBus;
use super::registry::NodeRegistry;
use super::store::{CommandStore, RunStore};

#[derive(Clone)]
pub struct ControlPlaneState {
    pub nodes: Arc<NodeRegistry>,
    pub capabilities: Arc<CapabilityIndex>,
    pub events: EventBus,
    pub commands: Arc<CommandStore>,
    pub runs: Arc<RunStore>,
    /// Active data-plane node WebSocket connections, keyed by node_id.
    /// Populated when a DataPlaneNode connects via /control/v1/ws.
    pub node_connections: Arc<RwLock<HashMap<String, Arc<dyn Connection>>>>,
}

impl ControlPlaneState {
    pub fn new() -> Self {
        Self {
            nodes: Arc::new(NodeRegistry::new()),
            capabilities: Arc::new(CapabilityIndex::new()),
            events: EventBus::new(),
            commands: Arc::new(CommandStore::new()),
            runs: Arc::new(RunStore::new()),
            node_connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ControlPlaneState {
    /// Store or re-key a node connection. When a node registers, call with
    /// old_temp_key (the Arc address) and the real node_id to replace the entry.
    pub fn rekey_node_connection(&self, old_temp_key: &str, node_id: &str) {
        let mut map = self
            .node_connections
            .write()
            .expect("node_connections lock poisoned");
        if let Some(conn) = map.remove(old_temp_key) {
            map.insert(node_id.to_string(), conn);
        }
    }

    /// Get a stored node connection by node_id.
    pub fn get_node_connection(&self, node_id: &str) -> Option<Arc<dyn Connection>> {
        self.node_connections
            .read()
            .expect("node_connections lock poisoned")
            .get(node_id)
            .cloned()
    }
}

impl Default for ControlPlaneState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    

    #[test]
    fn state_new_creates_all_fields() {
        let state = ControlPlaneState::new();
        assert!(state.nodes.list().is_empty());
        assert!(state.capabilities.list(None).is_empty());
        assert!(state.commands.get("none").is_none());
        assert!(state.runs.get("none").is_none());
    }

    #[test]
    fn state_default_creates_valid_state() {
        let state = ControlPlaneState::default();
        assert!(state.nodes.list().is_empty());
        assert!(state.capabilities.list(None).is_empty());
    }

    #[test]
    fn state_clone_produces_identical_shared_state() {
        let state = ControlPlaneState::new();
        let cloned = state.clone();
        // Both clones share the same Arcs
        assert!(Arc::ptr_eq(&state.nodes, &cloned.nodes));
        assert!(Arc::ptr_eq(&state.capabilities, &cloned.capabilities));
    }
}
