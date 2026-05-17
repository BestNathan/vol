//! vol-llm-observability: JSONL event logging and observability for LLM agents.
//!
//! Provides:
//! - A `LoggerPlugin` that writes structured run logs as JSONL files
//! - An `init_otel_logs()` helper to initialize the OTel log layer
//! - A `LokiPlugin` that sends agent events to OTel via tracing macros

pub mod plugin;
pub mod run_log;
pub mod otel_init;

pub use plugin::LoggerPlugin;
pub use run_log::{LogEntry, append_log};
pub use otel_init::init_otel_logs;

pub mod loki_plugin;
pub use loki_plugin::LokiPlugin;
