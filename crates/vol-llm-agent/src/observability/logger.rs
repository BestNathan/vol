//! Async log writer for JSONL file logs and stdout output.

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;
use chrono::{DateTime, Utc};
use serde_json::Value;

pub struct ObservabilityLogger {
    agent_id: String,
    log_base_path: PathBuf,
}

impl ObservabilityLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        Self { agent_id, log_base_path }
    }
}

pub enum LogType {
    Session { session_id: String, date: String },
    Run { run_id: String },
}

pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: Value,
}
