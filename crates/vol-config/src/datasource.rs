//! Data source configuration types.
//!
//! Data-specific configuration only. Transport-layer configuration
//! is defined globally in [clients] section.

use serde::{Deserialize, Serialize};

/// Volatility data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolatilityConfig {
    pub id: String,
    #[serde(default = "default_symbols")]
    pub symbols: Vec<String>,
}

/// Portfolio data source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioConfig {
    pub id: String,
    #[serde(default = "default_currencies")]
    pub currencies: Vec<String>,
    #[serde(default = "default_30")]
    pub poll_interval_secs: u64,
}

/// Data source configuration enum - organized by data type
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DataSourceConfig {
    Volatility(VolatilityConfig),
    Portfolio(PortfolioConfig),
}

impl DataSourceConfig {
    pub fn id(&self) -> &str {
        match self {
            DataSourceConfig::Volatility(c) => &c.id,
            DataSourceConfig::Portfolio(c) => &c.id,
        }
    }
}

fn default_30() -> u64 {
    30
}

fn default_symbols() -> Vec<String> {
    vec!["BTC".to_string(), "ETH".to_string()]
}

fn default_currencies() -> Vec<String> {
    vec!["BTC".to_string(), "ETH".to_string()]
}
