//! Absolute IV threshold rule.

use vol_config::AbsoluteIvRuleConfig;
use vol_core::{
    Alert, AlertType, EventType, MonitoringEvent, Result, RuleAction, RuleProcessor, Tenor,
    VolatilityData,
};

/// Extract underlying symbol from instrument name
/// e.g., "BTC-29MAR24-70000-C" -> "BTC"
fn extract_symbol(instrument_name: &str) -> Option<&str> {
    instrument_name.split('-').next()
}

/// Absolute IV threshold rule
#[derive(Clone)]
pub struct AbsoluteIvRule {
    config: AbsoluteIvRuleConfig,
    id: String,
}

impl AbsoluteIvRule {
    pub fn new(config: AbsoluteIvRuleConfig) -> Self {
        Self {
            id: config.id.clone(),
            config,
        }
    }

    fn evaluate_volatility(&self, data: &VolatilityData) -> Option<Alert> {
        // Extract symbol from instrument name
        let symbol_name = extract_symbol(&data.symbol)?;

        // Check if this matches our configured symbol
        if symbol_name.to_lowercase() != self.config.symbol.to_lowercase() {
            return None;
        }

        // Get tenor classification - skip if in gap region
        let tenor = data.tenor()?;

        // Get IV threshold for this tenor
        let iv_threshold = match tenor {
            Tenor::Short => self.config.short_threshold,
            Tenor::Medium => self.config.medium_threshold,
            Tenor::Long => self.config.long_threshold,
        };

        // Get ATM threshold: per-DTE config takes precedence, fallback to tenor-based
        let dte_key = data.dte.to_string();
        let atm_threshold = self
            .config
            .dte_atm_thresholds
            .get(&dte_key)
            .copied()
            .unwrap_or(match tenor {
                Tenor::Short => self.config.short_atm_threshold,
                Tenor::Medium => self.config.medium_atm_threshold,
                Tenor::Long => self.config.long_atm_threshold,
            });

        // ATM filter - skip if not within ATM moneyness threshold for this tenor
        let moneyness = data.moneyness();
        if moneyness.abs() > atm_threshold {
            return None;
        }

        // IV threshold check
        if data.iv >= iv_threshold {
            let mark_price = data
                .extra
                .get("mark_price_coin")
                .and_then(serde_json::value::Value::as_f64)
                .unwrap_or(0.0);
            Some(Alert::new(
                AlertType::AbsoluteIv {
                    threshold: iv_threshold,
                },
                tenor,
                data.symbol.clone(),
                data.iv,
                format!(
                    "{} {} IV {:.1}% (symbol: {}, moneyness: {:.2}%) >= threshold {:.1}%",
                    data.symbol,
                    tenor,
                    data.iv * 100.0,
                    symbol_name,
                    moneyness * 100.0,
                    iv_threshold * 100.0
                ),
                data.timestamp,
                data.source.clone(),
                data.index_price,
                data.dte,
                data.option_type,
                moneyness,
                mark_price,
                String::new(), // trace_id - set by engine layer
            ))
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl RuleProcessor for AbsoluteIvRule {
    fn id(&self) -> &str {
        &self.id
    }

    fn rule_type(&self) -> &str {
        "absolute-iv"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Volatility]
    }

    async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert> {
        let MonitoringEvent::Volatility(vol_data) = event else {
            return vec![];
        };
        self.evaluate_volatility(vol_data).into_iter().collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_symbol() {
        assert_eq!(extract_symbol("BTC-29MAR24-70000-C"), Some("BTC"));
        assert_eq!(extract_symbol("ETH-29MAR24-3500-P"), Some("ETH"));
        assert_eq!(extract_symbol("INVALID"), Some("INVALID"));
    }
}
