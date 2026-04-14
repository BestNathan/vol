//! Observability configuration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the observability plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable/disable run log recording.
    pub enable_run_log: bool,
    /// Enable/disable metrics collection.
    pub enable_metrics: bool,
    /// Enable/disable tracing spans.
    pub enable_tracing: bool,
    /// Base path for agent logs.
    pub log_base_path: PathBuf,
    /// Maximum number of run log files to retain per agent.
    pub max_run_logs: usize,
    /// Number of days to retain session logs.
    pub session_retention_days: u32,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_run_log: true,
            enable_metrics: true,
            enable_tracing: true,
            log_base_path: PathBuf::from("logs/agents"),
            max_run_logs: 10,
            session_retention_days: 7,
        }
    }
}
