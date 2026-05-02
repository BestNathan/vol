use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::state::models::{AgentState, AgentStatus};

/// Thread-safe store for agent states.
pub struct AgentStateManager {
    agents: Arc<RwLock<HashMap<String, AgentState>>>,
}

impl AgentStateManager {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register or re-register an agent (overwrites existing).
    pub async fn register(&self, state: AgentState) {
        let mut guard = self.agents.write().await;
        guard.insert(state.agent_id.clone(), state);
    }

    /// Get agent state by ID.
    pub async fn get(&self, agent_id: &str) -> Option<AgentState> {
        let guard = self.agents.read().await;
        guard.get(agent_id).cloned()
    }

    /// Update heartbeat timestamp.
    pub async fn update_heartbeat(&self, agent_id: &str) {
        let mut guard = self.agents.write().await;
        if let Some(state) = guard.get_mut(agent_id) {
            state.last_heartbeat = chrono::Utc::now();
        }
    }

    /// Update agent status.
    pub async fn update_status(&self, agent_id: &str, status: AgentStatus) {
        let mut guard = self.agents.write().await;
        if let Some(state) = guard.get_mut(agent_id) {
            state.status = status;
        }
    }

    /// List all agents.
    pub async fn list_all(&self) -> Vec<AgentState> {
        let guard = self.agents.read().await;
        guard.values().cloned().collect()
    }
}

impl Default for AgentStateManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::models::{AgentStatus, HostInfo};
    use chrono::Utc;

    fn make_state(id: &str) -> AgentState {
        AgentState {
            agent_id: id.to_string(),
            name: id.to_string(),
            r#type: "test".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec![],
            host_info: HostInfo {
                hostname: "h".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                ip: "127.0.0.1".to_string(),
            },
            status: AgentStatus::Connected,
            connected_at: Utc::now(),
            last_heartbeat: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_register_and_get() {
        let mgr = AgentStateManager::new();
        let state = make_state("agent-1");
        mgr.register(state).await;
        let got = mgr.get("agent-1").await;
        assert!(got.is_some());
        assert_eq!(got.unwrap().agent_id, "agent-1");
    }

    #[tokio::test]
    async fn test_list_all() {
        let mgr = AgentStateManager::new();
        mgr.register(make_state("a")).await;
        mgr.register(make_state("b")).await;
        let all = mgr.list_all().await;
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_update_heartbeat() {
        let mgr = AgentStateManager::new();
        mgr.register(make_state("agent-1")).await;
        let before = mgr.get("agent-1").await.unwrap().last_heartbeat;
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        mgr.update_heartbeat("agent-1").await;
        let after = mgr.get("agent-1").await.unwrap().last_heartbeat;
        assert!(after > before);
    }

    #[tokio::test]
    async fn test_update_status() {
        let mgr = AgentStateManager::new();
        mgr.register(make_state("agent-1")).await;
        mgr.update_status("agent-1", AgentStatus::Busy).await;
        let state = mgr.get("agent-1").await.unwrap();
        assert_eq!(state.status, AgentStatus::Busy);
    }

    #[tokio::test]
    async fn test_register_overwrites_existing() {
        let mgr = AgentStateManager::new();
        let mut s1 = make_state("dup");
        s1.version = "v1".to_string();
        mgr.register(s1).await;

        let mut s2 = make_state("dup");
        s2.version = "v2".to_string();
        mgr.register(s2).await;

        let got = mgr.get("dup").await.unwrap();
        assert_eq!(got.version, "v2");
    }

    #[tokio::test]
    async fn test_get_nonexistent() {
        let mgr = AgentStateManager::new();
        assert!(mgr.get("nope").await.is_none());
    }
}
