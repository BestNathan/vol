//! Skew rule.

use vol_config::SkewConfig;
use vol_core::{Alert, EventType, MonitoringEvent, Result, RuleAction, RuleProcessor};

/// Skew rule for put/call IV divergence
#[derive(Clone)]
pub struct SkewRule {
    #[allow(dead_code)]
    config: SkewConfig,
    id: String,
}

impl SkewRule {
    pub fn new(config: SkewConfig, id: String) -> Self {
        Self { config, id }
    }
}

#[async_trait::async_trait]
impl RuleProcessor for SkewRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn rule_type(&self) -> &str {
        "skew"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Volatility]
    }

    async fn evaluate(&self, _event: &MonitoringEvent) -> Vec<Alert> {
        // Skew alerts require comparing put vs call IV at similar strikes
        // Placeholder implementation
        vec![]
    }

    fn notification_ids(&self) -> Vec<String> {
        vec![]
    }

    async fn on_alert(&self, _alert: &Alert) -> Result<RuleAction> {
        Ok(RuleAction::Continue)
    }

    fn clone_box_rule(&self) -> Box<dyn RuleProcessor> {
        Box::new(self.clone())
    }
}
