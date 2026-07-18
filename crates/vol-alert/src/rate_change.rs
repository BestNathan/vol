//! Rate of change alert handler.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use vol_config::RateOfChangeConfig;
use vol_core::{Alert, AlertAction, AlertHandler, AlertType, Result, VolatilityData};

/// Alert handler for IV rate of change
pub struct RateChangeHandler {
    config: RateOfChangeConfig,
    // Rolling buffers per symbol: stores (timestamp, iv) pairs
    #[allow(clippy::type_complexity)]
    buffers: Arc<Mutex<std::collections::HashMap<String, VecDeque<(u64, f64)>>>>,
}

impl RateChangeHandler {
    pub fn new(config: RateOfChangeConfig) -> Self {
        Self {
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
}

#[async_trait::async_trait]
impl AlertHandler for RateChangeHandler {
    fn name(&self) -> &str {
        "rate_change"
    }

    fn evaluate(&self, data: &VolatilityData) -> Option<Alert> {
        let mut buffers = self.buffers.lock().ok()?;
        let buffer = buffers
            .entry(data.symbol.clone())
            .or_insert_with(VecDeque::new);

        // Add new data point
        buffer.push_back((data.timestamp, data.iv));

        // Keep only last 24 hours of data (at 1-min intervals = 1440 samples)
        while buffer.len() > 1500 {
            buffer.pop_front();
        }

        // Get tenor - skip if in gap region (no tenor classification)
        let tenor = data.tenor()?;

        // Check 1h rate of change
        if let Some(change) = self.calculate_rate_change(buffer, 3_600_000) {
            if change.abs() >= self.config.window_1h_threshold {
                return Some(Alert::new(
                    AlertType::RateChange {
                        window_hours: 1,
                        change_pct: change,
                    },
                    tenor,
                    data.symbol.clone(),
                    data.iv,
                    format!(
                        "{} {} IV changed {:.1}% in 1h (threshold: {:.1}%)",
                        data.symbol,
                        tenor,
                        change * 100.0,
                        self.config.window_1h_threshold * 100.0
                    ),
                    data.timestamp,
                    data.source.clone(),
                    data.index_price,
                    data.dte,
                    data.option_type,
                    data.moneyness(),
                    data.extra
                        .get("mark_price_coin")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0),
                    String::new(), // trace_id - set by engine layer
                ));
            }
        }

        // Check 4h rate of change
        if let Some(change) = self.calculate_rate_change(buffer, 14_400_000) {
            if change.abs() >= self.config.window_4h_threshold {
                return Some(Alert::new(
                    AlertType::RateChange {
                        window_hours: 4,
                        change_pct: change,
                    },
                    tenor,
                    data.symbol.clone(),
                    data.iv,
                    format!(
                        "{} {} IV changed {:.1}% in 4h (threshold: {:.1}%)",
                        data.symbol,
                        tenor,
                        change * 100.0,
                        self.config.window_4h_threshold * 100.0
                    ),
                    data.timestamp,
                    data.source.clone(),
                    data.index_price,
                    data.dte,
                    data.option_type,
                    data.moneyness(),
                    data.extra
                        .get("mark_price_coin")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0),
                    String::new(), // trace_id - set by engine layer
                ));
            }
        }

        // Check 24h rate of change
        if let Some(change) = self.calculate_rate_change(buffer, 86_400_000) {
            if change.abs() >= self.config.window_24h_threshold {
                return Some(Alert::new(
                    AlertType::RateChange {
                        window_hours: 24,
                        change_pct: change,
                    },
                    tenor,
                    data.symbol.clone(),
                    data.iv,
                    format!(
                        "{} {} IV changed {:.1}% in 24h (threshold: {:.1}%)",
                        data.symbol,
                        tenor,
                        change * 100.0,
                        self.config.window_24h_threshold * 100.0
                    ),
                    data.timestamp,
                    data.source.clone(),
                    data.index_price,
                    data.dte,
                    data.option_type,
                    data.moneyness(),
                    data.extra
                        .get("mark_price_coin")
                        .and_then(serde_json::Value::as_f64)
                        .unwrap_or(0.0),
                    String::new(), // trace_id - set by engine layer
                ));
            }
        }

        None
    }

    async fn on_alert(&self, _alert: &Alert) -> Result<AlertAction> {
        Ok(AlertAction::Send)
    }
}
