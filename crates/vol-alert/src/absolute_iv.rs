//! Absolute IV threshold alert handler.

use vol_core::{AlertHandler, Alert, AlertType, AlertAction, VolatilityData, Tenor, Result};
use vol_config::{AbsoluteIvConfig, SymbolIvConfig};

/// Alert handler for absolute IV threshold breaches
pub struct AbsoluteIvHandler {
    config: AbsoluteIvConfig,
}

/// Extract underlying symbol from instrument name
/// e.g., "BTC-29MAR24-70000-C" -> "BTC"
fn extract_symbol(instrument_name: &str) -> Option<&str> {
    instrument_name.split('-').next()
}

impl AbsoluteIvHandler {
    pub fn new(config: AbsoluteIvConfig) -> Self {
        Self { config }
    }

    fn get_symbol_config(&self, symbol: &str) -> Option<&SymbolIvConfig> {
        self.config.get_symbol_config(symbol)
    }
}

#[async_trait::async_trait]
impl AlertHandler for AbsoluteIvHandler {
    fn name(&self) -> &str {
        "absolute_iv"
    }

    fn evaluate(&self, data: &VolatilityData) -> Option<Alert> {
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
            ))
        } else {
            None
        }
    }

    #[allow(clippy::async_yields_async)]
    async fn on_alert(&self, _alert: &Alert) -> Result<AlertAction> {
        Ok(AlertAction::Send)
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

        let handler = AbsoluteIvHandler::new(AbsoluteIvConfig { symbols });

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
        let alert = handler.evaluate(&btc_data);
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
        let alert = handler.evaluate(&eth_data);
        assert!(alert.is_none());
    }

    #[test]
    fn test_evaluate_missing_symbol_config() {
        let mut symbols = HashMap::new();
        symbols.insert("btc".to_string(), SymbolIvConfig {
            short_threshold: 0.80,
            medium_threshold: 0.70,
            long_threshold: 0.60,
            short_atm_threshold: 0.05,
            medium_atm_threshold: 0.10,
            long_atm_threshold: 0.15,
        });

        let handler = AbsoluteIvHandler::new(AbsoluteIvConfig { symbols });

        // SOL data - no config for SOL
        let sol_data = VolatilityData {
            symbol: "SOL-6JAN25-100-C".to_string(),
            dte: 5,
            iv: 0.95,
            timestamp: 1234567890,
            source: "deribit".to_string(),
            strike: 100.0,
            option_type: vol_core::OptionType::Call,
            index_price: 100.0,
            delta: None,
            extra: std::collections::HashMap::new(),
        };

        // Should return None for missing symbol config
        let alert = handler.evaluate(&sol_data);
        assert!(alert.is_none());
    }
}
