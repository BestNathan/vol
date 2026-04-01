//! Data source configuration types.

use serde::{Deserialize, Serialize};

/// Deribit-specific configuration (legacy format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitConfig {
    pub ws_url: String,
    pub symbols: Vec<String>,
    pub poll_interval_secs: u64,
    #[serde(default)]
    pub auth: Option<DeribitAuthConfig>,
}

/// Deribit authentication configuration (legacy format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeribitAuthConfig {
    /// OAuth client ID - env var DERIBIT_CLIENT_ID takes precedence
    #[serde(default)]
    pub client_id: Option<String>,
    /// OAuth client secret - env var DERIBIT_CLIENT_SECRET takes precedence
    #[serde(default)]
    pub client_secret: Option<String>,
}

/// WebSocket data source configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSocketDataSourceConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub provider: String,
    pub ws_url: String,
    pub channels: Vec<String>,
    #[serde(default)]
    pub auth: Option<DeribitAuthConfig>,
    #[serde(default = "default_60")]
    pub poll_interval_secs: u64,
}

/// HTTP polling data source configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpPollDataSourceConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub provider: String,
    pub url: String,
    #[serde(default = "default_30")]
    pub poll_interval_secs: u64,
    #[serde(default)]
    pub headers: std::collections::HashMap<String, String>,
}

/// Data source configuration enum
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum DataSourceConfig {
    WebSocket(WebSocketDataSourceConfig),
    HttpPoll(HttpPollDataSourceConfig),
}

impl DataSourceConfig {
    pub fn id(&self) -> &str {
        match self {
            DataSourceConfig::WebSocket(c) => &c.id,
            DataSourceConfig::HttpPoll(c) => &c.id,
        }
    }

    pub fn enabled(&self) -> bool {
        match self {
            DataSourceConfig::WebSocket(c) => c.enabled,
            DataSourceConfig::HttpPoll(c) => c.enabled,
        }
    }
}

fn default_true() -> bool { true }
fn default_60() -> u64 { 60 }
fn default_30() -> u64 { 30 }
