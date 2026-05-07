//! OTel log initialization helper.
//!
//! Provides a single `init_otel_logs()` function that sets up the full
//! tracing-subscriber stack with OTel log export.

use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::Resource;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter,
    Registry,
};

/// Initialize the tracing-subscriber with OTel log export.
///
/// This must be called exactly once, before any tracing macros are used.
/// Replaces the simple `tracing_subscriber::fmt().init()` call.
///
/// # Arguments
///
/// * `endpoint` — OTel Collector gRPC endpoint (e.g., `http://localhost:4317`).
///   Falls back to `OTEL_EXPORTER_OTLP_ENDPOINT` env var, then `http://localhost:4317`.
/// * `service_name` — Service name for OTel resource attributes.
///
/// # Example
///
/// ```rust,no_run
/// vol_llm_observability::init_otel_logs(
///     "http://localhost:4317",
///     "my-agent",
/// ).expect("Failed to initialize OTel logs");
/// ```
pub fn init_otel_logs(
    endpoint: &str,
    service_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let resolved_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| endpoint.to_string());

    let resource = Resource::builder()
        .with_service_name(service_name.to_string())
        .with_attributes([
            KeyValue::new("deployment.environment", "development"),
        ])
        .build();

    let exporter = opentelemetry_otlp::LogExporter::builder()
        .with_tonic()
        .with_endpoint(&resolved_endpoint)
        .build()?;

    let logger_provider = SdkLoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter)
        .build();

    let otel_log_layer = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider);

    // Console layer
    let console_layer = fmt::layer()
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stdout);

    // File layer (JSON, rolling)
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix("agent")
        .filename_suffix("log")
        .max_log_files(168) // 7 days of hourly files
        .build(".")
        .unwrap_or_else(|_| {
            // Fallback to a temp file if current dir is not writable
            RollingFileAppender::builder()
                .rotation(Rotation::HOURLY)
                .filename_prefix("agent")
                .filename_suffix("log")
                .build("/tmp")
                .expect("Failed to create file appender")
        });

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .with_writer(file_appender);

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    Registry::default()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .with(otel_log_layer)
        .init();

    tracing::info!(
        "OTel logs initialized: endpoint={} service={}",
        resolved_endpoint,
        service_name
    );

    Ok(())
}
