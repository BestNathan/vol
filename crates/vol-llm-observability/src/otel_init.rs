//! OTel initialization helper.
//!
//! Provides `init()` which sets up the full OpenTelemetry stack:
//! - Traces (OTLP gRPC span export)
//! - Metrics (OTLP gRPC periodic metric export)
//! - Logs (OTLP gRPC log export via tracing bridge)
//!
//! Also sets up console and rolling-file tracing layers.

use std::sync::OnceLock;
use std::time::Duration;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use opentelemetry_sdk::Resource;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Configuration for OpenTelemetry initialization.
#[derive(Debug, Clone)]
pub struct OtelConfig {
    /// Whether OTel export is enabled (traces + metrics + logs).
    pub enabled: bool,
    /// OTel Collector gRPC endpoint (e.g. `http://localhost:4317`).
    pub endpoint: String,
    /// Service name for OTel resource attributes.
    pub service_name: String,
    /// Service namespace (e.g. `vol-agent`).
    pub service_namespace: String,
    /// Deployment environment (e.g. `production`, `development`).
    pub deployment_environment: String,
    /// Trace sampling rate (0.0 = drop all, 1.0 = keep all).
    pub sample_rate: f64,
    /// Batch export timeout in milliseconds.
    pub batch_max_export_timeout_millis: u64,
}

impl Default for OtelConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            endpoint: "http://localhost:4317".to_string(),
            service_name: "vol-agent".to_string(),
            service_namespace: "vol-agent".to_string(),
            deployment_environment: "development".to_string(),
            sample_rate: 1.0,
            batch_max_export_timeout_millis: 30_000,
        }
    }
}

/// Guards holding OTel provider handles.
///
/// Call `shutdown()` during application teardown to flush remaining spans/metrics/logs.
pub struct OtelGuards {
    pub tracer_provider: Option<SdkTracerProvider>,
    pub logger_provider: Option<SdkLoggerProvider>,
    pub meter_provider: Option<SdkMeterProvider>,
}

impl OtelGuards {
    /// Create empty (disabled) guards.
    fn empty() -> Self {
        Self {
            tracer_provider: None,
            logger_provider: None,
            meter_provider: None,
        }
    }

    /// Shut down all active OTel providers, flushing pending data.
    pub fn shutdown(&self) {
        if let Some(ref tp) = self.tracer_provider {
            if let Err(e) = tp.shutdown() {
                eprintln!("tracer_provider shutdown error: {e}");
            }
        }
        if let Some(ref lp) = self.logger_provider {
            if let Err(e) = lp.shutdown() {
                eprintln!("logger_provider shutdown error: {e}");
            }
        }
        if let Some(ref mp) = self.meter_provider {
            if let Err(e) = mp.shutdown() {
                eprintln!("meter_provider shutdown error: {e}");
            }
        }
    }
}

