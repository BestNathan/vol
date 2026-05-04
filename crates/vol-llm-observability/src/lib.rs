//! vol-llm-observability: JSONL event logging for LLM agents.
//!
//! Provides a `LoggerPlugin` that implements `AgentPlugin` to:
//! - Write structured run logs as JSONL files

pub mod plugin;
pub mod run_log;
pub mod loki;

pub use plugin::LoggerPlugin;
pub use run_log::{LogEntry, append_log};
