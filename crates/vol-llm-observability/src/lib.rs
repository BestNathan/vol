//! vol-llm-observability: JSONL event logging and OTel log export for LLM agents.
//!
//! Provides:
//! - A `LoggerPlugin` that writes structured run logs as JSONL files
//! - A `LokiPlugin` that sends agent events to OTel Collector via `tracing::info!`
//! - An `init_otel_logs()` helper to initialize the OTel log layer

pub mod plugin;
pub mod run_log;
pub mod loki;
pub mod otel_init;

pub use plugin::LoggerPlugin;
pub use run_log::{LogEntry, append_log};
pub use otel_init::init_otel_logs;
