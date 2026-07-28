use std::path::PathBuf;

use async_trait::async_trait;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentServerMessage, LogOperation, LogPayload, Operation, Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;

/// Handler for log-domain operations.
///
/// Reads from the run log JSONL files written by `RunLogPlugin`
/// under `{logs_dir}/{run_id}.jsonl`.
pub struct LogHandler {
    logs_dir: PathBuf,
}

impl LogHandler {
    pub fn new(logs_dir: PathBuf) -> Self {
        Self { logs_dir }
    }

    /// Scan `logs_dir` for `*.jsonl` files and return a summary per run.
    async fn list_runs(&self) -> Result<Vec<serde_json::Value>, String> {
        let mut runs = Vec::new();
        let mut entries = match tokio::fs::read_dir(&self.logs_dir).await {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(runs),
            Err(e) => return Err(format!("failed to read logs dir: {e}")),
        };
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| format!("failed to read dir entry: {e}"))?
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let run_id = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string();

            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "Failed to read log file");
                    continue;
                }
            };

            let mut event_count = 0usize;
            let mut last_event = String::new();
            let mut last_event_time = String::new();

            for line in content.lines() {
                if line.trim().is_empty() {
                    continue;
                }
                event_count += 1;
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                    last_event = val
                        .get("event")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                    last_event_time = val
                        .get("timestamp")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .to_string();
                }
            }

            runs.push(serde_json::json!({
                "run_id": run_id,
                "event_count": event_count,
                "last_event": last_event,
                "last_event_time": last_event_time,
            }));
        }
        // Sort by last_event_time descending (newest first).
        runs.sort_by(|a, b| {
            b.get("last_event_time")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .cmp(
                    a.get("last_event_time")
                        .and_then(|v| v.as_str())
                        .unwrap_or(""),
                )
        });
        Ok(runs)
    }

    /// Read all log entries for a given run_id and return them as log lines.
    async fn read_run(&self, run_id: &str) -> Result<Vec<serde_json::Value>, String> {
        let path = self.logs_dir.join(format!("{run_id}.jsonl"));
        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(format!("log file not found for run_id: {run_id}"));
            }
            Err(e) => return Err(format!("failed to read log file: {e}")),
        };

        let mut entries = Vec::new();
        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(line) {
                let timestamp = val
                    .get("timestamp")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let event_type = val
                    .get("event")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                // Build a human-readable summary from the event data.
                let summary = Self::build_summary(&event_type, &val);
                entries.push(serde_json::json!({
                    "timestamp": timestamp,
                    "event_type": event_type,
                    "summary": summary,
                }));
            }
        }
        Ok(entries)
    }

    fn build_summary(event_type: &str, data: &serde_json::Value) -> String {
        match event_type {
            "AgentStart" => {
                let input = data.get("input").and_then(|v| v.as_str()).unwrap_or("");
                let preview: String = input.chars().take(80).collect();
                if input.chars().count() > 80 {
                    format!("Agent started — input: {preview}...")
                } else {
                    format!("Agent started — input: {preview}")
                }
            }
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => {
                let reason = data
                    .get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown");
                format!("Agent aborted: {reason}")
            }
            "LLMCallStart" => {
                let iteration = data
                    .get("iteration")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let msg_count = data
                    .get("message_count")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                format!("LLM call iteration={iteration}, {msg_count} messages")
            }
            "LLMCallComplete" => {
                let model = data.get("model").and_then(|v| v.as_str()).unwrap_or("");
                format!("LLM call complete — model: {model}")
            }
            "LLMCallError" => {
                let error = data.get("error").and_then(|v| v.as_str()).unwrap_or("");
                format!("LLM call error: {error}")
            }
            "ThinkingStart" => "Thinking started".to_string(),
            "ThinkingComplete" => {
                let thinking = data.get("thinking").and_then(|v| v.as_str()).unwrap_or("");
                let preview: String = thinking.chars().take(60).collect();
                if thinking.chars().count() > 60 {
                    format!("Thinking: {preview}...")
                } else {
                    format!("Thinking: {preview}")
                }
            }
            "ContentStart" => "Content generation started".to_string(),
            "ContentComplete" => {
                let content = data.get("content").and_then(|v| v.as_str()).unwrap_or("");
                let preview: String = content.chars().take(100).collect();
                if content.chars().count() > 100 {
                    format!("Content: {preview}...")
                } else {
                    format!("Content: {preview}")
                }
            }
            "ToolCallBegin" => {
                let tool = data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                format!("Tool call: {tool}")
            }
            "ToolCallComplete" => {
                let tool = data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let dur = data
                    .get("duration_ms")
                    .and_then(serde_json::Value::as_u64)
                    .map(|d| format!(" ({d}ms)"))
                    .unwrap_or_default();
                format!("Tool call complete: {tool}{dur}")
            }
            "ToolCallError" => {
                let tool = data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                let error = data.get("error").and_then(|v| v.as_str()).unwrap_or("");
                format!("Tool error: {tool} — {error}")
            }
            "ToolCallSkipped" => {
                let tool = data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
                format!("Tool skipped: {tool}")
            }
            "IterationComplete" => {
                let iter = data
                    .get("iteration")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let tc = data
                    .get("tool_calls")
                    .and_then(|v| v.as_array())
                    .map(Vec::len)
                    .unwrap_or(0);
                format!("Iteration {iter} complete ({tc} tool calls)")
            }
            "MaxIterationsReached" => {
                let cur = data
                    .get("current_iteration")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                let max = data
                    .get("max_iterations")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                format!("Max iterations: {cur}/{max}")
            }
            "IterationContinued" => {
                let from = data
                    .get("from_iteration")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0);
                format!("Continued from iteration {from}")
            }
            "PluginEvent" => {
                let name = data.get("name").and_then(|v| v.as_str()).unwrap_or("");
                format!("Plugin event: {name}")
            }
            _ => event_type.to_string(),
        }
    }
}

