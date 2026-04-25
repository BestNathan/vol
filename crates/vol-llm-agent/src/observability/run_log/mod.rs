//! Run log sub-package — re-exported from vol-llm-observability.

pub use vol_llm_observability::run_log::{LogEntry, append_log};
pub use vol_llm_observability::run_log::cleanup::{
    cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogError,
};
