//! WebSocket message protocol types.

use chrono::Utc;
use serde::{Deserialize, Serialize};

/// Unified message envelope for WebSocket communication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsMessage {
    pub message_type: String,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub task_id: Option<String>,
    #[serde(default)]
    pub target_agent_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub payload: serde_json::Value,
}

impl WsMessage {
    /// Create an agent->control message.
    pub fn agent_report(message_type: &str, agent_id: &str, payload: serde_json::Value) -> Self {
        Self {
            message_type: message_type.to_string(),
            agent_id: Some(agent_id.to_string()),
            task_id: None,
            target_agent_id: None,
            timestamp: Some(Utc::now().to_rfc3339()),
            payload,
        }
    }

    /// Create a control->agent message.
    pub fn control_command(
        message_type: &str,
        target_agent_id: &str,
        task_id: &str,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            message_type: message_type.to_string(),
            agent_id: None,
            task_id: Some(task_id.to_string()),
            target_agent_id: Some(target_agent_id.to_string()),
            timestamp: Some(Utc::now().to_rfc3339()),
            payload,
        }
    }

    /// Create an error response to agent.
    pub fn error(agent_id: &str, error: &str) -> Self {
        Self {
            message_type: "error".to_string(),
            agent_id: Some(agent_id.to_string()),
            task_id: None,
            target_agent_id: None,
            timestamp: Some(Utc::now().to_rfc3339()),
            payload: serde_json::json!({"error": error}),
        }
    }
}

// --- Payload types ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterPayload {
    pub name: String,
    pub r#type: String,
    pub version: String,
    pub capabilities: Vec<String>,
    pub host_info: HostInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterAckPayload {
    pub agent_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeartbeatPayload {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub load: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricPayload {
    pub samples: Vec<MetricSample>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricSample {
    pub name: String,
    pub value: f64,
    #[serde(default)]
    pub labels: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPayload {
    #[serde(default)]
    pub run_id: Option<String>,
    pub event_name: String,
    #[serde(default)]
    pub severity: Option<String>,
    #[serde(default)]
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPayload {
    pub task_type: String,
    #[serde(default)]
    pub parameters: serde_json::Value,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResultPayload {
    pub status: String,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostInfo {
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub ip: String,
}
