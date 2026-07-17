use std::collections::HashMap;
use std::sync::RwLock;

use vol_llm_agent_protocol::agent_server_protocol::{
    NodeLoad, NodeRecord, NodeRegistration, RegisterAck,
};

#[derive(Debug, Clone)]
struct NodeAuth {
    identity: String,
    generation: u64,
}

pub struct NodeRegistry {
    nodes: RwLock<HashMap<String, NodeRecord>>,
    auth: RwLock<HashMap<String, NodeAuth>>,
}

impl NodeRegistry {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            auth: RwLock::new(HashMap::new()),
        }
    }

    #[allow(clippy::expect_used)]
    pub fn register(
        &self,
        reg: NodeRegistration,
        auth_identity: String,
        now_ms: u64,
    ) -> Result<RegisterAck, String> {
        let mut auth = self
            .auth
            .write()
            .expect("node registry auth lock poisoned while registering node");
        let generation = match auth.get(&reg.node_id) {
            Some(existing) if existing.identity != auth_identity => {
                return Err("node_id already registered with different auth identity".to_string());
            }
            Some(existing) => existing.generation + 1,
            None => 1,
        };
        auth.insert(
            reg.node_id.clone(),
            NodeAuth {
                identity: auth_identity,
                generation,
            },
        );

        let mut nodes = self
            .nodes
            .write()
            .expect("node registry nodes lock poisoned while registering node");
        nodes.insert(
            reg.node_id.clone(),
            NodeRecord {
                node_id: reg.node_id.clone(),
                name: reg.name,
                version: reg.version,
                status: "online".to_string(),
                last_seen_at_ms: Some(now_ms),
                capability_revision: 0,
                load: NodeLoad::default(),
            },
        );

        Ok(RegisterAck {
            node_id: reg.node_id,
            accepted: true,
            generation,
        })
    }

    #[allow(clippy::expect_used)]
    pub fn heartbeat(&self, node_id: &str, load: NodeLoad, now_ms: u64) -> Result<(), String> {
        let mut nodes = self
            .nodes
            .write()
            .expect("node registry nodes lock poisoned while recording heartbeat");
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| "node_not_registered".to_string())?;
        node.status = "online".to_string();
        node.last_seen_at_ms = Some(now_ms);
        node.load = load;
        Ok(())
    }

    #[allow(clippy::expect_used)]
    pub fn get(&self, node_id: &str) -> Option<NodeRecord> {
        self.nodes
            .read()
            .expect("node registry nodes lock poisoned while getting node")
            .get(node_id)
            .cloned()
    }

    #[allow(clippy::expect_used)]
    pub fn update_capability_revision(&self, node_id: &str, revision: u64) -> Result<(), String> {
        let mut nodes = self
            .nodes
            .write()
            .expect("NodeRegistry nodes lock poisoned");
        let node = nodes
            .get_mut(node_id)
            .ok_or_else(|| "node_not_registered".to_string())?;
        node.capability_revision = revision;
        Ok(())
    }

    #[allow(clippy::expect_used)]
    pub fn list(&self) -> Vec<NodeRecord> {
        self.nodes
            .read()
            .expect("node registry nodes lock poisoned while listing nodes")
            .values()
            .cloned()
            .collect()
    }
}

impl Default for NodeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent_protocol::agent_server_protocol::{NodeLoad, NodeRegistration};

    #[test]
    fn register_creates_node_record_and_session_generation() {
        let registry = NodeRegistry::new();
        let reg = NodeRegistration {
            node_id: "node-a".to_string(),
            name: "Node A".to_string(),
            version: "0.1.0".to_string(),
        };

        let ack = registry.register(reg, "auth-a".to_string(), 1000).unwrap();
        assert_eq!(ack.node_id, "node-a");
        assert_eq!(ack.generation, 1);

        let node = registry.get("node-a").unwrap();
        assert_eq!(node.node_id, "node-a");
        assert_eq!(node.status, "online");
    }

    #[test]
    fn heartbeat_updates_last_seen_and_load() {
        let registry = NodeRegistry::new();
        registry
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth-a".to_string(),
                1000,
            )
            .unwrap();

        registry
            .heartbeat(
                "node-a",
                NodeLoad {
                    running: 2,
                    queued: 3,
                },
                2000,
            )
            .unwrap();

        let node = registry.get("node-a").unwrap();
        assert_eq!(node.last_seen_at_ms, Some(2000));
        assert_eq!(node.load.running, 2);
        assert_eq!(node.load.queued, 3);
    }

    #[test]
    fn list_returns_registered_nodes() {
        let registry = NodeRegistry::new();
        registry
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth-a".to_string(),
                1000,
            )
            .unwrap();

        let nodes = registry.list();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, "node-a");
    }

    #[test]
    fn update_capability_revision_works_for_registered_node() {
        let registry = NodeRegistry::new();
        registry
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth-a".to_string(),
                1000,
            )
            .unwrap();
        registry.update_capability_revision("node-a", 5).unwrap();
        let node = registry.get("node-a").unwrap();
        assert_eq!(node.capability_revision, 5);
    }

    #[test]
    fn update_capability_revision_fails_for_unregistered_node() {
        let registry = NodeRegistry::new();
        let err = registry
            .update_capability_revision("unknown", 1)
            .unwrap_err();
        assert!(err.contains("not_registered"));
    }

    #[test]
    fn heartbeat_fails_for_unregistered_node() {
        let registry = NodeRegistry::new();
        let err = registry.heartbeat(
            "unknown",
            NodeLoad {
                running: 0,
                queued: 0,
            },
            1000,
        );
        assert!(err.is_err());
    }

    #[test]
    fn register_rejects_different_auth_identity() {
        let registry = NodeRegistry::new();
        registry
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth-a".to_string(),
                1000,
            )
            .unwrap();
        let err = registry
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth-b".to_string(),
                2000,
            )
            .unwrap_err();
        assert!(err.contains("different auth identity"));
    }

    #[test]
    fn register_increments_generation_for_same_auth() {
        let registry = NodeRegistry::new();
        registry
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth-a".to_string(),
                1000,
            )
            .unwrap();
        let ack = registry
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.2.0".to_string(),
                },
                "auth-a".to_string(),
                2000,
            )
            .unwrap();
        assert_eq!(ack.generation, 2);
    }
}
