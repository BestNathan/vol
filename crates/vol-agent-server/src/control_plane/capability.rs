use std::collections::HashMap;
use std::sync::RwLock;

use vol_llm_agent_protocol::agent_server_protocol::CapabilitySnapshot;

pub struct CapabilityIndex {
    snapshots: RwLock<HashMap<String, CapabilitySnapshot>>,
}

impl CapabilityIndex {
    pub fn new() -> Self {
        Self {
            snapshots: RwLock::new(HashMap::new()),
        }
    }

    pub fn apply_snapshot(&self, snapshot: CapabilitySnapshot) -> Result<(), String> {
        let mut snapshots = self
            .snapshots
            .write()
            .expect("capability index snapshots lock poisoned while applying snapshot");
        if let Some(existing) = snapshots.get(&snapshot.node_id) {
            if snapshot.revision <= existing.revision {
                return Err("stale_capability_snapshot".to_string());
            }
        }
        snapshots.insert(snapshot.node_id.clone(), snapshot);
        Ok(())
    }

    pub fn list(&self, node_id: Option<&str>) -> Vec<CapabilitySnapshot> {
        let snapshots = self
            .snapshots
            .read()
            .expect("capability index snapshots lock poisoned while listing snapshots");
        match node_id {
            Some(id) => snapshots.get(id).cloned().into_iter().collect(),
            None => snapshots.values().cloned().collect(),
        }
    }
}

impl Default for CapabilityIndex {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent_protocol::agent_server_protocol::{AgentCapability, CapabilitySnapshot};

    fn snapshot(node_id: &str, revision: u64, agent_id: &str) -> CapabilitySnapshot {
        CapabilitySnapshot {
            node_id: node_id.to_string(),
            revision,
            generated_at_ms: Some(1000 + revision),
            agents: vec![AgentCapability {
                agent_id: agent_id.to_string(),
                name: agent_id.to_string(),
                description: None,
                status: Some("idle".to_string()),
            }],
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
        }
    }

    #[test]
    fn test_capability_index_apply_snapshot_replaces_existing_node_snapshot() {
        let index = CapabilityIndex::new();
        index
            .apply_snapshot(snapshot("node-a", 1, "agent-a"))
            .unwrap();
        index
            .apply_snapshot(snapshot("node-a", 2, "agent-b"))
            .unwrap();

        let snapshots = index.list(Some("node-a"));
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].revision, 2);
        assert_eq!(snapshots[0].agents.len(), 1);
        assert_eq!(snapshots[0].agents[0].agent_id, "agent-b");
    }

    #[test]
    fn test_capability_index_apply_snapshot_rejects_stale_revision() {
        let index = CapabilityIndex::new();
        index
            .apply_snapshot(snapshot("node-a", 2, "agent-b"))
            .unwrap();

        let err = index
            .apply_snapshot(snapshot("node-a", 2, "agent-c"))
            .unwrap_err();
        assert_eq!(err, "stale_capability_snapshot");

        let err = index
            .apply_snapshot(snapshot("node-a", 1, "agent-a"))
            .unwrap_err();
        assert_eq!(err, "stale_capability_snapshot");

        let snapshots = index.list(Some("node-a"));
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].revision, 2);
        assert_eq!(snapshots[0].agents[0].agent_id, "agent-b");
    }

    #[test]
    fn test_capability_index_list_filters_by_node_id() {
        let index = CapabilityIndex::new();
        index
            .apply_snapshot(snapshot("node-a", 1, "agent-a"))
            .unwrap();
        index
            .apply_snapshot(snapshot("node-b", 1, "agent-b"))
            .unwrap();

        let snapshots = index.list(Some("node-b"));
        assert_eq!(snapshots.len(), 1);
        assert_eq!(snapshots[0].node_id, "node-b");

        assert_eq!(index.list(None).len(), 2);
        assert!(index.list(Some("missing")).is_empty());
    }
}