#[async_trait]
impl DomainHandler for LogHandler {
    fn name(&self) -> &str {
        "log"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Log(LogOperation::List),
            Operation::Log(LogOperation::Read),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Log(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("log")),
        };
        match (op, message.payload) {
            (LogOperation::List, Payload::Log(LogPayload::List)) => {
                let runs = self.list_runs().await.unwrap_or_else(|e| {
                    tracing::warn!(error = %e, "Failed to list log runs");
                    vec![]
                });
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::List),
                    Payload::Log(LogPayload::ListResult { runs }),
                )])
            }
            (LogOperation::Read, Payload::Log(LogPayload::Read { run_id })) => {
                let entries = self.read_run(&run_id).await.unwrap_or_else(|e| {
                    tracing::warn!(run_id = %run_id, error = %e, "Failed to read log run");
                    vec![]
                });
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Log(LogOperation::Read),
                    Payload::Log(LogPayload::ReadResult { entries }),
                )])
            }
            (LogOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("log.list")),
            (LogOperation::Read, _) => Err(ProtocolError::PayloadDecodeFailed("log.read")),
        }
    }
}

#[cfg(test)]
mod tests {
    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentServerMessage, LogOperation, LogPayload, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;

    use super::LogHandler;

    fn msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".to_string(),
            message_id: id.to_string(),
            sender: "client".to_string(),
            receiver: "data-plane".to_string(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn log_list_returns_empty_runs() {
        let temp = tempfile::TempDir::new().unwrap();
        let handler = LogHandler::new(temp.path().join("logs"));
        let replies = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::List),
                Payload::Log(LogPayload::List),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        let runs = json["runs"].as_array().unwrap();
        assert!(runs.is_empty());
    }

    #[tokio::test]
    async fn log_read_returns_empty_entries() {
        let temp = tempfile::TempDir::new().unwrap();
        let handler = LogHandler::new(temp.path().join("logs"));
        let replies = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::Read),
                Payload::Log(LogPayload::Read {
                    run_id: "run-1".to_string(),
                }),
            ))
            .await
            .unwrap();
        let json = replies[0].payload.data_json();
        let entries = json["entries"].as_array().unwrap();
        assert!(entries.is_empty());
    }

    #[tokio::test]
    async fn log_list_with_wrong_payload_returns_error() {
        let temp = tempfile::TempDir::new().unwrap();
        let handler = LogHandler::new(temp.path().join("logs"));
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::List),
                Payload::Log(LogPayload::Read {
                    run_id: "run-1".to_string(),
                }),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("log.list"));
    }

    #[tokio::test]
    async fn log_read_with_wrong_payload_returns_error() {
        let temp = tempfile::TempDir::new().unwrap();
        let handler = LogHandler::new(temp.path().join("logs"));
        let err = handler
            .handle(msg(
                "1",
                Operation::Log(LogOperation::Read),
                Payload::Log(LogPayload::List),
            ))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("log.read"));
    }
}
