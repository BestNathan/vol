//! Term structure alert handler.

use vol_core::{AlertHandler, Alert, AlertAction, VolatilityData, Result};
use vol_config::TermStructureConfig;

/// Alert handler for term structure anomalies (spread between short and long tenors)
pub struct TermStructureHandler {
    #[allow(dead_code)]
    config: TermStructureConfig,
}

impl TermStructureHandler {
    pub fn new(config: TermStructureConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl AlertHandler for TermStructureHandler {
    fn name(&self) -> &str {
        "term_structure"
    }

    fn evaluate(&self, _data: &VolatilityData) -> Option<Alert> {
        // Term structure alerts require comparing short vs long tenor IVs
        // This is a simplified implementation - real version would track
        // IV levels across tenors and compare them

        // For now, we'll just flag if we see unusual IV levels in context
        // A proper implementation would maintain state across symbols/tenors

        // Placeholder: just return None for now since term structure
        // requires multi-instrument comparison that needs more state
        None
    }

    async fn on_alert(&self, _alert: &Alert) -> Result<AlertAction> {
        Ok(AlertAction::Send)
    }
}
