//! Service configuration.

use serde::Deserialize;

/// Top-level observability service configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default = "default_listen_addr")]
    pub listen_addr: String,

    #[serde(default)]
    pub loki: LokiConfig,

    #[serde(default)]
    pub tdengine: TdengineWriterConfig,
}

fn default_listen_addr() -> String {
    "0.0.0.0:3030".to_string()
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            listen_addr: default_listen_addr(),
            loki: LokiConfig::default(),
            tdengine: TdengineWriterConfig::default(),
        }
    }
}

/// Loki writer configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct LokiConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_loki_url")]
    pub url: String,

    #[serde(default = "default_loki_batch_size")]
    pub batch_size: usize,

    #[serde(default = "default_loki_flush_ms")]
    pub flush_interval_ms: u64,
}

fn default_true() -> bool {
    true
}
fn default_loki_url() -> String {
    "http://localhost:3100".to_string()
}
fn default_loki_batch_size() -> usize {
    50
}
fn default_loki_flush_ms() -> u64 {
    200
}

impl Default for LokiConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            url: default_loki_url(),
            batch_size: default_loki_batch_size(),
            flush_interval_ms: default_loki_flush_ms(),
        }
    }
}

/// TDengine writer configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct TdengineWriterConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default = "default_tdengine_dsn")]
    pub dsn: String,

    #[serde(default = "default_tdengine_database")]
    pub database: String,

    #[serde(default = "default_tdengine_batch_size")]
    pub batch_size: usize,

    #[serde(default = "default_tdengine_flush_ms")]
    pub flush_interval_ms: u64,
}

fn default_tdengine_dsn() -> String {
    "taos://localhost:6030".to_string()
}
fn default_tdengine_database() -> String {
    "vol_observability".to_string()
}
fn default_tdengine_batch_size() -> usize {
    100
}
fn default_tdengine_flush_ms() -> u64 {
    500
}

impl Default for TdengineWriterConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dsn: default_tdengine_dsn(),
            database: default_tdengine_database(),
            batch_size: default_tdengine_batch_size(),
            flush_interval_ms: default_tdengine_flush_ms(),
        }
    }
}
