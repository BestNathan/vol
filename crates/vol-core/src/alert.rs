use crate::models::VolatilityData;
use crate::event::Alert;
use crate::error::Result;
use async_trait::async_trait;

/// Action to take after an alert fires
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlertAction {
    /// Send the alert and continue monitoring
    Send,
    /// Suppress this alert (e.g., due to cooldown)
    Suppress,
    /// Stop monitoring this condition
    Stop,
}

/// AlertHandler trait - evaluates volatility data and emits alerts.
///
/// All alert handler plugins (absolute_iv, rate_change, etc.) must implement this trait.
#[async_trait]
pub trait AlertHandler: Send + Sync {
    /// Returns the name of this alert handler (e.g., "absolute_iv", "rate_change")
    fn name(&self) -> &str;

    /// Evaluate incoming data and optionally return an Alert.
    /// Returns None if no alert should be fired.
    fn evaluate(&self, data: &VolatilityData) -> Option<Alert>;

    /// Called when an alert fires. Returns the action to take.
    #[allow(async_fn_in_trait)]
    async fn on_alert(&self, alert: &Alert) -> Result<AlertAction>;
}
