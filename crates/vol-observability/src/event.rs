//! Ingest event types and deserialization.

use serde::{Deserialize, Serialize};

/// Event received from agent via HTTP ingest API.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestEvent {
    pub run_id: String,
    pub session_id: String,
    pub agent_id: String,
    pub agent_type: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub event: String,
    pub data: serde_json::Value,
}

/// Batch of events sent in a single HTTP POST.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IngestBatch {
    pub events: Vec<IngestEvent>,
}

/// Loki log entry: a single timestamped log line.
#[derive(Debug, Clone)]
pub struct LokiLogEntry {
    pub labels: std::collections::HashMap<String, String>,
    pub timestamp_nanos: i64,
    pub line: String,
}

impl IngestEvent {
    /// Convert to Loki log entry with appropriate labels.
    pub fn to_loki_entry(&self) -> LokiLogEntry {
        let mut labels = std::collections::HashMap::new();
        labels.insert("run_id".to_string(), self.run_id.clone());
        labels.insert("session_id".to_string(), self.session_id.clone());
        labels.insert("agent_id".to_string(), self.agent_id.clone());
        labels.insert("agent_type".to_string(), self.agent_type.clone());
        labels.insert("event_type".to_string(), self.event.clone());

        // Add tool_name label for tool-related events
        if let Some(tool_name) = self.data.get("tool_name").and_then(|v| v.as_str()) {
            labels.insert("tool_name".to_string(), tool_name.to_string());
        }

        let timestamp_nanos = self.timestamp.timestamp_nanos_opt().unwrap_or(0);
        let line = serde_json::to_string(&self.data).unwrap_or_default();

        LokiLogEntry {
            labels,
            timestamp_nanos,
            line,
        }
    }
}

/// Metric extracted from an event for TDengine storage.
#[derive(Debug, Clone)]
pub enum ExtractedMetric {
    AgentRun {
        run_id: String,
        session_id: String,
        agent_id: String,
        agent_type: String,
        timestamp: chrono::DateTime<chrono::Utc>,
        duration_ms: i64,
        iterations: i32,
        tool_calls: i32,
        final_answer_len: i32,
        status: i8,
    },
    LlmCall {
        run_id: String,
        session_id: String,
        agent_id: String,
        agent_type: String,
        timestamp: chrono::DateTime<chrono::Utc>,
        duration_ms: i64,
        iteration: i32,
        input_tokens: i32,
        output_tokens: i32,
        total_tokens: i32,
        model: String,
        is_error: bool,
    },
    ToolCall {
        run_id: String,
        session_id: String,
        agent_id: String,
        agent_type: String,
        timestamp: chrono::DateTime<chrono::Utc>,
        duration_ms: i64,
        status: i8,
        tool_name: String,
    },
}

