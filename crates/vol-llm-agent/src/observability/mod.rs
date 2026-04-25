//! Observability re-exports from vol-llm-observability.
//!
//! LoggerPlugin lives in vol-llm-observability. This module provides
//! convenient re-exports for downstream crates.

pub mod plugin;
pub mod run_log;

// Re-export types
pub use vol_llm_observability::{LogEntry, append_log, LoggerPlugin};
pub use plugin::ObservabilityPlugin;
