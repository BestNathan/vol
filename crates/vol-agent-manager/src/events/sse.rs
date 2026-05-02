use chrono::{DateTime, Utc};
use serde::Serialize;
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Internal event emitted by the control plane.
#[derive(Debug, Clone, Serialize)]
pub struct ManagerEvent {
    pub event_type: String,
    pub agent_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl ManagerEvent {
    pub fn agent_registered(agent_id: &str) -> Self {
        Self {
            event_type: "agent_registered".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: None,
        }
    }

    pub fn agent_disconnected(agent_id: &str) -> Self {
        Self {
            event_type: "agent_disconnected".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: None,
        }
    }

    pub fn agent_dead(agent_id: &str) -> Self {
        Self {
            event_type: "agent_dead".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: None,
        }
    }

    pub fn task_dispatched(task_id: &str, agent_id: &str) -> Self {
        Self {
            event_type: "task_dispatched".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id})),
        }
    }

    pub fn task_completed(task_id: &str, agent_id: &str) -> Self {
        Self {
            event_type: "task_completed".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id})),
        }
    }

    pub fn task_failed(task_id: &str, agent_id: &str, error: &str) -> Self {
        Self {
            event_type: "task_failed".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id, "error": error})),
        }
    }

    pub fn task_timeout(task_id: &str, agent_id: &str) -> Self {
        Self {
            event_type: "task_timeout".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"task_id": task_id})),
        }
    }

    pub fn agent_event(agent_id: &str, event_name: &str, data: serde_json::Value) -> Self {
        Self {
            event_type: "agent_event".to_string(),
            agent_id: agent_id.to_string(),
            timestamp: Utc::now(),
            data: Some(serde_json::json!({"event_name": event_name, "data": data})),
        }
    }

    pub fn to_json_string(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Broadcast bus for manager events.
pub struct EventBus {
    tx: broadcast::Sender<ManagerEvent>,
    /// Append-only event log (used by drain for testing and replay).
    log: Arc<Mutex<Vec<ManagerEvent>>>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self {
            tx,
            log: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn emit(&self, event: ManagerEvent) {
        self.log.lock().unwrap().push(event.clone());
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ManagerEvent> {
        self.tx.subscribe()
    }

    /// Drain all pending events (for testing).
    pub fn drain(&self) -> Vec<ManagerEvent> {
        let mut log = self.log.lock().unwrap();
        std::mem::take(&mut *log)
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_emit_and_receive() {
        let bus = EventBus::new();
        bus.emit(ManagerEvent::agent_registered("agent-1"));
        let events = bus.drain();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "agent_registered");
    }

    #[tokio::test]
    async fn test_event_serialization() {
        let event = ManagerEvent::agent_registered("agent-1");
        let json = event.to_json_string();
        assert!(json.contains("agent_registered"));
        assert!(json.contains("agent-1"));
    }

    #[test]
    fn test_agent_dead_event() {
        let event = ManagerEvent::agent_dead("agent-1");
        assert_eq!(event.event_type, "agent_dead");
        assert_eq!(event.agent_id, "agent-1");
    }
}
