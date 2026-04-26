//! LoggerPlugin - Writes agent events to JSONL files.

use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use vol_llm_core::plugin::{AgentPlugin, PluginContext, PluginDecision};
use vol_llm_core::stream::AgentStreamEvent;

use crate::run_log::logger::{LogEntry, append_log};

/// Writes all agent events to JSONL files.
///
/// File layout:
///   {base_dir}/logs/{run_id}.jsonl          (regular events)
///   {base_dir}/logs/{plugin_name}/{run_id}.jsonl  (PluginEvent)
pub struct LoggerPlugin {
    base_dir: PathBuf,
}

impl LoggerPlugin {
    pub fn new(base_dir: PathBuf) -> Self {
        let logs_dir = base_dir.join("logs");
        if let Err(e) = std::fs::create_dir_all(&logs_dir) {
            tracing::warn!(error = %e, "Failed to create logs directory");
        }
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &std::path::Path {
        &self.base_dir
    }

    pub fn log_path(&self, event: &AgentStreamEvent, run_id: &str) -> PathBuf {
        match event {
            AgentStreamEvent::PluginEvent { name, .. } => {
                self.base_dir.join("logs").join(name).join(format!("{run_id}.jsonl"))
            }
            _ => self.base_dir.join("logs").join(format!("{run_id}.jsonl")),
        }
    }

    /// Whether an event should be logged to the JSONL file.
    /// Skips high-frequency streaming delta events.
    fn should_log(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    fn create_log_entry(event: &AgentStreamEvent, run_id: &str) -> LogEntry {
        let data = match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                json!({ "input": input })
            }
            AgentStreamEvent::AgentComplete { response, .. } => {
                json!({ "response": response })
            }
            AgentStreamEvent::AgentAborted { reason, .. } => {
                json!({ "reason": reason })
            }
            AgentStreamEvent::LLMCallStart { iteration, messages, .. } => {
                let last_n: Vec<_> = messages.iter().rev().take(5).rev().collect();
                let msgs: Vec<Value> = last_n.iter().map(|m| {
                    let content = m.content.as_ref().map(|c| {
                        let s = c.as_str();
                        if s.chars().count() > 100 {
                            let truncated: String = s.chars().take(100).collect();
                            format!("{}...", truncated)
                        } else {
                            s.to_string()
                        }
                    }).unwrap_or_default();
                    json!({ "role": m.role, "content": content })
                }).collect();
                json!({ "iteration": iteration, "message_count": messages.len(), "messages": msgs })
            }
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
                json!({ "model": model, "usage": usage })
            }
            AgentStreamEvent::LLMCallError { error, .. } => {
                json!({ "error": error })
            }
            AgentStreamEvent::ThinkingStart { .. } => {
                json!({})
            }
            AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                json!({ "thinking": thinking })
            }
            AgentStreamEvent::ContentStart { .. } => {
                json!({})
            }
            AgentStreamEvent::ContentComplete { content, .. } => {
                json!({ "content": content })
            }
            AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, arguments, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "arguments": arguments })
            }
            AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "result": result, "duration_ms": duration_ms })
            }
            AgentStreamEvent::ToolCallError { tool_call_id, tool_name, error, duration_ms, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "error": error, "duration_ms": duration_ms })
            }
            AgentStreamEvent::ToolCallSkipped { tool_call_id, tool_name, reason, duration_ms, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "reason": reason, "duration_ms": duration_ms })
            }
            AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer, .. } => {
                let tc: Vec<Value> = tool_calls.iter().map(|tc| {
                    json!({
                        "id": &tc.id,
                        "name": &tc.name,
                        "arguments": &tc.arguments,
                        "type": &tc.r#type,
                    })
                }).collect();
                json!({ "iteration": iteration, "tool_calls": tc, "final_answer": final_answer })
            }
            AgentStreamEvent::PluginEvent { name, data, .. } => {
                let mut map = serde_json::Map::new();
                map.insert("name".to_string(), Value::String(name.clone()));
                for (k, v) in data {
                    map.insert(k.clone(), v.clone());
                }
                Value::Object(map)
            }
            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                json!({ "current_iteration": current_iteration, "max_iterations": max_iterations })
            }
            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                json!({ "from_iteration": from_iteration })
            }
            // Delta events are filtered out by should_log() but required for exhaustive matching
            AgentStreamEvent::ThinkingDelta { .. }
            | AgentStreamEvent::ContentDelta { .. }
            | AgentStreamEvent::ToolCallArgumentDelta { .. } => {
                unreachable!("delta events should be filtered by should_log()")
            }
        };

        let event_name = event_name(event);
        LogEntry {
            timestamp: Utc::now(),
            run_id: run_id.to_string(),
            event: event_name,
            data,
        }
    }
}

