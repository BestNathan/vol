use vol_llm_agent_protocol::agent_server_protocol::{
    AgentCapability, CapabilitySnapshot, NodeLoad,
};

#[async_trait::async_trait]
pub trait RuntimeCapabilitySource {
    async fn snapshot_capabilities(&self) -> CapabilitySnapshot;
    async fn current_load(&self) -> NodeLoad;
}

pub struct StaticCapabilitySource {
    pub node_id: String,
}

#[async_trait::async_trait]
impl RuntimeCapabilitySource for StaticCapabilitySource {
    async fn snapshot_capabilities(&self) -> CapabilitySnapshot {
        CapabilitySnapshot {
            node_id: self.node_id.clone(),
            revision: 1,
            generated_at_ms: None,
            agents: Vec::<AgentCapability>::new(),
            tools: vec![],
            mcp_servers: vec![],
            skills: vec![],
        }
    }

    async fn current_load(&self) -> NodeLoad {
        NodeLoad {
            running: 0,
            queued: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource;

    #[async_trait::async_trait]
    impl RuntimeCapabilitySource for FakeSource {
        async fn snapshot_capabilities(&self) -> CapabilitySnapshot {
            CapabilitySnapshot {
                node_id: "node-a".to_string(),
                revision: 1,
                generated_at_ms: Some(1000),
                agents: vec![AgentCapability {
                    agent_id: "coding".to_string(),
                    name: "coding".to_string(),
                    description: Some("Coding agent".to_string()),
                    status: Some("idle".to_string()),
                }],
                tools: vec![],
                mcp_servers: vec![],
                skills: vec![],
            }
        }

        async fn current_load(&self) -> NodeLoad {
            NodeLoad {
                running: 0,
                queued: 0,
            }
        }
    }

    #[tokio::test]
    async fn fake_source_returns_snapshot() {
        let snapshot = FakeSource.snapshot_capabilities().await;
        assert_eq!(snapshot.node_id, "node-a");
        assert_eq!(snapshot.revision, 1);
        assert_eq!(snapshot.agents[0].agent_id, "coding");
    }

    #[tokio::test]
    async fn static_source_returns_empty_snapshot() {
        let source = StaticCapabilitySource {
            node_id: "test-node".to_string(),
        };
        let snapshot = source.snapshot_capabilities().await;
        assert_eq!(snapshot.node_id, "test-node");
        assert_eq!(snapshot.revision, 1);
        assert!(snapshot.agents.is_empty());
        assert!(snapshot.tools.is_empty());
        assert!(snapshot.mcp_servers.is_empty());
        assert!(snapshot.skills.is_empty());
    }

    #[tokio::test]
    async fn static_source_returns_zero_load() {
        let source = StaticCapabilitySource {
            node_id: "test-node".to_string(),
        };
        let load = source.current_load().await;
        assert_eq!(load.running, 0);
        assert_eq!(load.queued, 0);
    }
}
