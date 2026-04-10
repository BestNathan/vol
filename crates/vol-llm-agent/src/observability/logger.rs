//! Async log writer for JSONL file logs and stdout output.

use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use std::path::PathBuf;

pub struct ObservabilityLogger {
    agent_id: String,
    log_base_path: PathBuf,
}

impl ObservabilityLogger {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        Self { agent_id, log_base_path }
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
    use serde_json::json;

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
