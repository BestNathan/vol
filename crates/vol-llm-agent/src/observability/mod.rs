//! Observability plugin for structured logging and log retention.

pub mod cleanup;
pub mod logger;
pub mod plugin;

pub use cleanup::{cleanup_old_logs, cleanup_run_logs, cleanup_session_logs};
pub use logger::{LogEntry, LogType, ObservabilityLogger};
pub use plugin::ObservabilityPlugin;
