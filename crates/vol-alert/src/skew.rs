//! Skew alert handler.

use vol_config::SkewConfig;
use vol_core::{Alert, AlertAction, AlertHandler, Result, VolatilityData};

/// Alert handler for skew divergence (put IV vs call IV)
pub struct SkewHandler {
    #[allow(dead_code)]
    config: SkewConfig,
}

impl SkewHandler {
    pub fn new(config: SkewConfig) -> Self {
        Self { config }
    }
}

#[async_trait::async_trait]
impl AlertHandler for SkewHandler {
    fn name(&self) -> &str {
        "skew"
    }

    fn evaluate(&self, _data: &VolatilityData) -> Option<Alert> {
        // Skew alerts require comparing put vs call IV at similar strikes
        // This is a simplified implementation - real version would track
        // put/call IV pairs and calculate the spread

        // For now, placeholder implementation
        //
        // NOTE: When this handler is implemented, it will need to populate
        // the new Alert fields (tenor, alert_type, symbol, value) to support
        // the enriched notification template system.
        None
    }

    async fn on_alert(&self, _alert: &Alert) -> Result<AlertAction> {
        Ok(AlertAction::Send)
    }
}
