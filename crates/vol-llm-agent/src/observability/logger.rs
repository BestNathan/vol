//! Async log writer for JSONL file logs and stdout output.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt;
use tracing;

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

    fn get_session_log_path(&self, session_id: &str, date: &str) -> PathBuf {
        self.agent_path
            .join("sessions")
            .join(format!("session_{}_{}.jsonl", session_id, date))
    }

    fn get_run_log_path(&self, run_id: &str) -> PathBuf {
        self.agent_path
            .join("runs")
            .join(format!("run_{}.jsonl", run_id))
    }

    /// Log an event to both file and stdout
    pub async fn log(&self, entry: LogEntry, log_type: LogType) {
        let json_line = entry.to_json_line();
        let stdout_line = entry.to_stdout_line();

        // Always print to stdout
        println!("{}", stdout_line);

        // Write to file (best effort)
        let file_path = match log_type {
            LogType::Session { session_id, date } => self.get_session_log_path(&session_id, &date),
            LogType::Run { run_id } => self.get_run_log_path(&run_id),
        };

        if let Err(e) = self.append_to_file(&file_path, &json_line).await {
            tracing::warn!(
                agent_id = %self.agent_id,
                run_id = %entry.run_id,
                file = %file_path.display(),
                error = %e,
                "Failed to write log entry"
            );
        }
    }

    async fn append_to_file(&self, path: &Path, line: &str) -> Result<(), std::io::Error> {
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await?;

        file.write_all(line.as_bytes()).await?;
        file.write_all(b"\n").await?;
        file.flush().await?;

        Ok(())
    }
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

    /// Format log entry for stdout (human-readable)
    pub fn to_stdout_line(&self) -> String {
        let level = match self.event.as_str() {
            "AgentAborted" | "AgentError" => "ERROR",
            "ToolCallBegin" | "ToolCallComplete" => "INFO",
            _ => "INFO",
        };

        let data_str = self.format_data_for_stdout();
        format!(
            "[{}] [{}] [{}] {}{}",
            level,
            self.agent_id,
            self.run_id,
            self.format_event_summary(),
            if data_str.is_empty() { String::new() } else { format!(" - {}", data_str) }
        )
    }

    fn format_event_summary(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => "Agent started".to_string(),
            "ThinkingComplete" => "Thinking complete".to_string(),
            "ToolCallBegin" => format!("Tool call: {}",
                self.data.get("tool_name").map(|v| v.as_str().unwrap_or("unknown")).unwrap_or("unknown")),
            "ToolCallComplete" => format!("Tool result: {}",
                self.data.get("result").map(|v| v.as_str().unwrap_or("")).unwrap_or("")),
            "IterationComplete" => format!("Iteration {} complete",
                self.data.get("iteration").map(|v| v.as_u64().unwrap_or(0)).unwrap_or(0)),
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => format!("Agent aborted: {}",
                self.data.get("reason").map(|v| v.as_str().unwrap_or("unknown")).unwrap_or("unknown")),
            "PluginEvent" => format!("Plugin event: {}",
                self.data.get("name").map(|v| v.as_str().unwrap_or("unknown")).unwrap_or("unknown")),
            _ => self.event.clone(),
        }
    }

    fn format_data_for_stdout(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => {
                self.data.get("input").map(|v| format!("input: {:?}", v.as_str().unwrap_or(""))).unwrap_or_default()
            }
            "ToolCallBegin" => {
                self.data.get("arguments").map(|v| v.to_string()).unwrap_or_default()
            }
            _ => String::new(),
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
            run_id: "run_test".to_string(),
            agent_id: agent_id.to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "test"}),
        };

        logger.log(entry.clone(), LogType::Run { run_id: "run_test".to_string() }).await;

        // Verify file was created and contains the log entry
        let run_log_path = logger.agent_path.join("runs").join("run_run_test.jsonl");
        assert!(run_log_path.exists());

        let content = std::fs::read_to_string(&run_log_path).unwrap();
        assert!(content.contains("AgentStart"));
        assert!(content.contains("run_test"));
    }

    #[test]
    fn test_log_entry_to_json() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "run_123".to_string(),
            agent_id: "test_agent".to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "test"}),
        };

        let json_line = entry.to_json_line();
        assert!(json_line.contains("run_123"));
        assert!(json_line.contains("test_agent"));
        assert!(json_line.contains("AgentStart"));
    }

    #[test]
    fn test_log_entry_to_stdout() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "run_123".to_string(),
            agent_id: "test_agent".to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "hello"}),
        };

        let stdout_line = entry.to_stdout_line();
        assert!(stdout_line.contains("[INFO]"));
        assert!(stdout_line.contains("[test_agent]"));
        assert!(stdout_line.contains("[run_123]"));
        assert!(stdout_line.contains("Agent started"));
    }
}
