//! vol-config: Configuration management for the volatility monitoring system.

use serde::{Deserialize, Serialize};
use vol_core::Tenor;

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub data_sources: DataSourcesConfig,
    pub tenors: TenorConfig,
    pub alerts: AlertsConfig,
    pub notifications: NotificationsConfig,
    pub state: StateConfig,
}

/// Data sources configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataSourcesConfig {
    pub enabled: Vec<String>,
    pub deribit: Option<DeribitConfig>,
}

/// Deribit-specific configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitConfig {
    pub ws_url: String,
    pub symbols: Vec<String>,
    pub poll_interval_secs: u64,
}

/// Tenor configuration - DTE boundaries
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenorConfig {
    pub short_max_dte: u32,
    pub medium_min_dte: u32,
    pub medium_max_dte: u32,
    pub long_min_dte: u32,
}

impl TenorConfig {
    /// Classify DTE into tenor based on config
    pub fn classify(&self, dte: u32) -> Tenor {
        if dte <= self.short_max_dte {
            Tenor::Short
        } else if dte > self.medium_min_dte && dte < self.medium_max_dte {
            Tenor::Medium
        } else {
            Tenor::Long
        }
    }
}

/// Alerts configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlertsConfig {
    pub enabled: Vec<String>,
    pub cooldown_secs: u64,
    pub absolute_iv: AbsoluteIvConfig,
    pub rate_of_change: RateOfChangeConfig,
    pub term_structure: TermStructureConfig,
    pub skew: SkewConfig,
}

/// Per-symbol IV and ATM threshold configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolIvConfig {
    pub short_threshold: f64,
    pub medium_threshold: f64,
    pub long_threshold: f64,
    pub short_atm_threshold: f64,
    pub medium_atm_threshold: f64,
    pub long_atm_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbsoluteIvConfig {
    /// Per-symbol configuration keyed by lowercase symbol name (e.g., "btc", "eth")
    pub symbols: std::collections::HashMap<String, SymbolIvConfig>,
}

impl AbsoluteIvConfig {
    /// Get symbol-specific config (case-insensitive)
    pub fn get_symbol_config(&self, symbol: &str) -> Option<&SymbolIvConfig> {
        self.symbols.get(&symbol.to_lowercase())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateOfChangeConfig {
    pub window_1h_threshold: f64,
    pub window_4h_threshold: f64,
    pub window_24h_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TermStructureConfig {
    pub short_long_spread_threshold: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkewConfig {
    pub threshold: f64,
}

/// Notifications configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationsConfig {
    pub enabled: Vec<String>,
    pub feishu: Option<FeishuConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuConfig {
    /// App ID for OAuth 2.0 authentication
    #[serde(default)]
    pub app_id: Option<String>,
    /// App Secret for OAuth 2.0 authentication
    #[serde(default)]
    pub app_secret: Option<String>,
    /// Receive ID (chat_id or user_id)
    #[serde(default)]
    pub receive_id: Option<String>,
    /// Message template for text notifications
    #[serde(default = "default_message_template")]
    pub message_template: String,
}

fn default_message_template() -> String {
    "🚨 {tenor} {alert_type}: {symbol} | IV={value:.1}% | 指数={index_price} | DTE={dte}天 | {option_type} | 价格={mark_price_coin} ({mark_price_usd} USD)".to_string()
}

/// State persistence configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateConfig {
    pub path: String,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn load(path: &str) -> Result<Self, vol_core::VolError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| vol_core::VolError::Config(format!("Failed to read config file: {}", e)))?;

        toml::from_str(&content)
            .map_err(|e| vol_core::VolError::Config(format!("Failed to parse config: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_symbol_iv_config_parsing() {
        // Test direct SymbolIvConfig parsing
        let toml_str = r#"
            short_threshold = 0.80
            medium_threshold = 0.70
            long_threshold = 0.60
            short_atm_threshold = 0.05
            medium_atm_threshold = 0.10
            long_atm_threshold = 0.15
        "#;

        let config: SymbolIvConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.short_threshold, 0.80);
        assert_eq!(config.short_atm_threshold, 0.05);
    }

    #[test]
    fn test_case_insensitive_symbol_lookup() {
        let mut symbols = std::collections::HashMap::new();
        symbols.insert("btc".to_string(), SymbolIvConfig {
            short_threshold: 0.80,
            medium_threshold: 0.70,
            long_threshold: 0.60,
            short_atm_threshold: 0.05,
            medium_atm_threshold: 0.10,
            long_atm_threshold: 0.15,
        });

        let config = AbsoluteIvConfig { symbols };

        assert!(config.get_symbol_config("BTC").is_some());
        assert!(config.get_symbol_config("btc").is_some());
        assert!(config.get_symbol_config("Btc").is_some());
    }
}
