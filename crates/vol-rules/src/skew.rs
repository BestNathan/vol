//! Skew rule.

use vol_core::{RuleProcessor, MonitoringEvent, Alert, EventType, RuleAction, Result};
use vol_config::SkewConfig;

/// Skew rule for put/call IV divergence
#[derive(Clone)]
pub struct SkewRule {
    #[allow(dead_code)]
    config: SkewConfig,
}

impl SkewRule {
    pub fn new(config: SkewConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl RuleProcessor for SkewRule {
    fn name(&self) -> &str {
        "skew"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Volatility]
    }

    fn evaluate(&self, _event: &MonitoringEvent) -> Option<Alert> {
        // Skew alerts require comparing put vs call IV at similar strikes
        // This is a simplified implementation - real version would track
        // put/call IV pairs and calculate the spread

        // For now, placeholder implementation
        //
        // NOTE: When this rule is fully implemented, it will need to populate
        // the new Alert fields (tenor, alert_type, symbol, value) to support
        // the enriched notification template system.
        None
    }

    async fn on_alert(&self, _alert: &Alert) -> Result<RuleAction> {
        Ok(RuleAction::Continue)
    }

    fn clone_box_rule(&self) -> Box<dyn RuleProcessor> {
        Box::new(self.clone())
    }
}
