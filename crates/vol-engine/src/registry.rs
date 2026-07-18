//! Rule and notification registry with hot reload support.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;
use vol_core::{NotificationHandler, RuleProcessor};

/// Runtime registry for rules and notifications
pub struct RuleRegistry {
    rules: Arc<RwLock<HashMap<String, Box<dyn RuleProcessor>>>>,
    notification_map: Arc<RwLock<HashMap<String, Arc<dyn NotificationHandler>>>>,
}

impl RuleRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(HashMap::new())),
            notification_map: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a rule processor
    pub async fn register_rule(&self, rule: Box<dyn RuleProcessor>) {
        let id = rule.id().to_string();
        let mut rules = self.rules.write().await;
        info!("Registered rule: {}", id);
        rules.insert(id, rule);
    }

    /// Register a notification handler
    pub async fn register_notification(&self, handler: Arc<dyn NotificationHandler>) {
        let id = handler.name().to_string();
        let mut map = self.notification_map.write().await;
        info!("Registered notification: {}", id);
        map.insert(id, handler);
    }

    /// Get notification handlers for a rule
    pub async fn get_notifications_for_rule(
        &self,
        _rule_id: &str,
        notification_ids: &[String],
    ) -> Vec<Arc<dyn NotificationHandler>> {
        let map = self.notification_map.read().await;
        notification_ids
            .iter()
            .filter_map(|id| map.get(id).cloned())
            .collect()
    }

    /// Get all rules
    pub async fn get_all_rules(&self) -> Vec<Box<dyn RuleProcessor>> {
        let rules = self.rules.read().await;
        rules
            .values()
            .map(vol_core::RuleProcessor::clone_box_rule)
            .collect()
    }

    /// Hot reload: replace all rules
    pub async fn reload_rules(&self, new_rules: Vec<Box<dyn RuleProcessor>>) {
        let mut rules = self.rules.write().await;
        let count = new_rules.len();
        *rules = new_rules
            .into_iter()
            .map(|r| (r.id().to_string(), r))
            .collect();
        info!("Hot reloaded {} rules", count);
    }

    /// Hot reload: replace all notifications
    pub async fn reload_notifications(&self, new_notifs: Vec<Arc<dyn NotificationHandler>>) {
        let mut map = self.notification_map.write().await;
        let count = new_notifs.len();
        *map = new_notifs
            .into_iter()
            .map(|n| (n.name().to_string(), n))
            .collect();
        info!("Hot reloaded {} notifications", count);
    }
}

impl Default for RuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}
