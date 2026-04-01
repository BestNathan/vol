//! Data source configuration types.
//!
//! Configuration is organized by provider type (Deribit, Binance, etc.)
//! rather than transport mechanism (WebSocket vs HTTP).

use serde::{Deserialize, Serialize};

/// Deribit authentication configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DeribitAuthConfig {
    #[serde(default)]
    pub client_id: Option<String>,
    #[serde(default)]
    pub client_secret: Option<String>,
}

/// Deribit data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitDataSourceConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub ws_url: String,
    #[serde(default = "default_symbols")]
    pub symbols: Vec<String>,
    #[serde(default = "default_60")]
    pub poll_interval_secs: u64,
    #[serde(default)]
    pub auth: Option<DeribitAuthConfig>,
}

/// Internal/portfolio data source configuration (HTTP polling)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InternalDataSourceConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub url: String,
    #[serde(default = "default_30")]
    pub poll_interval_secs: u64,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

/// Binance data source configuration (for future expansion)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinanceDataSourceConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub api_url: String,
    #[serde(default = "default_symbols")]
    pub symbols: Vec<String>,
    #[serde(default = "default_60")]
    pub poll_interval_secs: u64,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub api_secret: Option<String>,
}

/// Data source configuration enum - organized by provider
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "provider", rename_all = "lowercase")]
pub enum DataSourceConfig {
    Deribit(DeribitDataSourceConfig),
    Binance(BinanceDataSourceConfig),
    Internal(InternalDataSourceConfig),
}

impl DataSourceConfig {
    pub fn id(&self) -> &str {
        match self {
            DataSourceConfig::Deribit(c) => &c.id,
            DataSourceConfig::Binance(c) => &c.id,
            DataSourceConfig::Internal(c) => &c.id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            DataSourceConfig::Deribit(c) => c.enabled,
            DataSourceConfig::Binance(c) => c.enabled,
            DataSourceConfig::Internal(c) => c.enabled,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_60() -> u64 {
    60
}

fn default_30() -> u64 {
    30
}

fn default_symbols() -> Vec<String> {
    vec!["BTC".to_string(), "ETH".to_string()]
}
