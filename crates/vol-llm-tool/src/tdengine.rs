//! TDengine client for querying historical data.

use reqwest::Client;
use serde::Deserialize;
use serde_json::Value;

/// TDengine client configuration
#[derive(Debug, Clone)]
pub struct TdengineConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

impl Default for TdengineConfig {
    fn default() -> Self {
        Self {
            host: "192.168.2.106".to_string(),
            port: 6041,
            user: "root".to_string(),
            password: "taosdata".to_string(),
            database: "deribit".to_string(),
        }
    }
}

/// TDengine client
#[derive(Clone)]
pub struct TdengineClient {
    client: Client,
    base_url: String,
    config: TdengineConfig,
}

/// TDengine response format (TDengine 3.x REST API)
#[derive(Debug, Deserialize)]
pub struct TdengineResponse {
    pub code: i32,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub data: Option<Value>,
    #[serde(default)]
    pub column_meta: Option<Value>,
    #[serde(default)]
    pub rows: Option<u32>,
}

impl TdengineClient {
    /// Create new TDengine client
    pub fn new(config: TdengineConfig) -> Self {
        let base_url = format!("http://{}:{}/rest", config.host, config.port);
        Self {
            client: Client::new(),
            base_url,
            config,
        }
    }

    /// Execute SQL query
    pub async fn query(&self, sql: &str) -> Result<TdengineResponse, reqwest::Error> {
        // TDengine REST API: POST http://host:6041/rest/sql/database
        self.query_with_db(sql).await
    }

    /// Execute SQL query with database
    pub async fn query_with_db(&self, sql: &str) -> Result<TdengineResponse, reqwest::Error> {
        // TDengine REST API: POST http://host:6041/rest/sql/database
        let url = format!("{}/sql/{}", self.base_url, self.config.database);

        self.client
            .post(&url)
            .basic_auth(&self.config.user, Some(&self.config.password))
            .body(sql.to_string())
            .send()
            .await?
            .json()
            .await
    }

    /// Query alert history - maps to deribit_volatility_index
    pub async fn query_alert_history(
        &self,
        symbol: &str,
        limit: u32,
        hours: Option<u32>,
    ) -> Result<TdengineResponse, reqwest::Error> {
        let time_filter = match hours {
            Some(h) => format!("AND _ts >= NOW - {}h", h),
            None => String::new(),
        };

        // Convert symbol to lowercase full format (e.g., "BTC" -> "btc_usd", "btc_usd" -> "btc_usd")
        let index_name = if symbol.contains('_') {
            symbol.to_lowercase()
        } else {
            format!("{}_usd", symbol.to_lowercase())
        };

        let sql = format!(
            "SELECT _ts, volatility, index_name \
             FROM deribit_volatility_index \
             WHERE index_name = '{}' {} \
             ORDER BY _ts DESC \
             LIMIT {}",
            index_name, time_filter, limit
        );

        self.query_with_db(&sql).await
    }

    /// Query IV curve data - maps to deribit_options
    pub async fn query_iv_curve(
        &self,
        instrument: &str,
        _deltas: Option<&[f64]>,
    ) -> Result<TdengineResponse, reqwest::Error> {
        // Parse instrument name to extract expiry and strike
        // Format: BTC-29DEC23-3000-C or similar
        let sql = format!(
            "SELECT _ts, instrument_name, iv, mark_price, expiry_date, strike_price, type \
             FROM deribit_options \
             WHERE instrument_name = '{}' \
             ORDER BY _ts DESC \
             LIMIT {}",
            instrument, 100
        );

        self.query_with_db(&sql).await
    }

    /// Query market data - maps to deribit_index_price
    pub async fn query_market_data(
        &self,
        instrument: &str,
    ) -> Result<TdengineResponse, reqwest::Error> {
        // Convert to uppercase (e.g., "btc_usd" -> "BTC", "btc" -> "BTC")
        let index_name = instrument
            .to_uppercase()
            .split('_')
            .next()
            .unwrap_or(instrument)
            .to_string();

        let sql = format!(
            "SELECT _ts, price, index_name \
             FROM deribit_index_price \
             WHERE index_name = '{}' \
             ORDER BY _ts DESC \
             LIMIT 1",
            index_name
        );

        self.query_with_db(&sql).await
    }

    /// Query rule info - maps to deribit_rv (realized volatility)
    pub async fn query_rules(&self, rule_name: Option<&str>) -> Result<TdengineResponse, reqwest::Error> {
        let sql = match rule_name {
            Some(name) => {
                // Convert to uppercase and extract base (e.g., "btc_usd" -> "BTC")
                let index_name = name.to_uppercase().split('_').next().unwrap_or(name).to_string();
                format!(
                    "SELECT _ts, rv, index_name \
                     FROM deribit_rv \
                     WHERE index_name = '{}'",
                    index_name
                )
            },
            None => "SELECT _ts, rv, index_name FROM deribit_rv ORDER BY _ts DESC LIMIT 100".to_string(),
        };

        self.query_with_db(&sql).await
    }

    /// Show databases
    pub async fn show_databases(&self) -> Result<TdengineResponse, reqwest::Error> {
        self.query("SHOW DATABASES").await
    }

    /// Show tables in current database
    pub async fn show_tables(&self) -> Result<TdengineResponse, reqwest::Error> {
        self.query("SHOW TABLES").await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_default() {
        let config = TdengineConfig::default();
        assert_eq!(config.host, "192.168.2.106");
        assert_eq!(config.port, 6041);
    }

    #[test]
    fn test_client_creation() {
        let config = TdengineConfig::default();
        let client = TdengineClient::new(config);
        assert!(client.base_url.contains("192.168.2.106"));
    }
}
