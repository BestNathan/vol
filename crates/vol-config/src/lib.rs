//! vol-config: Configuration management for the volatility monitoring system.

use serde::{Deserialize, Serialize};
use vol_core::Tenor;

pub mod datasource;
pub mod metrics;
pub mod notification;
pub mod rule;

pub use datasource::*;
pub use metrics::*;
pub use notification::*;
pub use rule::*;

/// Engine configuration - layered arrays for extensibility
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EngineConfig {
    #[serde(default)]
    pub datasources: Vec<DataSourceConfig>,
    #[serde(default)]
    pub rules: Vec<RuleConfig>,
    #[serde(default)]
    pub notifications: Vec<NotificationConfig>,
}

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
    #[serde(default)]
    pub auth: Option<DeribitAuthConfig>,
}

/// Deribit OAuth authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitAuthConfig {
    /// OAuth client ID - env var DERIBIT_CLIENT_ID takes precedence
    #[serde(default)]
    pub client_id: Option<String>,
    /// OAuth client secret - env var DERIBIT_CLIENT_SECRET takes precedence
    #[serde(default)]
    pub client_secret: Option<String>,
}

impl DeribitAuthConfig {
    /// Get client_id with env var override
    pub fn client_id(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_ID")
            .ok()
            .or_else(|| self.client_id.clone())
    }

    /// Get client_secret with env var override
    pub fn client_secret(&self) -> Option<String> {
        std::env::var("DERIBIT_CLIENT_SECRET")
            .ok()
            .or_else(|| self.client_secret.clone())
    }
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
    #[serde(default)]
    pub metrics: Vec<MetricConfig>,
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

    #[test]
    fn test_deribit_auth_config_parsing() {
        // Test DeribitAuthConfig parsing from TOML
        let toml_str = r#"
            client_id = "test_client_id"
            client_secret = "test_client_secret"
        "#;

        let config: DeribitAuthConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.client_id, Some("test_client_id".to_string()));
        assert_eq!(config.client_secret, Some("test_client_secret".to_string()));
    }

    #[test]
    fn test_deribit_auth_config_with_env_override() {
        // Set env vars for testing
        std::env::set_var("DERIBIT_CLIENT_ID", "env_client_id");
        std::env::set_var("DERIBIT_CLIENT_SECRET", "env_client_secret");

        let config = DeribitAuthConfig {
            client_id: Some("file_client_id".to_string()),
            client_secret: Some("file_client_secret".to_string()),
        };

        // Env vars should take precedence
        assert_eq!(config.client_id(), Some("env_client_id".to_string()));
        assert_eq!(config.client_secret(), Some("env_client_secret".to_string()));

        // Clean up
        std::env::remove_var("DERIBIT_CLIENT_ID");
        std::env::remove_var("DERIBIT_CLIENT_SECRET");
    }

    #[test]
    fn test_deribit_auth_config_fallback_to_file() {
        // Ensure env vars are not set
        std::env::remove_var("DERIBIT_CLIENT_ID");
        std::env::remove_var("DERIBIT_CLIENT_SECRET");

        let config = DeribitAuthConfig {
            client_id: Some("file_client_id".to_string()),
            client_secret: Some("file_client_secret".to_string()),
        };

        // Should fallback to file values
        assert_eq!(config.client_id(), Some("file_client_id".to_string()));
        assert_eq!(config.client_secret(), Some("file_client_secret".to_string()));
    }

    #[test]
    fn test_metric_config_free_balance_parsing() {
        // Test FreeBalance variant parsing
        let toml_str = r#"
            type = "free_balance"
            enabled = true
            min_threshold = 1.0
            max_threshold = 10.0
        "#;

        let config: MetricConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled());
        assert_eq!(config.name(), "free_balance");
    }

    #[test]
    fn test_metric_config_margin_ratio_parsing() {
        // Test MarginRatio variant parsing
        let toml_str = r#"
            type = "margin_ratio"
            enabled = true
            min_threshold = 0.5
        "#;

        let config: MetricConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled());
        assert_eq!(config.name(), "margin_ratio");
    }

    #[test]
    fn test_metric_config_delta_exposure_parsing() {
        // Test DeltaExposure variant parsing
        let toml_str = r#"
            type = "delta_exposure"
            enabled = true
            min_threshold = -100.0
            max_threshold = 100.0
        "#;

        let config: MetricConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled());
        assert_eq!(config.name(), "delta_exposure");
    }

    #[test]
    fn test_metric_config_session_pnl_parsing() {
        // Test SessionPnl variant parsing
        let toml_str = r#"
            type = "session_pnl"
            enabled = false
            min_threshold = -1000.0
        "#;

        let config: MetricConfig = toml::from_str(toml_str).unwrap();
        assert!(!config.enabled());
        assert_eq!(config.name(), "session_pnl");
    }

    #[test]
    fn test_metric_config_total_greeks_parsing() {
        // Test TotalGreeks variant parsing
        let toml_str = r#"
            type = "total_greeks"
            enabled = true
            gamma_threshold = 0.5
            vega_threshold = 10.0
            theta_threshold = 100.0
            delta_threshold = 50.0
        "#;

        let config: MetricConfig = toml::from_str(toml_str).unwrap();
        assert!(config.enabled());
        assert_eq!(config.name(), "total_greeks");
    }

    #[test]
    fn test_metric_config_vec_parsing() {
        // Test Vec<MetricConfig> parsing from TOML array
        let toml_str = r#"
            [[metrics]]
            type = "free_balance"
            enabled = true
            min_threshold = 1.0

            [[metrics]]
            type = "margin_ratio"
            enabled = true
            min_threshold = 0.5

            [[metrics]]
            type = "total_greeks"
            enabled = true
            gamma_threshold = 0.5
            vega_threshold = 10.0
        "#;

        #[derive(Debug, Deserialize)]
        struct TestConfig {
            metrics: Vec<MetricConfig>,
        }

        let config: TestConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.metrics.len(), 3);
        assert_eq!(config.metrics[0].name(), "free_balance");
        assert_eq!(config.metrics[1].name(), "margin_ratio");
        assert_eq!(config.metrics[2].name(), "total_greeks");
    }
}
