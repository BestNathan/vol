//! ObservabilityPlugin - Full AgentPlugin implementation.
//!
//! Intercepts agent events to collect metrics and create tracing spans,
//! and listens to events to write structured run logs (JSONL).

use std::sync::Arc;
use tokio::sync::Mutex;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Map, Value};
use vol_llm_core::plugin::{AgentPlugin, PluginContext, PluginDecision};
use vol_llm_core::stream::AgentStreamEvent;

use crate::config::ObservabilityConfig;
use crate::metrics::MetricsCollector;
use crate::run_log::{LogEntry, RunLogLogger};

/// Observability plugin that collects metrics, creates tracing spans,
/// and writes structured run logs.
pub struct ObservabilityPlugin {
    config: ObservabilityConfig,
    logger: Arc<RunLogLogger>,
    metrics: Arc<Mutex<MetricsCollector>>,
}

impl ObservabilityPlugin {
    /// Create a new ObservabilityPlugin with default configuration.
    pub fn new(agent_id: String, log_base_path: std::path::PathBuf) -> Self {
        let config = ObservabilityConfig::default();
        let logger = Arc::new(RunLogLogger::new(agent_id.clone(), log_base_path));
        let metrics = Arc::new(Mutex::new(MetricsCollector::new(
            String::new(), // run_id is set per-run
            agent_id,
        )));
        Self { config, logger, metrics }
    }

    /// Create a new ObservabilityPlugin with a specific configuration.
    pub fn with_config(agent_id: String, config: ObservabilityConfig) -> Self {
        let logger = Arc::new(RunLogLogger::new(
            agent_id.clone(),
            config.log_base_path.clone(),
        ));
        let metrics = Arc::new(Mutex::new(MetricsCollector::new(
            String::new(),
            agent_id,
        )));
        Self { config, logger, metrics }
    }

    /// Create a LogEntry from an AgentStreamEvent.
    fn create_log_entry(event: &AgentStreamEvent, run_id: &str, agent_id: &str) -> LogEntry {
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
                let msg_count = messages.len();
                json!({ "iteration": iteration, "message_count": msg_count })
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
            AgentStreamEvent::ThinkingDelta { delta, .. } => {
                json!({ "delta": delta })
            }
            AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                json!({ "thinking": thinking })
            }
            AgentStreamEvent::ContentStart { .. } => {
                json!({})
            }
            AgentStreamEvent::ContentDelta { delta, .. } => {
                json!({ "delta": delta })
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
                let mut map = Map::new();
                map.insert("name".to_string(), Value::String(name.clone()));
                for (k, v) in data {
                    map.insert(k.clone(), v.clone());
                }
                Value::Object(map)
            }
        };

        let event_name = event_name(event);
        LogEntry {
            timestamp: Utc::now(),
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            event: event_name,
            data,
        }
    }

    /// Update metrics collector with a new run_id (called per-run).
    pub fn set_run_id(&self, _run_id: String) {
        // Note: In a real scenario, we'd create a new MetricsCollector per run.
        // For simplicity, we use a lock to update. In production, consider
        // using a per-run metrics instance.
        // This method is provided for external re-initialization if needed.
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
        AgentStreamEvent::ThinkingDelta { .. } => "ThinkingDelta".to_string(),
        AgentStreamEvent::ThinkingComplete { .. } => "ThinkingComplete".to_string(),
        AgentStreamEvent::ContentStart { .. } => "ContentStart".to_string(),
        AgentStreamEvent::ContentDelta { .. } => "ContentDelta".to_string(),
        AgentStreamEvent::ContentComplete { .. } => "ContentComplete".to_string(),
        AgentStreamEvent::ToolCallBegin { .. } => "ToolCallBegin".to_string(),
        AgentStreamEvent::ToolCallComplete { .. } => "ToolCallComplete".to_string(),
        AgentStreamEvent::ToolCallError { .. } => "ToolCallError".to_string(),
        AgentStreamEvent::ToolCallSkipped { .. } => "ToolCallSkipped".to_string(),
        AgentStreamEvent::IterationComplete { .. } => "IterationComplete".to_string(),
        AgentStreamEvent::PluginEvent { .. } => "PluginEvent".to_string(),
    }
}

