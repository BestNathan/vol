//! Run log sub-package for structured JSONL logging.

mod logger;
pub mod cleanup;

pub use logger::{LogEntry, RunLogLogger};
pub use cleanup::{cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogError};
