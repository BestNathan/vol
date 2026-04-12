//! Term structure rule.

use vol_config::TermStructureConfig;
use vol_core::{Alert, EventType, MonitoringEvent, Result, RuleAction, RuleProcessor};

/// Term structure rule for spread between short and long tenors
#[derive(Clone)]
pub struct TermStructureRule {
    #[allow(dead_code)]
    config: TermStructureConfig,
    id: String,
}

impl TermStructureRule {
    pub fn new(config: TermStructureConfig, id: String) -> Self {
        Self { config, id }
    }
}

#[async_trait::async_trait]
impl RuleProcessor for TermStructureRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn rule_type(&self) -> &str {
        "term-structure"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Volatility]
    }

    async fn evaluate(&self, _event: &MonitoringEvent) -> Vec<Alert> {
        // Placeholder implementation - term structure requires multi-instrument comparison
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
