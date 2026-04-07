//! TDengine configuration.

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
