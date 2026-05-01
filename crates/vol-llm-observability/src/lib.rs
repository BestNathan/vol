//! vol-llm-observability: JSONL event logging and observability for LLM agents.
//!
//! Provides:
//! - `LoggerPlugin`: Writes structured run logs as JSONL files
//! - `ObservabilityPlugin`: Sends agent events to the observability service

pub mod plugin;
pub mod run_log;

pub mod agent_config;
pub mod agent_client;
pub mod agent_plugin;

pub use plugin::LoggerPlugin;
pub use run_log::{LogEntry, append_log};

pub use agent_config::ObservabilityAgentConfig;
pub use agent_plugin::ObservabilityPlugin;
