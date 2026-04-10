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

        // Create directory structure (best effort, don't fail if can't)
        if let Err(e) = std::fs::create_dir_all(agent_path.join("sessions")) {
            tracing::warn!(agent_id = %agent_id, error = %e, "Failed to create sessions directory");
        }
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

    fn get_session_log_path(&self, session_id: &str, date: &str) -> PathBuf {
        self.agent_path
            .join("sessions")
            .join(format!("session_{}_{}.jsonl", session_id, date))
    }

    fn get_run_log_path(&self, run_id: &str) -> PathBuf {
        self.agent_path
            .join("runs")
            .join(format!("{}.jsonl", run_id))
    }

    /// Log an event to both stdout (via tracing) and file
    pub async fn log(&self, entry: &LogEntry, log_type: &LogType) {
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
        let file_path = match log_type {
            LogType::Session { session_id, date } => {
                self.get_session_log_path(session_id, date)
            }
            LogType::Run { run_id } => self.get_run_log_path(run_id),
        };

        // Use tokio spawn to write file asynchronously without blocking
        let json_line_clone = json_line.clone();
        let _ = tokio::spawn(async move {
            let _ = append_to_file(&file_path, &json_line_clone).await;
        }).await;
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
pub enum LogType {
    Session { session_id: String, date: String },
    Run { run_id: String },
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
        }).to_string()
    }

    fn format_event_summary(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => format!("Agent started - input: {:?}",
                self.data.get("input").and_then(|v| v.as_str()).unwrap_or("")),
            "ThinkingComplete" => "Thinking complete".to_string(),
            "ToolCallBegin" => format!("Tool call: {}",
                self.data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown")),
            "ToolCallComplete" => format!("Tool result: {}",
                self.data.get("result").and_then(|v| v.as_str()).unwrap_or("")),
            "IterationComplete" => format!("Iteration {} complete",
                self.data.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0)),
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => format!("Agent aborted: {}",
                self.data.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown")),
            "PluginEvent" => format!("Plugin event: {}",
                self.data.get("name").and_then(|v| v.as_str()).unwrap_or("unknown")),
            _ => self.event.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use serde_json::json;

    #[tokio::test]
    async fn test_logger_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let log_base = temp_dir.path().join("logs");
        let agent_id = "test_agent";

        let logger = ObservabilityLogger::new(agent_id.to_string(), log_base.clone());

        // Logger should create directory structure
        let agent_path = log_base.join(agent_id);
        assert!(agent_path.exists());
        assert!(agent_path.join("sessions").exists());
        assert!(agent_path.join("runs").exists());
    }

    #[tokio::test]
    async fn test_logger_log_writes_to_file() {
        let temp_dir = TempDir::new().unwrap();
        let log_base = temp_dir.path().join("logs");
        let agent_id = "test_agent";

        let logger = ObservabilityLogger::new(agent_id.to_string(), log_base);

        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "test-run".to_string(),
            agent_id: agent_id.to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "test"}),
        };

        logger.log(&entry, &LogType::Run { run_id: "test-run".to_string() }).await;

        // Give async write time to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify file was created and contains the log entry
        let run_log_path = logger.agent_path.join("runs").join("test-run.jsonl");
        assert!(run_log_path.exists());

        let content = std::fs::read_to_string(&run_log_path).unwrap();
        assert!(content.contains("AgentStart"));
        assert!(content.contains("test-run"));
    }

    #[test]
    fn test_log_entry_to_json() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "123".to_string(),
            agent_id: "test_agent".to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "test"}),
        };

        let json_line = entry.to_json_line();
        assert!(json_line.contains("123"));
        assert!(json_line.contains("test_agent"));
        assert!(json_line.contains("AgentStart"));
    }
}