fn event_name(event: &AgentStreamEvent) -> String {
    match event {
        AgentStreamEvent::AgentStart { .. } => "AgentStart".to_string(),
        AgentStreamEvent::AgentComplete { .. } => "AgentComplete".to_string(),
        AgentStreamEvent::AgentAborted { .. } => "AgentAborted".to_string(),
        AgentStreamEvent::LLMCallStart { .. } => "LLMCallStart".to_string(),
        AgentStreamEvent::LLMCallComplete { .. } => "LLMCallComplete".to_string(),
        AgentStreamEvent::LLMCallError { .. } => "LLMCallError".to_string(),
        AgentStreamEvent::ThinkingStart { .. } => "ThinkingStart".to_string(),
        AgentStreamEvent::ThinkingComplete { .. } => "ThinkingComplete".to_string(),
        AgentStreamEvent::ContentStart { .. } => "ContentStart".to_string(),
        AgentStreamEvent::ContentComplete { .. } => "ContentComplete".to_string(),
        AgentStreamEvent::ToolCallBegin { .. } => "ToolCallBegin".to_string(),
        AgentStreamEvent::ToolCallComplete { .. } => "ToolCallComplete".to_string(),
        AgentStreamEvent::ToolCallError { .. } => "ToolCallError".to_string(),
        AgentStreamEvent::ToolCallSkipped { .. } => "ToolCallSkipped".to_string(),
        AgentStreamEvent::IterationComplete { .. } => "IterationComplete".to_string(),
        AgentStreamEvent::PluginEvent { .. } => "PluginEvent".to_string(),
        AgentStreamEvent::MaxIterationsReached { .. } => "MaxIterationsReached".to_string(),
        AgentStreamEvent::IterationContinued { .. } => "IterationContinued".to_string(),
        // Delta events are filtered out by should_log() but required for exhaustive matching
        AgentStreamEvent::ThinkingDelta { .. }
        | AgentStreamEvent::ContentDelta { .. }
        | AgentStreamEvent::ToolCallArgumentDelta { .. } => {
            unreachable!("delta events should be filtered by should_log()")
        }
    }
}

