//! Stdout notification handler for testing.

use vol_core::{NotificationHandler, Alert, Result};
use tracing::info;

/// Stdout notification handler - prints alerts to console
pub struct StdoutNotification;

impl StdoutNotification {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StdoutNotification {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl NotificationHandler for StdoutNotification {
    fn name(&self) -> &str {
        "stdout"
    }

    async fn send(&self, alert: &Alert) -> Result<()> {
        let message = format!(
            "[ALERT] {} | {} | {} | IV: {:.1}% | {}",
            alert.tenor,
            alert.alert_type,
            alert.symbol,
            alert.iv * 100.0,
            alert.message
        );
        info!("{}", message);
        println!("{}", message);
        Ok(())
    }
}
