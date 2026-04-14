//! vol-llm-observability: Tracing, metrics, and audit logging for LLM agents.
//!
//! Provides an `ObservabilityPlugin` that implements `AgentPlugin` to:
//! - Record structured run logs (JSONL)
//! - Collect metrics (TTFT, tool latency, token usage)
//! - Create tracing spans for LLM calls and tool executions
//! - Clean up old logs based on retention policy

pub mod config;
pub mod metrics;
pub mod plugin;
pub mod run_log;
pub mod tracing;

pub use config::ObservabilityConfig;
pub use metrics::MetricsCollector;
pub use plugin::ObservabilityPlugin;
