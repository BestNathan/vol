//! vol-llm-observability: Agent observability plugins and OTel initialization.
//!
//! - `LoggingPlugin` — emits structured JSON agent events to stdout via tracing
//! - `MetricsPlugin` — records OTel metrics
//! - `otel_init` — full OTel initialization (traces + logs via OTLP push, metrics via Prometheus pull)

pub mod logging_plugin;
pub mod metrics_plugin;
pub mod metrics_router;
pub mod otel_init;

pub use logging_plugin::LoggingPlugin;
pub use metrics_plugin::MetricsPlugin;
pub use metrics_router::build_metrics_router;
pub use otel_init::{init, OtelConfig, OtelGuards};
