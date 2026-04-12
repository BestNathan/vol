//! Async log writer for JSONL file logs using tracing_appender.
//!
//! stdout output is handled by tracing_subscriber::fmt::layer() globally.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tracing::info;

pub struct ObservabilityLogger {
    agent_id: String,
    agent_path: PathBuf,
}

impl ObservabilityLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        let agent_path = log_base_path.join(&agent_id);

        // Create runs directory (best effort, don't fail if can't)
        if let Err(e) = std::fs::create_dir_all(agent_path.join("runs")) {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to create runs directory");
        }

        Self {
            agent_id,
            agent_path,
        }
    }

    /// Get the agent ID
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    fn get_run_log_path(&self, run_id: &str) -> PathBuf {
        self.agent_path
            .join("runs")
            .join(format!("{}.jsonl", run_id))
    }

    /// Log an event to both stdout (via tracing) and file
    pub async fn log(&self, entry: &LogEntry, run_id: &str) {
        // Emit to stdout via tracing
        info!(
            run_id = %entry.run_id,
            agent_id = %entry.agent_id,
            event = %entry.event,
            "{}",
            entry.format_event_summary()
        );

        // Write JSON to file
        let json_line = entry.to_json_line();
        let file_path = self.get_run_log_path(run_id);

        // Use tokio spawn to write file asynchronously without blocking
        tokio::spawn(async move {
            let _ = append_to_file(&file_path, &json_line).await;
        });
    }
}

/// Append a line to a file (async helper)
async fn append_to_file(path: &Path, line: &str) -> Result<(), std::io::Error> {
    use tokio::fs::OpenOptions;
    use tokio::io::AsyncWriteExt;

    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;

    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    let _ = file.flush().await;

    Ok(())
}

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: Value,
}

impl LogEntry {
    /// Serialize log entry as JSON line
    pub fn to_json_line(&self) -> String {
        json!({
            "timestamp": self.timestamp.to_rfc3339(),
            "run_id": self.run_id,
            "agent_id": self.agent_id,
            "event": self.event,
            "data": self.data,
        })
        .to_string()
    }

    fn format_event_summary(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => format!(
                "Agent started - input: {:?}",
                self.data
                    .get("input")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            ),
            "ThinkingComplete" => "Thinking complete".to_string(),
            "ToolCallBegin" => format!(
                "Tool call: {}",
                self.data
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            ),
            "ToolCallComplete" => format!(
                "Tool result: {}",
                self.data
                    .get("result")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
            ),
            "IterationComplete" => format!(
                "Iteration {} complete",
                self.data
                    .get("iteration")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0)
            ),
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => format!(
                "Agent aborted: {}",
                self.data
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            ),
            "PluginEvent" => format!(
                "Plugin event: {}",
                self.data
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            ),
            _ => self.event.clone(),
        }
    }
}
