//! vol-llm-observability: JSONL event logging and observability for LLM agents.
//!
//! Provides:
//! - A `LoggerPlugin` that writes structured run logs as JSONL files
//! - An `ObservabilityPlugin` that sends agent events to the observability service
//! - An `init_otel_logs()` helper to initialize the OTel log layer

pub mod plugin;
pub mod run_log;
pub mod otel_init;

pub mod agent_config;
pub mod agent_client;
pub mod agent_plugin;

pub use plugin::LoggerPlugin;
pub use run_log::{LogEntry, append_log};
pub use otel_init::init_otel_logs;

pub use agent_config::ObservabilityAgentConfig;
pub use agent_plugin::ObservabilityPlugin;
