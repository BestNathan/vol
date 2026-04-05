//! Tracing and observability configuration.
//!
//! Supports:
//! - Console logging (stdout)
//! - File logging with rotation
//! - OpenTelemetry/Jaeger distributed tracing

use serde::{Deserialize, Serialize};

/// Main tracing configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TracingConfig {
    /// Logging configuration
    #[serde(default)]
    pub logging: LoggingConfig,

    /// OpenTelemetry/Jaeger configuration
    #[serde(default)]
    pub opentelemetry: OpenTelemetryConfig,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            logging: LoggingConfig::default(),
            opentelemetry: OpenTelemetryConfig::default(),
        }
    }
}

/// Console and file logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    /// Log directory path
    #[serde(default = "default_log_dir")]
    pub log_dir: String,

    /// Log file prefix
    #[serde(default = "default_log_prefix")]
    pub log_prefix: String,

    /// Number of days to retain log files
    #[serde(default = "default_log_retention_days")]
    pub retention_days: u32,

    /// Enable JSON format for file logs
    #[serde(default = "default_true")]
    pub json_format: bool,

    /// Console log level (trace, debug, info, warn, error)
    #[serde(default = "default_console_level")]
    pub console_level: String,

    /// File log level
    #[serde(default = "default_file_level")]
    pub file_level: String,

    /// Enable separate error log file
    #[serde(default = "default_true")]
    pub error_file: bool,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            log_dir: default_log_dir(),
            log_prefix: default_log_prefix(),
            retention_days: default_log_retention_days(),
            json_format: true,
            console_level: default_console_level(),
            file_level: default_file_level(),
            error_file: true,
        }
    }
}

/// OpenTelemetry/Jaeger configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OpenTelemetryConfig {
    /// Enable OpenTelemetry tracing
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Jaeger/OTLP collector endpoint (gRPC)
    #[serde(default = "default_jaeger_endpoint")]
    pub endpoint: String,

    /// Service name (appears in Jaeger UI)
    #[serde(default = "default_service_name")]
    pub service_name: String,

    /// Service namespace (for multi-service deployments)
    #[serde(default = "default_service_namespace")]
    pub service_namespace: String,

    /// Deployment environment
    #[serde(default = "default_deployment_env")]
    pub deployment_environment: String,

    /// Trace sample rate (0.0-1.0)
    /// 1.0 = sample all traces, 0.1 = sample 10%
    #[serde(default = "default_sample_rate")]
    pub sample_rate: f64,

    /// Batch span processor configuration
    #[serde(default)]
    pub batch: BatchConfig,
}

impl Default for OpenTelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            endpoint: default_jaeger_endpoint(),
            service_name: default_service_name(),
            service_namespace: default_service_namespace(),
            deployment_environment: default_deployment_env(),
            sample_rate: default_sample_rate(),
            batch: BatchConfig::default(),
        }
    }
}

/// Batch span processor configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BatchConfig {
    /// Maximum queue size
    #[serde(default = "default_batch_max_queue_size")]
    pub max_queue_size: usize,

    /// Maximum batch size
    #[serde(default = "default_batch_max_batch_size")]
    pub max_batch_size: usize,

    /// Scheduled delay between exports
    #[serde(default = "default_batch_scheduled_delay_millis")]
    pub scheduled_delay_millis: u64,

    /// Maximum export timeout
    #[serde(default = "default_batch_max_export_timeout_millis")]
    pub max_export_timeout_millis: u64,
}

impl Default for BatchConfig {
    fn default() -> Self {
        Self {
            max_queue_size: default_batch_max_queue_size(),
            max_batch_size: default_batch_max_batch_size(),
            scheduled_delay_millis: default_batch_scheduled_delay_millis(),
            max_export_timeout_millis: default_batch_max_export_timeout_millis(),
        }
    }
}

// Default value functions

fn default_log_dir() -> String {
    "logs".to_string()
}

fn default_log_prefix() -> String {
    "vol-monitor".to_string()
}

fn default_log_retention_days() -> u32 {
    7
}

fn default_true() -> bool {
    true
}

fn default_console_level() -> String {
    "info".to_string()
}

fn default_file_level() -> String {
    "debug".to_string()
}

fn default_jaeger_endpoint() -> String {
    "http://localhost:4317".to_string()
}

fn default_service_name() -> String {
    "vol-monitor".to_string()
}

fn default_service_namespace() -> String {
    "deribit".to_string()
}

fn default_deployment_env() -> String {
    "production".to_string()
}

fn default_sample_rate() -> f64 {
    1.0
}

fn default_batch_max_queue_size() -> usize {
    2048
}

fn default_batch_max_batch_size() -> usize {
    512
}

fn default_batch_scheduled_delay_millis() -> u64 {
    5000
}

fn default_batch_max_export_timeout_millis() -> u64 {
    30000
}

/// Jaeger-specific configuration (optional, for advanced setups)
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct JaegerConfig {
    /// Jaeger UI endpoint (for display purposes)
    #[serde(default = "default_jaeger_ui_endpoint")]
    pub ui_endpoint: String,

    /// Agent host (for UDP agent mode, deprecated in favor of OTLP)
    #[serde(default)]
    pub agent_host: Option<String>,

    /// Agent port (for UDP agent mode)
    #[serde(default)]
    pub agent_port: Option<u16>,
}

fn default_jaeger_ui_endpoint() -> String {
    "http://localhost:16686".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tracing_config_default() {
        let config = TracingConfig::default();
        assert_eq!(config.logging.log_dir, "logs");
        assert_eq!(config.logging.retention_days, 7);
        assert!(config.opentelemetry.enabled);
        assert_eq!(config.opentelemetry.endpoint, "http://localhost:4317");
        assert_eq!(config.opentelemetry.service_name, "vol-monitor");
    }

    #[test]
    fn test_tracing_config_from_toml() {
        let toml_str = r#"
            [logging]
            log_dir = "/var/log/vol-monitor"
            retention_days = 14
            json_format = true

            [opentelemetry]
            enabled = true
            endpoint = "http://jaeger-collector:4317"
            service_name = "vol-monitor-prod"
            sample_rate = 0.5
        "#;

        let config: TracingConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.logging.log_dir, "/var/log/vol-monitor");
        assert_eq!(config.logging.retention_days, 14);
        assert_eq!(config.opentelemetry.endpoint, "http://jaeger-collector:4317");
        assert_eq!(config.opentelemetry.service_name, "vol-monitor-prod");
        assert_eq!(config.opentelemetry.sample_rate, 0.5);
    }

    #[test]
    fn test_batch_config_default() {
        let batch = BatchConfig::default();
        assert_eq!(batch.max_queue_size, 2048);
        assert_eq!(batch.max_batch_size, 512);
        assert_eq!(batch.scheduled_delay_millis, 5000);
    }
}