/// Initialize the full OTel stack (traces + metrics + logs) plus console/file layers.
///
/// This must be called exactly once, before any tracing macros are used.
/// Subsequent calls return empty guards (idempotent via `OnceLock`).
///
/// # Arguments
///
/// * `config` — OTel configuration (endpoint, service name, sampling, etc.).
///   Individual fields may be overridden by environment variables:
///   - `OTEL_EXPORTER_OTLP_ENDPOINT` overrides `config.endpoint`
///   - `OTEL_SERVICE_NAME` overrides `config.service_name`
///   - `OTEL_SAMPLE_RATE` overrides `config.sample_rate`
/// * `log_level` — Fallback log level when `RUST_LOG` is not set (e.g. `"info"`).
///
/// # Returns
///
/// `OtelGuards` containing the provider handles (or `None` for disabled providers).
/// Call `guards.shutdown()` on application exit.
pub fn init(
    config: &OtelConfig,
    log_level: &str,
) -> Result<OtelGuards, Box<dyn std::error::Error + Send + Sync>> {
    // Idempotent: if already initialized, return empty guards.
    if INITIALIZED.get().is_some() {
        return Ok(OtelGuards::empty());
    }

    // Resolve config with env overrides.
    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| config.endpoint.clone());
    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| config.service_name.clone());
    let sample_rate: f64 = std::env::var("OTEL_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(config.sample_rate);

    let timeout = Duration::from_millis(config.batch_max_export_timeout_millis);

    let resource = Resource::builder()
        .with_service_name(service_name.clone())
        .with_attributes([
            KeyValue::new("service.namespace", config.service_namespace.clone()),
            KeyValue::new(
                "deployment.environment",
                config.deployment_environment.clone(),
            ),
        ])
        .build();

    // Console layer (colored, human-readable).
    let console_layer = fmt::layer()
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stdout);

    // File layer (JSON, hourly rotation, 7-day retention).
    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix("agent")
        .filename_suffix("log")
        .max_log_files(168) // 7 days of hourly files
        .build(".")
        .unwrap_or_else(|_| {
            RollingFileAppender::builder()
                .rotation(Rotation::HOURLY)
                .filename_prefix("agent")
                .filename_suffix("log")
                .max_log_files(168)
                .build("/tmp")
                .expect("Failed to create file appender in /tmp")
        });

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true)
        .json()
        .with_current_span(true)
        .with_writer(file_appender);

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(log_level));

    if config.enabled && sample_rate > 0.0 {
        // --- Traces ---
        let span_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .with_timeout(timeout)
            .build()?;

        let sampler = if sample_rate >= 1.0 {
            Sampler::AlwaysOn
        } else {
            Sampler::TraceIdRatioBased(sample_rate)
        };

        let tracer_provider = SdkTracerProvider::builder()
            .with_sampler(sampler)
            .with_resource(resource.clone())
            .with_batch_exporter(span_exporter)
            .build();

        let tracer = tracer_provider.tracer(service_name.clone());
        let otel_trace_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_location(true)
            .with_threads(true);

        global::set_tracer_provider(tracer_provider.clone());

        // --- Logs ---
        let log_exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .with_timeout(timeout)
            .build()?;

        let logger_provider = SdkLoggerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(log_exporter)
            .build();

        let otel_log_layer =
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(
                &logger_provider,
            );

        // --- Metrics ---
        let metric_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&endpoint)
            .with_timeout(timeout)
            .build()?;

        let reader = PeriodicReader::builder(metric_exporter)
            .with_interval(Duration::from_secs(60))
            .build();

        let meter_provider = SdkMeterProvider::builder()
            .with_resource(resource.clone())
            .with_reader(reader)
            .build();

        global::set_meter_provider(meter_provider.clone());

        // --- Assemble subscriber ---
        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .with(otel_trace_layer)
            .with(otel_log_layer)
            .init();

        tracing::info!(
            "OTel full stack initialized: endpoint={} service={} sample_rate={}",
            endpoint,
            service_name,
            sample_rate
        );

        INITIALIZED.get_or_init(|| ());

        Ok(OtelGuards {
            tracer_provider: Some(tracer_provider),
            logger_provider: Some(logger_provider),
            meter_provider: Some(meter_provider),
        })
    } else {
        // OTel disabled — console + file only.
        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .init();

        tracing::info!("OTel disabled — console + file layers only");

        INITIALIZED.get_or_init(|| ());

        Ok(OtelGuards::empty())
    }
}

/// Initialize the tracing-subscriber with OTel log export (backward-compatible).
///
/// This is a thin wrapper around `init()` with default trace/metric sampling disabled.
/// Prefer `init()` for new code.
///
/// # Arguments
///
/// * `endpoint` — OTel Collector gRPC endpoint (e.g., `http://localhost:4317`).
///   Falls back to `OTEL_EXPORTER_OTLP_ENDPOINT` env var, then the provided value.
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
) -> Result<OtelGuards, Box<dyn std::error::Error + Send + Sync>> {
    let config = OtelConfig {
        enabled: false, // backward-compat: logs-only mode, no traces/metrics
        endpoint: endpoint.to_string(),
        service_name: service_name.to_string(),
        ..OtelConfig::default()
    };
    init(&config, "info")
}
