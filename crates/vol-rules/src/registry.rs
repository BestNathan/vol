//! Rule registry for dynamic rule management.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use vol_core::{Alert, EventType, MonitoringEvent, RuleProcessor};

/// Registry for managing rules dynamically
pub struct RuleRegistry {
    rules: Arc<RwLock<HashMap<String, Box<dyn RuleProcessor>>>>,
}

impl RuleRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a rule
    pub async fn register(&self, rule: Box<dyn RuleProcessor>) {
        let mut rules = self.rules.write().await;
        rules.insert(rule.id().to_string(), rule);
    }

    /// Unregister a rule by name
    pub async fn unregister(&self, name: &str) -> Option<Box<dyn RuleProcessor>> {
        let mut rules = self.rules.write().await;
        rules.remove(name)
    }

    /// Get all rules interested in a specific event type
    pub async fn get_interested_rules(
        &self,
        event_type: &EventType,
    ) -> Vec<Box<dyn RuleProcessor>> {
        let rules = self.rules.read().await;
        rules
            .values()
            .filter(|r| r.interests().contains(event_type))
            .map(vol_core::RuleProcessor::clone_box_rule)
            .collect()
    }

    /// Evaluate an event against all registered rules
    pub async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
        let rules = self.rules.read().await;
        let event_type = event.event_type();

        let mut alerts = Vec::new();
        for rule in rules.values() {
            if rule.interests().contains(&event_type) {
                let rule_alerts = rule.evaluate(event).await;
                alerts.extend(rule_alerts);
            }
        }
        alerts
    }

    /// Get count of registered rules
    pub async fn len(&self) -> usize {
        self.rules.read().await.len()
    }

    /// Check if the registry has no rules registered
    pub async fn is_empty(&self) -> bool {
        self.rules.read().await.is_empty()
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_registry_basic() {
        let registry = RuleRegistry::new();
        assert_eq!(registry.len().await, 0);
    }
}
