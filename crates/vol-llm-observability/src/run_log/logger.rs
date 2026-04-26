//! LogEntry and file append utilities for LoggerPlugin.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub event: String,
    pub data: Value,
}

impl LogEntry {
    pub fn to_json_line(&self) -> String {
        json!({
            "timestamp": self.timestamp.to_rfc3339(),
            "run_id": self.run_id,
            "event": self.event,
            "data": self.data,
        }).to_string()
    }

    pub fn format_event_summary(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => format!("Agent started - input: {:?}", self.data.get("input").and_then(|v| v.as_str()).unwrap_or("")),
            "ThinkingComplete" => "Thinking complete".to_string(),
            "ToolCallBegin" => format!("Tool call: {}", self.data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown")),
            "ToolCallComplete" => format!("Tool result: {}", self.data.get("result").and_then(|v| v.as_str()).unwrap_or("")),
            "IterationComplete" => format!("Iteration {} complete", self.data.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0)),
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => format!("Agent aborted: {}", self.data.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown")),
            "PluginEvent" => format!("Plugin event: {}", self.data.get("name").and_then(|v| v.as_str()).unwrap_or("unknown")),
            _ => self.event.clone(),
        }
    }
}

/// Append a line to a file, creating it if it doesn't exist.
/// Creates parent directories as needed.
pub async fn append_log(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    // Single write to avoid interleaving with concurrent append_log calls.
    let mut buf = line.as_bytes().to_vec();
    buf.push(b'\n');
    file.write_all(&buf).await?;
    file.flush().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "r1".to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "hello"}),
        };
        let line = entry.to_json_line();
        assert!(line.contains("AgentStart"));
        assert!(line.contains("r1"));
        assert!(line.contains("hello"));
    }

    #[test]
    fn test_format_event_summary() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "r1".to_string(),
            event: "ToolCallBegin".to_string(),
            data: json!({"tool_name": "bash"}),
        };
        assert!(entry.format_event_summary().contains("bash"));
    }

    #[tokio::test]
    async fn test_append_log_creates_dirs_and_writes() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("logs/subdir/test.jsonl");
        append_log(&path, "hello").await.unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), "hello");
    }

    #[tokio::test]
    async fn test_append_log_appends() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("logs/test.jsonl");
        append_log(&path, "line1").await.unwrap();
        append_log(&path, "line2").await.unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 2);
    }
}
