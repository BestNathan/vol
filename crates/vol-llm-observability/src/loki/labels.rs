//! Loki label building utilities.
//!
//! Labels are low-cardinality key-value pairs used by Loki for indexing.
//! High-cardinality fields (run_id, session_id) are placed in the log line,
//! not as labels, to avoid Loki performance issues.

use std::collections::HashMap;

/// Fixed namespace label value.
pub const NAMESPACE: &str = "agent";

/// Build Loki labels for an agent event.
///
/// # Labels
/// - `namespace`: Fixed to `"agent"`
/// - `agent`: Agent type (e.g., `"coding"`, `"advice"`, `"qa"`, `"ppt"`)
/// - `agent_id`: Agent instance identifier from `AgentConfig`
#[derive(Debug, Clone)]
pub struct LokiLabels {
    labels: HashMap<String, String>,
}

impl LokiLabels {
    pub fn new(agent_type: &str, agent_id: &str) -> Self {
        let mut labels = HashMap::new();
        labels.insert("namespace".to_string(), NAMESPACE.to_string());
        labels.insert("agent".to_string(), agent_type.to_string());
        labels.insert("agent_id".to_string(), agent_id.to_string());
        Self { labels }
    }

    pub fn into_inner(self) -> HashMap<String, String> {
        self.labels
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.labels.get(key).map(|v| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_labels_creation() {
        let labels = LokiLabels::new("coding", "agent-001");
        assert_eq!(labels.get("namespace"), Some("agent"));
        assert_eq!(labels.get("agent"), Some("coding"));
        assert_eq!(labels.get("agent_id"), Some("agent-001"));
    }

    #[test]
    fn test_labels_into_inner() {
        let labels = LokiLabels::new("qa", "agent-xyz");
        let map = labels.into_inner();
        assert_eq!(map.len(), 3);
        assert_eq!(map["namespace"], "agent");
    }
}
