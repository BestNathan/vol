//! Absolute IV threshold rule.

use vol_core::{RuleProcessor, MonitoringEvent, Alert, AlertType, EventType, RuleAction, Result, Tenor, VolatilityData};
use vol_config::{AbsoluteIvConfig, SymbolIvConfig};

/// Extract underlying symbol from instrument name
/// e.g., "BTC-29MAR24-70000-C" -> "BTC"
fn extract_symbol(instrument_name: &str) -> Option<&str> {
    instrument_name.split('-').next()
}

/// Absolute IV threshold rule
#[derive(Clone)]
pub struct AbsoluteIvRule {
    config: AbsoluteIvConfig,
}

impl AbsoluteIvRule {
    pub fn new(config: AbsoluteIvConfig) -> Self {
        Self { config }
    }

    fn get_symbol_config(&self, symbol: &str) -> Option<&SymbolIvConfig> {
        self.config.get_symbol_config(symbol)
    }

    fn evaluate_volatility(&self, data: &VolatilityData) -> Option<Alert> {
        // Extract symbol from instrument name
        let symbol_name = extract_symbol(&data.symbol)?;

        // Get symbol-specific config
        let symbol_config = self.get_symbol_config(symbol_name)?;

        let tenor = data.tenor();

        // Get IV threshold for this symbol and tenor
        let iv_threshold = match tenor {
            Tenor::Short => symbol_config.short_threshold,
            Tenor::Medium => symbol_config.medium_threshold,
            Tenor::Long => symbol_config.long_threshold,
        };

        // Get ATM threshold for this symbol and tenor
        let atm_threshold = match tenor {
            Tenor::Short => symbol_config.short_atm_threshold,
            Tenor::Medium => symbol_config.medium_atm_threshold,
            Tenor::Long => symbol_config.long_atm_threshold,
        };

        // ATM filter - skip if not ATM for this symbol's threshold
        if !data.is_atm(atm_threshold) {
            return None;
        }

        // IV threshold check
        if data.iv >= iv_threshold {
            let moneyness = data.moneyness();
            let mark_price = data.extra.get("mark_price_coin")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            Some(Alert::new(
                AlertType::AbsoluteIv { threshold: iv_threshold },
                tenor,
                data.symbol.clone(),
                data.iv,
                format!(
                    "{} {} IV {:.1}% (symbol: {}, moneyness: {:.2}%, ATM: {:.1}%) >= threshold {:.1}%",
                    data.symbol, tenor,
                    data.iv * 100.0, symbol_name, moneyness * 100.0, atm_threshold * 100.0, iv_threshold * 100.0
                ),
                data.timestamp,
                data.source.clone(),
                data.index_price,
                data.dte,
                data.option_type,
                moneyness,
                mark_price,
            ))
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl RuleProcessor for AbsoluteIvRule {
    fn name(&self) -> &str {
        "absolute_iv"
    }

    fn interests(&self) -> Vec<EventType> {
        vec![EventType::Volatility]
    }

    fn evaluate(&self, event: &MonitoringEvent) -> Option<Alert> {
        let MonitoringEvent::Volatility(vol_data) = event else {
            return None;
        };
        self.evaluate_volatility(vol_data)
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
    use std::collections::HashMap;

    #[test]
    fn test_extract_symbol() {
        assert_eq!(extract_symbol("BTC-29MAR24-70000-C"), Some("BTC"));
        assert_eq!(extract_symbol("ETH-29MAR24-3500-P"), Some("ETH"));
        assert_eq!(extract_symbol("INVALID"), Some("INVALID"));
    }

    #[test]
    fn test_evaluate_with_symbol_specific_config() {
        let mut symbols = HashMap::new();

        // BTC config - lower thresholds
        symbols.insert("btc".to_string(), SymbolIvConfig {
            short_threshold: 0.80,
            medium_threshold: 0.70,
            long_threshold: 0.60,
            short_atm_threshold: 0.05,
            medium_atm_threshold: 0.10,
            long_atm_threshold: 0.15,
        });

        // ETH config - higher thresholds
        symbols.insert("eth".to_string(), SymbolIvConfig {
            short_threshold: 0.90,
            medium_threshold: 0.80,
            long_threshold: 0.70,
            short_atm_threshold: 0.08,
            medium_atm_threshold: 0.12,
            long_atm_threshold: 0.18,
        });

        let handler = AbsoluteIvRule::new(AbsoluteIvConfig { symbols });

        // Create test data - BTC at 85% IV (should trigger for BTC)
        let btc_data = VolatilityData {
            symbol: "BTC-6JAN25-95000-C".to_string(),
            dte: 5,
            iv: 0.85,
            timestamp: 1234567890,
            source: "deribit".to_string(),
            strike: 95000.0,
            option_type: vol_core::OptionType::Call,
            index_price: 95000.0,
            delta: None,
            extra: std::collections::HashMap::new(),
        };

        // BTC 85% IV should trigger (threshold 80%)
        let event = MonitoringEvent::Volatility(btc_data);
        let alert = handler.evaluate(&event);
        assert!(alert.is_some());

        // Create ETH data at 85% IV (should NOT trigger for ETH)
        let eth_data = VolatilityData {
            symbol: "ETH-6JAN25-3800-C".to_string(),
            dte: 5,
            iv: 0.85,
            timestamp: 1234567890,
            source: "deribit".to_string(),
            strike: 3800.0,
            option_type: vol_core::OptionType::Call,
            index_price: 3800.0,
            delta: None,
            extra: std::collections::HashMap::new(),
        };

        // ETH 85% IV should NOT trigger (threshold 90%)
        let event = MonitoringEvent::Volatility(eth_data);
        let alert = handler.evaluate(&event);
        assert!(alert.is_none());
    }
}
