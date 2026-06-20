//! vol-llm-observability: JSONL event logging and observability for LLM agents.
//!
//! Provides:
//! - A `LoggerPlugin` that writes structured run logs as JSONL files
//! - An `init()` function for full OTel initialization (traces + metrics + logs)
//! - A `LokiPlugin` that sends agent events to OTel via tracing macros
//! - A `MetricsPlugin` that records OTel metrics from agent events

pub mod loki_plugin;
pub mod metrics_plugin;
pub mod otel_init;
pub mod plugin;
pub mod run_log;

pub use loki_plugin::LokiPlugin;
pub use metrics_plugin::MetricsPlugin;
pub use otel_init::{init, init_otel_logs, OtelConfig, OtelGuards};
pub use plugin::LoggerPlugin;
pub use run_log::{append_log, LogEntry};
