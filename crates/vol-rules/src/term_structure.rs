//! Term structure rule.

use vol_core::{RuleProcessor, MonitoringEvent, Alert, EventType, RuleAction, Result, VolatilityData};
use vol_config::TermStructureConfig;

/// Term structure rule for spread between short and long tenors
#[derive(Clone)]
pub struct TermStructureRule {
    #[allow(dead_code)]
    config: TermStructureConfig,
}

impl TermStructureRule {
    pub fn new(config: TermStructureConfig) -> Self {
        Self { config }
    }

    #[allow(dead_code)]
    fn evaluate_volatility(&self, _data: &VolatilityData) -> Option<Alert> {
        // Term structure alerts require comparing short vs long tenor IVs
        // This is a simplified implementation - real version would track
        // IV levels across tenors and compare them

        // For now, we'll just flag if we see unusual IV levels in context
        // A proper implementation would maintain state across symbols/tenors

        // Placeholder: just return None for now since term structure
        // requires multi-instrument comparison that needs more state
        None
    }
}

#[async_trait::async_trait]
impl RuleProcessor for TermStructureRule {
    fn name(&self) -> &str {
        "term_structure"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Volatility]
    }

    fn evaluate(&self, _event: &MonitoringEvent) -> Option<Alert> {
        // Placeholder implementation
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
