//! Observability plugin for structured logging and log retention.

pub mod logger;
pub mod cleanup;
pub mod plugin;

pub use logger::{ObservabilityLogger, LogEntry, LogType};
pub use cleanup::{cleanup_old_logs, cleanup_session_logs, cleanup_run_logs};
pub use plugin::ObservabilityPlugin;