#[async_trait]
impl AgentPlugin for LoggerPlugin {
    fn id(&self) -> String {
        "logger".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        if !Self::should_log(event) {
            return;
        }
        let entry = Self::create_log_entry(event, &ctx.run_id);
        let path = self.log_path(event, &ctx.run_id);
        let line = entry.to_json_line();
        if let Err(e) = append_log(&path, &line).await {
            tracing::warn!(path = %path.display(), error = %e, "Failed to write log entry");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    use tempfile::TempDir;

    fn create_test_plugin(temp_dir: &TempDir) -> LoggerPlugin {
        LoggerPlugin::new(temp_dir.path().to_path_buf())
    }

    fn create_test_context() -> PluginContext {
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::sync::RwLock;
        PluginContext {
            run_id: "test-run".to_string(),
            user_input: "test input".to_string(),
            session_id: "session-1".to_string(),
            all_tool_calls: Arc::new(RwLock::new(Vec::new())),
            current_tool_calls: Arc::new(RwLock::new(Vec::new())),
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[test]
    fn test_plugin_id() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        assert_eq!(plugin.id(), "logger");
    }

    #[test]
    fn test_plugin_priority() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        assert_eq!(plugin.priority(), 10);
    }

    #[test]
    fn test_should_log_skips_delta_events() {
        assert!(LoggerPlugin::should_log(&AgentStreamEvent::ThinkingStart {
            timestamp: Utc::now(),
        }));
        assert!(!LoggerPlugin::should_log(&AgentStreamEvent::ThinkingDelta {
            timestamp: Utc::now(),
            delta: "chunk".to_string(),
        }));
        assert!(LoggerPlugin::should_log(&AgentStreamEvent::ThinkingComplete {
            timestamp: Utc::now(),
            thinking: "done".to_string(),
        }));
        assert!(!LoggerPlugin::should_log(&AgentStreamEvent::ContentDelta {
            timestamp: Utc::now(),
            delta: "partial".to_string(),
        }));
        assert!(LoggerPlugin::should_log(&AgentStreamEvent::ContentComplete {
            timestamp: Utc::now(),
            content: "full".to_string(),
        }));
        assert!(!LoggerPlugin::should_log(&AgentStreamEvent::ToolCallArgumentDelta {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            delta: "arg".to_string(),
        }));
        assert!(LoggerPlugin::should_log(&AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_log_path_regular_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        let path = plugin.log_path(&event, "run-1");
        assert_eq!(path, temp_dir.path().join("logs/run-1.jsonl"));
    }

    #[test]
    fn test_log_path_plugin_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let path = plugin.log_path(&event, "run-1");
        assert_eq!(path, temp_dir.path().join("logs/my_plugin/run-1.jsonl"));
    }

    #[test]
    fn test_log_entry_all_variants() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_context();

        let events = vec![
            AgentStreamEvent::AgentStart {
                timestamp: Utc::now(),
                input: "hello".to_string(),
            },
            AgentStreamEvent::AgentComplete {
                timestamp: Utc::now(),
                response: None,
            },
            AgentStreamEvent::AgentAborted {
                timestamp: Utc::now(),
                reason: "stop".to_string(),
            },
            AgentStreamEvent::LLMCallStart {
                timestamp: Utc::now(),
                iteration: 1,
                messages: vec![],
            },
            AgentStreamEvent::LLMCallComplete {
                timestamp: Utc::now(),
                model: "test".to_string(),
                usage: None,
            },
            AgentStreamEvent::LLMCallError {
                timestamp: Utc::now(),
                error: "timeout".to_string(),
            },
            AgentStreamEvent::ThinkingStart { timestamp: Utc::now() },
            AgentStreamEvent::ThinkingComplete {
                timestamp: Utc::now(),
                thinking: "done".to_string(),
            },
            AgentStreamEvent::ContentStart { timestamp: Utc::now() },
            AgentStreamEvent::ContentComplete {
                timestamp: Utc::now(),
                content: "final".to_string(),
            },
            AgentStreamEvent::ToolCallBegin {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                arguments: "{}".to_string(),
            },
            AgentStreamEvent::ToolCallComplete {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                result: "ok".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::ToolCallError {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                error: "fail".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::ToolCallSkipped {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                reason: "not allowed".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::IterationComplete {
                timestamp: Utc::now(),
                iteration: 1,
                tool_calls: vec![],
                final_answer: Some("done".to_string()),
            },
            AgentStreamEvent::PluginEvent {
                timestamp: Utc::now(),
                name: "custom".to_string(),
                data: {
                    let mut m = Map::new();
                    m.insert("k".to_string(), json!("v"));
                    m
                },
            },
            AgentStreamEvent::MaxIterationsReached {
                timestamp: Utc::now(),
                current_iteration: 10,
                max_iterations: 10,
            },
            AgentStreamEvent::IterationContinued {
                timestamp: Utc::now(),
                from_iteration: 11,
            },
        ];

        let _ = &ctx; // suppress unused warning
        for event in events {
            if !LoggerPlugin::should_log(&event) {
                continue;
            }
            let entry = LoggerPlugin::create_log_entry(&event, "run-1");
            let line = entry.to_json_line();
            let path = plugin.log_path(&event, "run-1");
            // Verify log_path is deterministic
            let path2 = plugin.log_path(&event, "run-1");
            assert_eq!(path, path2);
            // Verify JSON serialization works
            assert!(line.contains("run-1"));
        }
    }

    #[tokio::test]
    async fn test_listen_writes_log_file() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_context();

        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let log_path = temp_dir.path().join("logs/test-run.jsonl");
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("AgentStart"));
    }

    #[tokio::test]
    async fn test_listen_writes_plugin_event_log() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_context();

        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        plugin.listen(&event, &ctx).await;

        let log_path = temp_dir.path().join("logs/my_plugin/test-run.jsonl");
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("PluginEvent"));
        assert!(content.contains("my_plugin"));
    }
}
