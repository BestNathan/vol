//! Run log sub-package for structured JSONL logging.

pub mod logger;

pub use logger::{LogEntry, append_log};
