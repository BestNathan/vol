//! Loki configuration.
//!
//! Reads Loki URL from `LOKI_URL` environment variable or TOML config fallback.

use std::env;

/// Loki client configuration.
#[derive(Debug, Clone)]
pub struct LokiConfig {
    /// Loki base URL (e.g., `http://loki.observability.svc.cluster.local:3100`).
    pub url: String,
    /// Number of entries to buffer before flushing.
    pub batch_size: usize,
    /// Maximum milliseconds between flushes.
    pub flush_interval_ms: u64,
}

impl LokiConfig {
    /// Create config from environment variable `LOKI_URL`.
    ///
    /// Returns `None` if the environment variable is not set.
    pub fn from_env() -> Option<Self> {
        env::var("LOKI_URL").ok().map(|url| Self {
            url,
            batch_size: 50,
            flush_interval_ms: 1000,
        })
    }

    /// Create config with a specific URL.
    pub fn with_url(url: String) -> Self {
        Self {
            url,
            batch_size: 50,
            flush_interval_ms: 1000,
        }
    }

    /// Override the batch size.
    pub fn batch_size(mut self, size: usize) -> Self {
        self.batch_size = size;
        self
    }

    /// Override the flush interval in milliseconds.
    pub fn flush_interval_ms(mut self, ms: u64) -> Self {
        self.flush_interval_ms = ms;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_from_url() {
        let config = LokiConfig::with_url("http://loki:3100".to_string());
        assert_eq!(config.url, "http://loki:3100");
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.flush_interval_ms, 1000);
    }

    #[test]
    fn test_config_builder() {
        let config = LokiConfig::with_url("http://loki:3100".to_string())
            .batch_size(100)
            .flush_interval_ms(2000);
        assert_eq!(config.batch_size, 100);
        assert_eq!(config.flush_interval_ms, 2000);
    }

    #[test]
    fn test_config_from_env_missing() {
        // Ensure LOKI_URL is not set
        let _ = env::remove_var("LOKI_URL");
        assert!(LokiConfig::from_env().is_none());
    }

    #[test]
    fn test_config_from_env_present() {
        env::set_var("LOKI_URL", "http://test-loki:3100");
        let config = LokiConfig::from_env().unwrap();
        assert_eq!(config.url, "http://test-loki:3100");
        env::remove_var("LOKI_URL");
    }
}
