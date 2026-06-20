//! Full OTel initialization: traces + metrics + logs.

use std::sync::OnceLock;

use opentelemetry::global;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry::KeyValue;
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::logs::SdkLoggerProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use opentelemetry_sdk::trace::{Sampler, SdkTracerProvider};
use opentelemetry_sdk::Resource;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter, Registry};

static INITIALIZED: OnceLock<()> = OnceLock::new();

/// Configuration for OTel initialization.
#[derive(Debug, Clone)]
pub struct OtelConfig {
    pub enabled: bool,
    pub endpoint: String,
    pub service_name: String,
    pub service_namespace: String,
    pub deployment_environment: String,
    pub sample_rate: f64,
    pub batch_max_export_timeout_millis: u64,
}

/// Guards that keep OTel providers alive. Must be held for the app's lifetime.
pub struct OtelGuards {
    pub tracer_provider: Option<SdkTracerProvider>,
    pub logger_provider: Option<SdkLoggerProvider>,
    pub meter_provider: Option<SdkMeterProvider>,
}

impl OtelGuards {
    /// Shutdown all providers gracefully, flushing pending spans/logs/metrics.
    pub fn shutdown(self) {
        if let Some(tp) = self.tracer_provider {
            let _ = tp.shutdown();
        }
        if let Some(lp) = self.logger_provider {
            let _ = lp.shutdown();
        }
        if let Some(mp) = self.meter_provider {
            let _ = mp.shutdown();
        }
    }
}

/// Initialize the full OTel stack: traces + metrics + logs.
pub fn init(
    config: &OtelConfig,
    log_level: &str,
) -> Result<OtelGuards, Box<dyn std::error::Error + Send + Sync>> {
    if INITIALIZED.get().is_some() {
        return Ok(OtelGuards {
            tracer_provider: None,
            logger_provider: None,
            meter_provider: None,
        });
    }

    let resolved_endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| config.endpoint.clone());
    let resolved_service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| config.service_name.clone());
    let resolved_sample_rate: f64 = std::env::var("OTEL_SAMPLE_RATE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(config.sample_rate);

    let resource = Resource::builder()
        .with_service_name(resolved_service_name.clone())
        .with_attributes([
            KeyValue::new("service.namespace", config.service_namespace.clone()),
            KeyValue::new("deployment.environment", config.deployment_environment.clone()),
        ])
        .build();

    let timeout = std::time::Duration::from_millis(config.batch_max_export_timeout_millis);

    let console_layer = fmt::layer()
        .with_target(true)
        .with_file(true)
        .with_line_number(true)
        .with_ansi(true)
        .with_writer(std::io::stdout);

    let file_appender = RollingFileAppender::builder()
        .rotation(Rotation::HOURLY)
        .filename_prefix("agent")
        .filename_suffix("log")
        .max_log_files(168)
        .build(".")
        .unwrap_or_else(|_| {
            RollingFileAppender::builder()
                .rotation(Rotation::HOURLY)
                .filename_prefix("agent")
                .filename_suffix("log")
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

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(log_level));

    if config.enabled && resolved_sample_rate > 0.0 {
        let span_exporter = opentelemetry_otlp::SpanExporter::builder()
            .with_tonic()
            .with_endpoint(&resolved_endpoint)
            .with_timeout(timeout)
            .build()?;

        let sampler = if resolved_sample_rate >= 1.0 {
            Sampler::AlwaysOn
        } else {
            Sampler::TraceIdRatioBased(resolved_sample_rate)
        };

        let tracer_provider = SdkTracerProvider::builder()
            .with_sampler(sampler)
            .with_resource(resource.clone())
            .with_batch_exporter(span_exporter)
            .build();

        let tracer = tracer_provider.tracer(resolved_service_name.clone());
        let otel_trace_layer = tracing_opentelemetry::layer()
            .with_tracer(tracer)
            .with_location(true)
            .with_threads(true);

        global::set_tracer_provider(tracer_provider.clone());

        let log_exporter = opentelemetry_otlp::LogExporter::builder()
            .with_tonic()
            .with_endpoint(&resolved_endpoint)
            .with_timeout(timeout)
            .build()?;

        let logger_provider = SdkLoggerProvider::builder()
            .with_resource(resource.clone())
            .with_batch_exporter(log_exporter)
            .build();

        let otel_log_layer =
            opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge::new(&logger_provider);

        let metrics_exporter = opentelemetry_otlp::MetricExporter::builder()
            .with_tonic()
            .with_endpoint(&resolved_endpoint)
            .with_timeout(timeout)
            .build()?;

        let meter_provider = SdkMeterProvider::builder()
            .with_resource(resource.clone())
            .with_periodic_exporter(metrics_exporter)
            .build();

        global::set_meter_provider(meter_provider.clone());

        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .with(otel_trace_layer)
            .with(otel_log_layer)
            .init();

        tracing::info!(
            endpoint = %resolved_endpoint,
            service = %resolved_service_name,
            sample_rate = resolved_sample_rate,
            "OpenTelemetry enabled: traces + metrics + logs"
        );

        INITIALIZED.get_or_init(|| ());

        Ok(OtelGuards {
            tracer_provider: Some(tracer_provider),
            logger_provider: Some(logger_provider),
            meter_provider: Some(meter_provider),
        })
    } else {
        type OtelLogLayer = opentelemetry_appender_tracing::layer::OpenTelemetryTracingBridge<
            SdkLoggerProvider,
            opentelemetry_sdk::logs::SdkLogger,
        >;

        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .with(Option::<OtelLogLayer>::None)
            .init();

        tracing::info!("OpenTelemetry disabled: console + file logging only");

        INITIALIZED.get_or_init(|| ());

        Ok(OtelGuards {
            tracer_provider: None,
            logger_provider: None,
            meter_provider: None,
        })
    }
}

/// Backward-compatible init_otel_logs. Deprecated: prefer init().
pub fn init_otel_logs(
    endpoint: &str,
    service_name: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let config = OtelConfig {
        enabled: true,
        endpoint: endpoint.to_string(),
        service_name: service_name.to_string(),
        service_namespace: "vol-agent".to_string(),
        deployment_environment: "development".to_string(),
        sample_rate: 1.0,
        batch_max_export_timeout_millis: 5000,
    };
    let _guards = init(&config, "info")?;
    Ok(())
}
