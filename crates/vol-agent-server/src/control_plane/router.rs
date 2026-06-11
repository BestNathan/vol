use crate::control_plane::capability::CapabilityIndex;
use crate::control_plane::registry::NodeRegistry;

pub struct ControlRouter<'a> {
    nodes: &'a NodeRegistry,
    capabilities: &'a CapabilityIndex,
}

impl<'a> ControlRouter<'a> {
    pub fn new(nodes: &'a NodeRegistry, capabilities: &'a CapabilityIndex) -> Self {
        Self {
            nodes,
            capabilities,
        }
    }

    pub fn route_agent(&self, target: Option<&str>) -> Result<String, String> {
        let snapshots = self.capabilities.list(None);
        for snapshot in snapshots {
            let is_online = self
                .nodes
                .get(&snapshot.node_id)
                .map(|node| node.status == "online")
                .unwrap_or(false);

            if !is_online {
                continue;
            }

            if let Some(target) = target {
                if snapshot
                    .agents
                    .iter()
                    .any(|agent| agent.agent_id == target || agent.name == target)
                {
                    return Ok(snapshot.node_id);
                }
            } else if !snapshot.agents.is_empty() {
                return Ok(snapshot.node_id);
            }
        }

        Err("capability_not_found".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::control_plane::capability::CapabilityIndex;
    use crate::control_plane::registry::NodeRegistry;
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentCapability, CapabilitySnapshot, NodeRegistration,
    };

    #[test]
    fn route_agent_prefers_node_with_agent_capability() {
        let nodes = NodeRegistry::new();
        nodes
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

        let capabilities = CapabilityIndex::new();
        capabilities
            .apply_snapshot(CapabilitySnapshot {
                node_id: "node-a".to_string(),
                revision: 1,
                generated_at_ms: Some(1000),
                agents: vec![AgentCapability {
                    agent_id: "coding".to_string(),
                    name: "coding".to_string(),
                    description: None,
                    status: Some("idle".to_string()),
                }],
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
            })
            .unwrap();

        let router = ControlRouter::new(&nodes, &capabilities);
        assert_eq!(router.route_agent(Some("coding")).unwrap(), "node-a");
    }

    #[test]
    fn route_agent_returns_none_for_empty_capabilities() {
        let nodes = NodeRegistry::new();
        let capabilities = CapabilityIndex::new();
        let router = ControlRouter::new(&nodes, &capabilities);
        let err = router.route_agent(None).unwrap_err();
        assert_eq!(err, "capability_not_found");
    }

    #[test]
    fn route_agent_picks_first_online_node_with_agents_when_no_target() {
        let nodes = NodeRegistry::new();
        nodes
            .register(
                NodeRegistration {
                    node_id: "node-a".to_string(),
                    name: "Node A".to_string(),
                    version: "0.1.0".to_string(),
                },
                "auth".to_string(),
                1000,
            )
            .unwrap();

        let capabilities = CapabilityIndex::new();
        capabilities
            .apply_snapshot(CapabilitySnapshot {
                node_id: "node-a".to_string(),
                revision: 1,
                generated_at_ms: Some(1000),
                agents: vec![AgentCapability {
                    agent_id: "agent-a".to_string(),
                    name: "agent-a".to_string(),
                    description: None,
                    status: Some("idle".to_string()),
                }],
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
            })
            .unwrap();

        let router = ControlRouter::new(&nodes, &capabilities);
        assert_eq!(router.route_agent(None).unwrap(), "node-a");
    }
}
