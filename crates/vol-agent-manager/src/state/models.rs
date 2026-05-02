use chrono::{DateTime, Utc};
use serde::Serialize;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum AgentStatus {
    Connected,
    Idle,
    Busy,
    Disconnected,
    Dead,
}

#[derive(Debug, Clone, Serialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub ip: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AgentState {
    pub agent_id: String,
    pub name: String,
    pub r#type: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub host_info: HostInfo,
    pub status: AgentStatus,
    pub connected_at: DateTime<Utc>,
    pub last_heartbeat: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_status_serialization() {
        let status = AgentStatus::Connected;
        let json = serde_json::to_string(&status).unwrap();
        assert!(json.contains("Connected"));
    }

    #[test]
    fn test_agent_state_creation() {
        let state = AgentState {
            agent_id: "repo:test-agent".to_string(),
            name: "test-agent".to_string(),
            r#type: "test".to_string(),
            version: "0.1.0".to_string(),
            capabilities: vec!["Read".to_string()],
            host_info: HostInfo {
                hostname: "host1".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                ip: "10.0.0.1".to_string(),
            },
            status: AgentStatus::Connected,
            connected_at: Utc::now(),
            last_heartbeat: Utc::now(),
        };
        assert_eq!(state.agent_id, "repo:test-agent");
        assert_eq!(state.status, AgentStatus::Connected);
    }
}
