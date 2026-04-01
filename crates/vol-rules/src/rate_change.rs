//! Rate of change rule.

use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use vol_core::{RuleProcessor, MonitoringEvent, Alert, AlertType, EventType, RuleAction, Result, VolatilityData};
use vol_config::RateChangeRuleConfig;

/// Rate of change rule for IV changes over time
pub struct RateChangeRule {
    config: RateChangeRuleConfig,
    id: String,
    // Rolling buffers per symbol: stores (timestamp, iv) pairs
    buffers: Arc<Mutex<std::collections::HashMap<String, VecDeque<(u64, f64)>>>>,
}

impl RateChangeRule {
    pub fn new(config: RateChangeRuleConfig) -> Self {
        Self {
            id: config.id.clone(),
            config,
            buffers: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    fn calculate_rate_change(&self, buffer: &VecDeque<(u64, f64)>, window_ms: u64) -> Option<f64> {
        if buffer.len() < 2 {
            return None;
        }

        let now = buffer.back()?.0;
        let window_start = now.saturating_sub(window_ms);

        // Find oldest sample in window
        let mut oldest_iv = None;
        for &(ts, iv) in buffer.iter() {
            if ts >= window_start {
                oldest_iv = Some(iv);
                break;
            }
        }

        let oldest = oldest_iv?;
        let latest = buffer.back()?.1;

        Some((latest - oldest) / oldest)
    }

    async fn evaluate_volatility_async(&self, data: &VolatilityData) -> Vec<Alert> {
        let mut alerts = Vec::new();
        let mut buffers = self.buffers.lock().await;
        let buffer = buffers.entry(data.symbol.clone()).or_insert_with(VecDeque::new);

        // Add new data point
        buffer.push_back((data.timestamp, data.iv));

        // Keep only last 24 hours of data (at 1-min intervals = 1440 samples)
        while buffer.len() > 1500 {
            buffer.pop_front();
        }

        // Check 1h rate of change
        if let Some(change) = self.calculate_rate_change(buffer, 3_600_000) {
            if change.abs() >= self.config.window_1h_threshold {
                alerts.push(Alert::new(
                    AlertType::RateChange { window_hours: 1, change_pct: change },
                    data.tenor(),
                    data.symbol.clone(),
                    data.iv,
                    format!(
                        "{} {} IV changed {:.1}% in 1h (threshold: {:.1}%)",
                        data.symbol, data.tenor(),
                        change * 100.0, self.config.window_1h_threshold * 100.0
                    ),
                    data.timestamp,
                    data.source.clone(),
                    data.index_price,
                    data.dte,
                    data.option_type,
                    data.moneyness(),
                    data.extra.get("mark_price_coin").and_then(|v| v.as_f64()).unwrap_or(0.0),
                ));
            }
        }

        // Check 4h rate of change
        if let Some(change) = self.calculate_rate_change(buffer, 14_400_000) {
            if change.abs() >= self.config.window_4h_threshold {
                alerts.push(Alert::new(
                    AlertType::RateChange { window_hours: 4, change_pct: change },
                    data.tenor(),
                    data.symbol.clone(),
                    data.iv,
                    format!(
                        "{} {} IV changed {:.1}% in 4h (threshold: {:.1}%)",
                        data.symbol, data.tenor(),
                        change * 100.0, self.config.window_4h_threshold * 100.0
                    ),
                    data.timestamp,
                    data.source.clone(),
                    data.index_price,
                    data.dte,
                    data.option_type,
                    data.moneyness(),
                    data.extra.get("mark_price_coin").and_then(|v| v.as_f64()).unwrap_or(0.0),
                ));
            }
        }

        // Check 24h rate of change
        if let Some(change) = self.calculate_rate_change(buffer, 86_400_000) {
            if change.abs() >= self.config.window_24h_threshold {
                alerts.push(Alert::new(
                    AlertType::RateChange { window_hours: 24, change_pct: change },
                    data.tenor(),
                    data.symbol.clone(),
                    data.iv,
                    format!(
                        "{} {} IV changed {:.1}% in 24h (threshold: {:.1}%)",
                        data.symbol, data.tenor(),
                        change * 100.0, self.config.window_24h_threshold * 100.0
                    ),
                    data.timestamp,
                    data.source.clone(),
                    data.index_price,
                    data.dte,
                    data.option_type,
                    data.moneyness(),
                    data.extra.get("mark_price_coin").and_then(|v| v.as_f64()).unwrap_or(0.0),
                ));
            }
        }

        alerts
    }
}

impl Clone for RateChangeRule {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            config: self.config.clone(),
            buffers: Arc::new(Mutex::new(self.buffers.try_lock().map(|b| b.clone()).unwrap_or_default())),
        }
    }
}

#[async_trait::async_trait]
impl RuleProcessor for RateChangeRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn rule_type(&self) -> &str {
        "rate-change"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Volatility]
    }

    async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
        let MonitoringEvent::Volatility(vol_data) = event else {
            return vec![];
        };
        self.evaluate_volatility_async(vol_data).await
    }

    fn notification_ids(&self) -> Vec<String> {
        self.config.notifications.clone()
    }

    async fn on_alert(&self, _alert: &Alert) -> Result<RuleAction> {
        Ok(RuleAction::Continue)
    }

    fn clone_box_rule(&self) -> Box<dyn RuleProcessor> {
        Box::new(self.clone())
    }
}
