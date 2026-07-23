use std::collections::HashMap;

/// Per-node data container. Fields will be populated with tab-specific state
/// types in later tasks as the DP-centric architecture is wired up.
#[derive(Debug, Clone, Default)]
pub struct NodeData {
    pub data: HashMap<String, serde_json::Value>,
}

/// Cache of per-node data, keyed by node_id.
/// Enables instant switching between data-plane nodes without re-fetching.
#[derive(Debug, Clone, Default)]
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

    pub fn get_mut(&mut self, node_id: &str) -> Option<&mut NodeData> {
        self.cache.get_mut(node_id)
    }

    pub fn get_or_insert(&mut self, node_id: &str) -> &mut NodeData {
        self.cache.entry(node_id.to_string()).or_default()
    }

    pub fn invalidate(&mut self, node_id: &str) {
        self.cache.remove(node_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_mut_mutates_existing_entry() {
        let mut cache = NodeDataCache::default();
        cache.get_or_insert("node-A");

        let data = cache.get_mut("node-A").unwrap();
        data.data
            .insert("test".to_string(), serde_json::json!("value"));

        assert!(cache.get("node-A").unwrap().data.get("test").is_some());
    }

    #[test]
    fn test_get_mut_returns_none_for_missing() {
        let mut cache = NodeDataCache::default();
        assert!(cache.get_mut("missing").is_none());
    }

    #[test]
    fn test_clone_produces_independent_copy() {
        let mut cache = NodeDataCache::default();
        cache.get_or_insert("node-A");
        let cloned = cache.clone();
        assert!(cloned.get("node-A").is_some());
    }
}