impl ExtractedMetric {
    /// Extract metrics from an ingest event, if applicable.
    pub fn from_event(event: &IngestEvent) -> Option<Self> {
        match event.event.as_str() {
            "AgentComplete" => {
                let data = &event.data;
                let response = data.get("response")?;
                let iterations = response.get("iterations")?.as_u64()? as i32;
                let tool_calls = response
                    .get("tool_calls")?
                    .as_array()
                    .map(|a| a.len())
                    .unwrap_or(0) as i32;
                let content = response.get("content")?.as_str().unwrap_or("");

                Some(ExtractedMetric::AgentRun {
                    run_id: event.run_id.clone(),
                    session_id: event.session_id.clone(),
                    agent_id: event.agent_id.clone(),
                    agent_type: event.agent_type.clone(),
                    timestamp: event.timestamp,
                    duration_ms: 0,
                    iterations,
                    tool_calls,
                    final_answer_len: content.len() as i32,
                    status: 0,
                })
            }
            "AgentAborted" => Some(ExtractedMetric::AgentRun {
                run_id: event.run_id.clone(),
                session_id: event.session_id.clone(),
                agent_id: event.agent_id.clone(),
                agent_type: event.agent_type.clone(),
                timestamp: event.timestamp,
                duration_ms: 0,
                iterations: 0,
                tool_calls: 0,
                final_answer_len: 0,
                status: 1,
            }),
            "LLMCallComplete" => {
                let usage = event.data.get("usage");
                let (input_tokens, output_tokens, total_tokens) = if let Some(u) = usage {
                    (
                        u.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as i32,
                        u.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as i32,
                        u.get("total_tokens").and_then(|v| v.as_u64()).unwrap_or(0) as i32,
                    )
                } else {
                    (0, 0, 0)
                };
                let model = event
                    .data
                    .get("model")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                Some(ExtractedMetric::LlmCall {
                    run_id: event.run_id.clone(),
                    session_id: event.session_id.clone(),
                    agent_id: event.agent_id.clone(),
                    agent_type: event.agent_type.clone(),
                    timestamp: event.timestamp,
                    duration_ms: 0,
                    iteration: event
                        .data
                        .get("iteration")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0) as i32,
                    input_tokens,
                    output_tokens,
                    total_tokens,
                    model,
                    is_error: false,
                })
            }
            "LLMCallError" => Some(ExtractedMetric::LlmCall {
                run_id: event.run_id.clone(),
                session_id: event.session_id.clone(),
                agent_id: event.agent_id.clone(),
                agent_type: event.agent_type.clone(),
                timestamp: event.timestamp,
                duration_ms: 0,
                iteration: 0,
                input_tokens: 0,
                output_tokens: 0,
                total_tokens: 0,
                model: "unknown".to_string(),
                is_error: true,
            }),
            "ToolCallComplete" => {
                let duration_ms = event
                    .data
                    .get("duration_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as i64;
                let tool_name = event
                    .data
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                Some(ExtractedMetric::ToolCall {
                    run_id: event.run_id.clone(),
                    session_id: event.session_id.clone(),
                    agent_id: event.agent_id.clone(),
                    agent_type: event.agent_type.clone(),
                    timestamp: event.timestamp,
                    duration_ms,
                    status: 0,
                    tool_name,
                })
            }
            "ToolCallError" => {
                let duration_ms = event
                    .data
                    .get("duration_ms")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as i64;
                let tool_name = event
                    .data
                    .get("tool_name")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();

                Some(ExtractedMetric::ToolCall {
                    run_id: event.run_id.clone(),
                    session_id: event.session_id.clone(),
                    agent_id: event.agent_id.clone(),
                    agent_type: event.agent_type.clone(),
                    timestamp: event.timestamp,
                    duration_ms,
                    status: 1,
                    tool_name,
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ingest_event_deserialize() {
        let json = json!({
            "run_id": "run-1",
            "session_id": "session-1",
            "agent_id": "agent-1",
            "agent_type": "CodingAgent",
            "timestamp": 1714370000,
            "event": "ToolCallComplete",
            "data": {"tool_name": "bash", "result": "ok", "duration_ms": 150}
        });

        let event: IngestEvent = serde_json::from_value(json).unwrap();
        assert_eq!(event.run_id, "run-1");
        assert_eq!(event.agent_type, "CodingAgent");
        assert_eq!(event.event, "ToolCallComplete");
    }

    #[test]
    fn test_to_loki_entry_labels() {
        let event = IngestEvent {
            run_id: "run-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_type: "CodingAgent".to_string(),
            timestamp: chrono::Utc::now(),
            event: "ToolCallComplete".to_string(),
            data: json!({"tool_name": "bash", "result": "ok"}),
        };

        let entry = event.to_loki_entry();
        assert_eq!(entry.labels["run_id"], "run-1");
        assert_eq!(entry.labels["event_type"], "ToolCallComplete");
        assert_eq!(entry.labels["tool_name"], "bash");
        assert!(entry.line.contains("ok"));
    }

    #[test]
    fn test_extract_metric_tool_call_complete() {
        let event = IngestEvent {
            run_id: "run-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_type: "CodingAgent".to_string(),
            timestamp: chrono::Utc::now(),
            event: "ToolCallComplete".to_string(),
            data: json!({"tool_name": "bash", "duration_ms": 150, "result": "ok"}),
        };

        let metric = ExtractedMetric::from_event(&event).unwrap();
        match metric {
            ExtractedMetric::ToolCall {
                duration_ms,
                status,
                tool_name,
                ..
            } => {
                assert_eq!(duration_ms, 150);
                assert_eq!(status, 0);
                assert_eq!(tool_name, "bash");
            }
            _ => panic!("Expected ToolCall metric"),
        }
    }

    #[test]
    fn test_extract_metric_agent_complete() {
        let event = IngestEvent {
            run_id: "run-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_type: "CodingAgent".to_string(),
            timestamp: chrono::Utc::now(),
            event: "AgentComplete".to_string(),
            data: json!({
                "response": {
                    "iterations": 3,
                    "tool_calls": [{"name": "bash"}, {"name": "read"}],
                    "content": "done"
                }
            }),
        };

        let metric = ExtractedMetric::from_event(&event).unwrap();
        match metric {
            ExtractedMetric::AgentRun {
                iterations,
                tool_calls,
                final_answer_len,
                status,
                ..
            } => {
                assert_eq!(iterations, 3);
                assert_eq!(tool_calls, 2);
                assert_eq!(final_answer_len, 4);
                assert_eq!(status, 0);
            }
            _ => panic!("Expected AgentRun metric"),
        }
    }

    #[test]
    fn test_extract_metric_llm_call_complete() {
        let event = IngestEvent {
            run_id: "run-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_type: "CodingAgent".to_string(),
            timestamp: chrono::Utc::now(),
            event: "LLMCallComplete".to_string(),
            data: json!({
                "model": "qwen3.5-plus",
                "iteration": 1,
                "usage": {
                    "input_tokens": 100,
                    "output_tokens": 50,
                    "total_tokens": 150
                }
            }),
        };

        let metric = ExtractedMetric::from_event(&event).unwrap();
        match metric {
            ExtractedMetric::LlmCall {
                input_tokens,
                output_tokens,
                total_tokens,
                model,
                is_error,
                ..
            } => {
                assert_eq!(input_tokens, 100);
                assert_eq!(output_tokens, 50);
                assert_eq!(total_tokens, 150);
                assert_eq!(model, "qwen3.5-plus");
                assert!(!is_error);
            }
            _ => panic!("Expected LlmCall metric"),
        }
    }

    #[test]
    fn test_extract_metric_no_match() {
        let event = IngestEvent {
            run_id: "run-1".to_string(),
            session_id: "session-1".to_string(),
            agent_id: "agent-1".to_string(),
            agent_type: "CodingAgent".to_string(),
            timestamp: chrono::Utc::now(),
            event: "ThinkingStart".to_string(),
            data: json!({}),
        };

        assert!(ExtractedMetric::from_event(&event).is_none());
    }
}