#[async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> String {
        "observability".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    /// Interceptor hook - records metrics and creates tracing spans.
    ///
    /// This is async per the trait definition but kept fast:
    /// - Records appropriate metrics based on event type
    /// - Creates tracing spans for LLMCallStart and ToolCallBegin
    async fn intercept(&self, event: &AgentStreamEvent, ctx: &PluginContext) -> PluginDecision {
        // Record metrics if enabled
        if self.config.enable_metrics {
            let mut metrics = self.metrics.lock().await;
            match event {
                AgentStreamEvent::LLMCallStart { .. } => {
                    metrics.record_llm_call_start();
                }
                AgentStreamEvent::ThinkingStart { .. } => {
                    metrics.record_thinking_start();
                }
                AgentStreamEvent::ContentStart { .. } => {
                    metrics.record_content_start();
                }
                AgentStreamEvent::LLMCallComplete { .. } => {
                    metrics.record_llm_call_complete();
                }
                AgentStreamEvent::ToolCallBegin { tool_call_id, .. } => {
                    metrics.record_tool_call_begin(tool_call_id.clone());
                }
                AgentStreamEvent::ToolCallComplete { tool_call_id, .. } => {
                    metrics.record_tool_call_complete(tool_call_id.clone());
                }
                _ => {}
            }
        }

        // Create tracing spans if enabled
        if self.config.enable_tracing {
            match event {
                AgentStreamEvent::LLMCallStart { iteration, .. } => {
                    let _span =
                        crate::tracing::llm_call_span(&ctx.run_id, &self.logger.agent_id(), *iteration)
                            .entered();
                }
                AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, .. } => {
                    let _span = crate::tracing::tool_call_span(
                        &ctx.run_id,
                        &self.logger.agent_id(),
                        tool_name,
                        tool_call_id,
                    )
                    .entered();
                }
                AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, .. } => {
                    let _span = crate::tracing::tool_call_span_with_result(
                        &ctx.run_id,
                        &self.logger.agent_id(),
                        tool_name,
                        tool_call_id,
                        true,
                    )
                    .entered();
                }
                AgentStreamEvent::ToolCallError { tool_call_id, tool_name, .. } => {
                    let _span = crate::tracing::tool_call_span_with_result(
                        &ctx.run_id,
                        &self.logger.agent_id(),
                        tool_name,
                        tool_call_id,
                        false,
                    )
                    .entered();
                }
                _ => {}
            }
        }

        PluginDecision::Continue
    }

    /// Listener hook - writes run log and outputs metrics summary on completion.
    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        // Write run log if enabled
        if self.config.enable_run_log {
            let entry = Self::create_log_entry(event, &ctx.run_id, &self.logger.agent_id());
            self.logger.log(&entry, &ctx.run_id).await;
        }

        // On completion, output metrics summary
        if self.config.enable_metrics {
            match event {
                AgentStreamEvent::AgentComplete { .. } | AgentStreamEvent::AgentAborted { .. } => {
                    let metrics = self.metrics.lock().await;
                    metrics.log_summary();
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::RwLock;
    use vol_llm_core::stream::AgentStreamEvent;
    use vol_llm_core::PluginContext;

    fn create_test_plugin_context() -> PluginContext {
        PluginContext {
            run_id: "test-run".to_string(),
            user_input: "test input".to_string(),
            session_id: "session-1".to_string(),
            messages: Arc::new(RwLock::new(Vec::new())),
            all_tool_calls: Arc::new(RwLock::new(Vec::new())),
            current_tool_calls: Arc::new(RwLock::new(Vec::new())),
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    fn create_test_plugin(temp_dir: &TempDir) -> ObservabilityPlugin {
        ObservabilityPlugin::new(
            "test_agent".to_string(),
            temp_dir.path().join("logs/agents"),
        )
    }

    // === Test plugin id and priority ===

    #[test]
    fn test_plugin_id() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        assert_eq!(plugin.id(), "observability");
    }

    #[test]
    fn test_plugin_priority() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        assert_eq!(plugin.priority(), 10);
    }

    // === Test create_log_entry for all 18 event variants ===

    #[test]
    fn test_log_entry_agent_start() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::AgentStart {
            timestamp: chrono::Utc::now(),
            input: "hello".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.run_id, "run-1");
        assert_eq!(entry.agent_id, "agent-1");
        assert_eq!(entry.event, "AgentStart");
        assert_eq!(entry.data.get("input").unwrap().as_str().unwrap(), "hello");
    }

    #[test]
    fn test_log_entry_agent_complete() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::AgentComplete {
            timestamp: chrono::Utc::now(),
            response: None,
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "AgentComplete");
    }

    #[test]
    fn test_log_entry_agent_aborted() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::AgentAborted {
            timestamp: chrono::Utc::now(),
            reason: "max iterations".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "AgentAborted");
        assert_eq!(entry.data.get("reason").unwrap().as_str().unwrap(), "max iterations");
    }

    #[test]
    fn test_log_entry_llm_call_start() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::LLMCallStart {
            timestamp: chrono::Utc::now(),
            iteration: 2,
            messages: vec![],
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "LLMCallStart");
        assert_eq!(entry.data.get("iteration").unwrap().as_u64().unwrap(), 2);
    }

    #[test]
    fn test_log_entry_llm_call_complete() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: chrono::Utc::now(),
            model: "qwen3.5-plus".to_string(),
            usage: None,
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "LLMCallComplete");
        assert_eq!(entry.data.get("model").unwrap().as_str().unwrap(), "qwen3.5-plus");
    }

    #[test]
    fn test_log_entry_llm_call_error() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::LLMCallError {
            timestamp: chrono::Utc::now(),
            error: "timeout".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "LLMCallError");
        assert_eq!(entry.data.get("error").unwrap().as_str().unwrap(), "timeout");
    }

    #[test]
    fn test_log_entry_thinking_events() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);

        let event = AgentStreamEvent::ThinkingStart {
            timestamp: chrono::Utc::now(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ThinkingStart");

        let event = AgentStreamEvent::ThinkingDelta {
            timestamp: chrono::Utc::now(),
            delta: "thinking...".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ThinkingDelta");
        assert_eq!(entry.data.get("delta").unwrap().as_str().unwrap(), "thinking...");

        let event = AgentStreamEvent::ThinkingComplete {
            timestamp: chrono::Utc::now(),
            thinking: "done".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ThinkingComplete");
        assert_eq!(entry.data.get("thinking").unwrap().as_str().unwrap(), "done");
    }

    #[test]
    fn test_log_entry_content_events() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);

        let event = AgentStreamEvent::ContentStart {
            timestamp: chrono::Utc::now(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ContentStart");

        let event = AgentStreamEvent::ContentDelta {
            timestamp: chrono::Utc::now(),
            delta: "partial".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ContentDelta");

        let event = AgentStreamEvent::ContentComplete {
            timestamp: chrono::Utc::now(),
            content: "final".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ContentComplete");
        assert_eq!(entry.data.get("content").unwrap().as_str().unwrap(), "final");
    }

    #[test]
    fn test_log_entry_tool_events() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);

        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ToolCallBegin");
        assert_eq!(entry.data.get("tool_name").unwrap().as_str().unwrap(), "bash");

        let event = AgentStreamEvent::ToolCallComplete {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            result: "ok".to_string(),
            duration_ms: Some(150),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ToolCallComplete");
        assert_eq!(entry.data.get("result").unwrap().as_str().unwrap(), "ok");

        let event = AgentStreamEvent::ToolCallError {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            error: "failed".to_string(),
            duration_ms: Some(50),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ToolCallError");
        assert_eq!(entry.data.get("error").unwrap().as_str().unwrap(), "failed");

        let event = AgentStreamEvent::ToolCallSkipped {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_1".to_string(),
            tool_name: "bash".to_string(),
            reason: "not allowed".to_string(),
            duration_ms: Some(10),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "ToolCallSkipped");
        assert_eq!(entry.data.get("reason").unwrap().as_str().unwrap(), "not allowed");
    }

    #[test]
    fn test_log_entry_iteration_complete() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::IterationComplete {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            tool_calls: vec![],
            final_answer: Some("done".to_string()),
        };
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "IterationComplete");
        assert_eq!(entry.data.get("iteration").unwrap().as_u64().unwrap(), 1);
    }

    #[test]
    fn test_log_entry_plugin_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("custom".to_string(), data);
        let entry = ObservabilityPlugin::create_log_entry(&event, "run-1", "agent-1");
        assert_eq!(entry.event, "PluginEvent");
        assert_eq!(entry.data.get("name").unwrap().as_str().unwrap(), "custom");
        assert_eq!(entry.data.get("key").unwrap().as_str().unwrap(), "value");
    }

    // === Test plugin intercept records metrics ===

    #[tokio::test]
    async fn test_intercept_records_llm_call_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_plugin_context();

        plugin
            .intercept(&AgentStreamEvent::LLMCallStart {
                timestamp: chrono::Utc::now(),
                iteration: 0,
                messages: vec![],
            }, &ctx)
            .await;
        plugin
            .intercept(&AgentStreamEvent::ContentStart {
                timestamp: chrono::Utc::now(),
            }, &ctx)
            .await;

        let metrics = plugin.metrics.lock().await;
        assert_eq!(metrics.summarize().llm_call_count, 0); // not complete yet
        assert!(metrics.summarize().avg_ttft_ms.is_some());
    }

    #[tokio::test]
    async fn test_intercept_records_tool_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_plugin_context();

        plugin
            .intercept(
                &AgentStreamEvent::ToolCallBegin {
                    timestamp: chrono::Utc::now(),
                    tool_call_id: "c1".to_string(),
                    tool_name: "bash".to_string(),
                    arguments: "{}".to_string(),
                },
                &ctx,
            )
            .await;

        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        plugin
            .intercept(
                &AgentStreamEvent::ToolCallComplete {
                    timestamp: chrono::Utc::now(),
                    tool_call_id: "c1".to_string(),
                    tool_name: "bash".to_string(),
                    result: "ok".to_string(),
                    duration_ms: Some(10),
                },
                &ctx,
            )
            .await;

        let metrics = plugin.metrics.lock().await;
        assert_eq!(metrics.summarize().tool_call_count, 1);
        assert!(metrics.summarize().avg_tool_latency_ms.is_some());
    }

    // === Test plugin listen writes logs ===

    #[tokio::test]
    async fn test_listen_writes_log() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_plugin_context();

        let event = AgentStreamEvent::AgentStart {
            timestamp: chrono::Utc::now(),
            input: "hello".to_string(),
        };
        plugin.listen(&event, &ctx).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let log_path = temp_dir
            .path()
            .join("logs/agents/test_agent/runs/test-run.jsonl");
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("AgentStart"));
    }

    #[tokio::test]
    async fn test_listen_outputs_metrics_on_complete() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_plugin_context();

        // Record some metrics first
        plugin
            .intercept(&AgentStreamEvent::LLMCallStart {
                timestamp: chrono::Utc::now(),
                iteration: 0,
                messages: vec![],
            }, &ctx)
            .await;
        plugin
            .intercept(&AgentStreamEvent::LLMCallComplete {
                timestamp: chrono::Utc::now(),
                model: "test".to_string(),
                usage: None,
            }, &ctx)
            .await;

        // Then complete
        plugin.listen(&AgentStreamEvent::AgentComplete {
            timestamp: chrono::Utc::now(),
            response: None,
        }, &ctx).await;

        // The metrics should show 1 LLM call
        let metrics = plugin.metrics.lock().await;
        assert_eq!(metrics.summarize().llm_call_count, 1);
    }

    // === Test disabled modes ===

    #[tokio::test]
    async fn test_disabled_run_log() {
        let temp_dir = TempDir::new().unwrap();
        let config = ObservabilityConfig {
            enable_run_log: false,
            enable_metrics: true,
            enable_tracing: true,
            log_base_path: temp_dir.path().join("logs/agents"),
            max_run_logs: 10,
            session_retention_days: 7,
        };
        let plugin = ObservabilityPlugin::with_config("test_agent".to_string(), config);
        let ctx = create_test_plugin_context();

        let event = AgentStreamEvent::AgentStart {
            timestamp: chrono::Utc::now(),
            input: "hello".to_string(),
        };
        plugin.listen(&event, &ctx).await;
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let log_path = temp_dir
            .path()
            .join("logs/agents/test_agent/runs/test-run.jsonl");
        // Log file should NOT exist because run_log is disabled
        assert!(!log_path.exists());
    }

    #[tokio::test]
    async fn test_disabled_metrics() {
        let temp_dir = TempDir::new().unwrap();
        let config = ObservabilityConfig {
            enable_run_log: true,
            enable_metrics: false,
            enable_tracing: true,
            log_base_path: temp_dir.path().join("logs/agents"),
            max_run_logs: 10,
            session_retention_days: 7,
        };
        let plugin = ObservabilityPlugin::with_config("test_agent".to_string(), config);
        let ctx = create_test_plugin_context();

        plugin
            .intercept(&AgentStreamEvent::LLMCallStart {
                timestamp: chrono::Utc::now(),
                iteration: 0,
                messages: vec![],
            }, &ctx)
            .await;
        plugin
            .intercept(&AgentStreamEvent::LLMCallComplete {
                timestamp: chrono::Utc::now(),
                model: "test".to_string(),
                usage: None,
            }, &ctx)
            .await;

        // Metrics should still be 0 because metrics are disabled
        let metrics = plugin.metrics.lock().await;
        assert_eq!(metrics.summarize().llm_call_count, 0);
    }

    #[tokio::test]
    async fn test_all_events_through_listen() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_plugin_context();

        let events = vec![
            AgentStreamEvent::AgentStart {
                timestamp: chrono::Utc::now(),
                input: "hello".to_string(),
            },
            AgentStreamEvent::LLMCallStart {
                timestamp: chrono::Utc::now(),
                iteration: 0,
                messages: vec![],
            },
            AgentStreamEvent::ThinkingStart {
                timestamp: chrono::Utc::now(),
            },
            AgentStreamEvent::ThinkingDelta {
                timestamp: chrono::Utc::now(),
                delta: "thinking...".to_string(),
            },
            AgentStreamEvent::ThinkingComplete {
                timestamp: chrono::Utc::now(),
                thinking: "done".to_string(),
            },
            AgentStreamEvent::ContentStart {
                timestamp: chrono::Utc::now(),
            },
            AgentStreamEvent::ContentDelta {
                timestamp: chrono::Utc::now(),
                delta: "hello world".to_string(),
            },
            AgentStreamEvent::ContentComplete {
                timestamp: chrono::Utc::now(),
                content: "hello world".to_string(),
            },
            AgentStreamEvent::LLMCallComplete {
                timestamp: chrono::Utc::now(),
                model: "test".to_string(),
                usage: None,
            },
            AgentStreamEvent::ToolCallBegin {
                timestamp: chrono::Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                arguments: "{}".to_string(),
            },
            AgentStreamEvent::ToolCallComplete {
                timestamp: chrono::Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                result: "ok".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::IterationComplete {
                timestamp: chrono::Utc::now(),
                iteration: 1,
                tool_calls: vec![],
                final_answer: Some("done".to_string()),
            },
            AgentStreamEvent::AgentComplete {
                timestamp: chrono::Utc::now(),
                response: None,
            },
        ];

        for event in events {
            plugin.listen(&event, &ctx).await;
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let log_path = temp_dir
            .path()
            .join("logs/agents/test_agent/runs/test-run.jsonl");
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        // Should have 13 lines (one per event)
        let line_count = content.lines().count();
        assert_eq!(line_count, 13);
    }
}
