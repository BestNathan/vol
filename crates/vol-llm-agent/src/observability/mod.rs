//! Observability plugin for structured logging and log retention.
//!
//! Core implementation lives in `vol-llm-observability` crate.
//! This module re-exports for backward compatibility.

pub mod plugin;
pub mod run_log;

// Re-export cleanup from vol-llm-observability
pub use vol_llm_observability::run_log::cleanup::{
    cleanup_old_logs, cleanup_run_logs, cleanup_session_logs,
};
// Re-export types
pub use vol_llm_observability::{LogEntry, RunLogLogger};
pub use plugin::ObservabilityPlugin;
