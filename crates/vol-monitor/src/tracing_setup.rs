//! Tracing and logging initialization.
//!
//! Sets up:
//! - Console layer (compact format, colored)
//! - File layer (JSON format, rolling hourly, 7-day retention)
//! - Error file layer (ERROR level only)
//! - OpenTelemetry layer (OTLP gRPC to Jaeger)
//!
//! Log rotation: Files are rotated hourly to prevent excessive file sizes.
//! Retention: Log files older than `retention_days` are automatically cleaned up.

use std::sync::OnceLock;

use opentelemetry::{global, KeyValue, trace::TracerProvider as _};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::{trace::{self, Sampler}, Resource};
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{
    fmt,
    fmt::format::FmtSpan,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
    Layer,
    Registry,
};
use tracing::subscriber::set_global_default;

use vol_config::{LoggingConfig, TracingConfig};

static TRACING_INITIALIZED: OnceLock<()> = OnceLock::new();
static OTEL_TRACER_PROVIDER: OnceLock<trace::TracerProvider> = OnceLock::new();

/// Initialize tracing and logging.
pub fn init(config: &TracingConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Check if already initialized
    if TRACING_INITIALIZED.get().is_some() {
        return Ok(());
    }

    // Create log directory
    std::fs::create_dir_all(&config.logging.log_dir)?;

    // Build EnvFilter for dynamic log levels
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&config.logging.console_level));

    // 1. Console layer (compact, colored) - writes to stdout
    let console_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stdout);

    // 2. File layer (JSON, rolling)
    let file_appender = create_file_appender(&config.logging);
    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_thread_names(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .with_current_span(true)
        .with_writer(file_appender);

    // 3. OpenTelemetry layer (OTLP gRPC to Jaeger)
    let endpoint = std::env::var("OTEL_ENDPOINT")
        .unwrap_or_else(|_| config.opentelemetry.endpoint.clone());

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| config.opentelemetry.service_name.clone());

    let sample_rate: f64 = std::env::var("OTEL_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(config.opentelemetry.sample_rate);

    if config.opentelemetry.enabled && sample_rate > 0.0 {
        let resource = Resource::new(vec![
            KeyValue::new("service.name", service_name.clone()),
            KeyValue::new("service.namespace", config.opentelemetry.service_namespace.clone()),
            KeyValue::new("deployment.environment", config.opentelemetry.deployment_environment.clone()),
        ]);

        let exporter = opentelemetry_otlp::new_exporter()
            .tonic()
            .with_endpoint(&endpoint)
            .with_timeout(std::time::Duration::from_millis(config.opentelemetry.batch.max_export_timeout_millis))
            .build_span_exporter()?;

        let tracer_provider = trace::TracerProvider::builder()
            .with_config(trace::Config::default()
                .with_sampler(Sampler::AlwaysOn)
                .with_resource(resource)
            )
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .build();

        let tracer = tracer_provider.tracer(service_name.clone());
        let otel_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_location(true)
            .with_threads(true);

        OTEL_TRACER_PROVIDER.set(tracer_provider.clone()).ok();
        global::set_tracer_provider(tracer_provider);

        tracing::info!(
            "OpenTelemetry tracing enabled: endpoint={} service={} sample_rate={}",
            endpoint,
            service_name,
            sample_rate
        );

        // Build subscriber with all layers using Registry as base
        let subscriber = Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .with(otel_layer);

        // Add error layer if enabled
        if config.logging.error_file {
            let error_appender = create_error_appender(&config.logging);
            let error_layer = fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
                .json()
                .with_current_span(true)
                .with_writer(error_appender)
                .with_filter(tracing_subscriber::filter::LevelFilter::ERROR);

            subscriber.with(error_layer).init();
        } else {
            subscriber.init();
        }
    } else {
        // OpenTelemetry disabled - use simple init like original
        let error_appender = create_error_appender(&config.logging);

        if config.logging.error_file {
            let error_filter = tracing_subscriber::filter::LevelFilter::ERROR;
            let error_layer = fmt::layer()
                .with_ansi(false)
                .with_target(true)
                .with_thread_ids(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
                .json()
                .with_writer(error_appender)
                .with_filter(error_filter);

            Registry::default()
                .with(env_filter)
                .with(console_layer)
                .with(file_layer)
                .with(error_layer)
                .init();
        } else {
            Registry::default()
                .with(env_filter)
                .with(console_layer)
                .with(file_layer)
                .init();
        }

        tracing::info!("OpenTelemetry tracing disabled");
    }

    // Mark as initialized
    TRACING_INITIALIZED.get_or_init(|| ());

    tracing::info!("Tracing initialized: logging={} opentelemetry={}",
        config.logging.log_dir,
        config.opentelemetry.enabled);

    Ok(())
}

fn create_file_appender(config: &LoggingConfig) -> RollingFileAppender {
    RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix(config.log_prefix.clone())
        .filename_suffix("log")
        .max_log_files((config.retention_days * 24).try_into().unwrap())  // Keep retention_days worth of hourly logs
        .build(&config.log_dir)
        .expect("Failed to create file appender")
}

fn create_error_appender(config: &LoggingConfig) -> RollingFileAppender {
    RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix(config.log_prefix.clone())
        .filename_suffix("error.log")
        .max_log_files((config.retention_days * 24).try_into().unwrap())  // Keep retention_days worth of hourly logs
        .build(&config.log_dir)
        .expect("Failed to create error appender")
}

pub fn shutdown() {
    tracing::info!("Shutting down OpenTelemetry");
    global::shutdown_tracer_provider();
}
