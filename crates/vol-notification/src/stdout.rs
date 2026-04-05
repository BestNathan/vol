//! Stdout notification handler for testing.

use vol_core::{NotificationHandler, Alert, Result};
use tracing::{info, info_span};

/// Stdout notification handler - prints alerts to console
#[derive(Clone)]
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
        // Use trace_id from alert (set by engine layer)
        let trace_id = &alert.trace_id;

        // Create span for notification with business attributes
        let span = info_span!(
            "notification_send",
            channel = "stdout",
            alert_type = %alert.alert_type,
            tenor = ?alert.tenor,
            symbol = %alert.symbol,
            iv = %alert.iv,
            trace_id = %trace_id,
        );

        // Record additional alert attributes
        span.record("alert.dte", &alert.dte);
        span.record("alert.index_price", &alert.index_price);
        span.record("alert.option_type", &alert.option_type.to_string());

        let _guard = span.enter();

        tracing::info!(
            trace_id = %trace_id,
            "notification sent to stdout"
        );

        let underlying = alert.symbol.split('-').next().unwrap_or("BTC").to_uppercase();
        let message = format!(
            "[ALERT] {} | {} | {} | IV: {:.1}% | 指数：{:.2} | DTE: {}天 | {} | 价格：{:.4} {} ({:.2} USD)",
            alert.tenor,
            alert.alert_type,
            alert.symbol,
            alert.iv * 100.0,
            alert.index_price,
            alert.dte,
            alert.option_type,
            alert.mark_price_coin,
            underlying,
            alert.mark_price_usd(),
        );
        info!("{}", message);
        Ok(())
    }

    fn clone_box(&self) -> Box<dyn NotificationHandler> {
        Box::new(self.clone())
    }
}
