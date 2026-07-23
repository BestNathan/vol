use std::collections::HashMap;

/// Per-node data container. Fields will be populated with tab-specific state
/// types in later tasks as the DP-centric architecture is wired up.
#[derive(Debug, Clone, Default)]
pub struct NodeData {
    pub data: HashMap<String, serde_json::Value>,
}

/// Cache of per-node data, keyed by node_id.
/// Enables instant switching between data-plane nodes without re-fetching.
#[derive(Debug, Default)]
pub struct NodeDataCache {
    cache: HashMap<String, NodeData>,
}

impl NodeDataCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn get(&self, node_id: &str) -> Option<&NodeData> {
        self.cache.get(node_id)
    }

    pub fn get_or_insert(&mut self, node_id: &str) -> &mut NodeData {
        self.cache.entry(node_id.to_string()).or_default()
    }

    pub fn invalidate(&mut self, node_id: &str) {
        self.cache.remove(node_id);
    }
}
