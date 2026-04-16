//! ObservabilityPlugin implementation.

use super::run_log::{LogEntry, RunLogLogger};
use crate::react::plugin::{AgentPlugin, PluginDecision, PluginId};
use crate::react::plugin::PluginContext;
use crate::AgentStreamEvent;
use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

pub struct ObservabilityPlugin {
    logger: Arc<RunLogLogger>,
}

impl ObservabilityPlugin {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        let logger = Arc::new(RunLogLogger::new(agent_id, log_base_path));
        Self { logger }
    }

    fn create_log_entry(&self, event: &AgentStreamEvent, ctx: &PluginContext) -> LogEntry {
        // Extract event type name and data separately for structured logging
        let (event_name, data) = match event {
            AgentStreamEvent::AgentStart { input, .. } => ("AgentStart", json!({ "input": input })),
            AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                ("ThinkingComplete", json!({ "thinking": thinking }))
            }
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => (
                "ToolCallBegin",
                json!({
                    "tool_call_id": tool_call_id,
                    "tool_name": tool_name,
                    "arguments": arguments
                }),
            ),
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
                ..
            } => (
                "ToolCallComplete",
                json!({
                    "tool_call_id": tool_call_id,
                    "tool_name": tool_name,
                    "result": result
                }),
            ),
            AgentStreamEvent::IterationComplete {
                iteration,
                tool_calls,
                final_answer,
                ..
            } => (
                "IterationComplete",
                json!({
                    "iteration": iteration,
                    "tool_calls": tool_calls,
                    "final_answer": final_answer,
                }),
            ),
            AgentStreamEvent::AgentComplete { .. } => ("AgentComplete", json!({})),
            AgentStreamEvent::AgentAborted { reason, .. } => {
                ("AgentAborted", json!({ "reason": reason }))
            }
            AgentStreamEvent::PluginEvent { name, data, .. } => {
                ("PluginEvent", json!({ "name": name, "data": data }))
            }
            // New lifecycle events (emit/observe only, no special data extraction needed)
            AgentStreamEvent::LLMCallStart { .. } => ("LLMCallStart", json!({})),
            AgentStreamEvent::LLMCallComplete { .. } => ("LLMCallComplete", json!({})),
            AgentStreamEvent::LLMCallError { .. } => ("LLMCallError", json!({})),
            AgentStreamEvent::ThinkingStart { .. } => ("ThinkingStart", json!({})),
            AgentStreamEvent::ThinkingDelta { .. } => ("ThinkingDelta", json!({})),
            AgentStreamEvent::ContentStart { .. } => ("ContentStart", json!({})),
            AgentStreamEvent::ContentDelta { .. } => ("ContentDelta", json!({})),
            AgentStreamEvent::ContentComplete { .. } => ("ContentComplete", json!({})),
            AgentStreamEvent::ToolCallError { .. } => ("ToolCallError", json!({})),
            AgentStreamEvent::ToolCallSkipped { .. } => ("ToolCallSkipped", json!({})),
            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                ("MaxIterationsReached", json!({ "current_iteration": current_iteration, "max_iterations": max_iterations }))
            }
            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                ("IterationContinued", json!({ "from_iteration": from_iteration }))
            }
        };

        LogEntry {
            timestamp: Utc::now(),
            run_id: ctx.run_id.clone(),
            agent_id: self.logger.agent_id().to_string(),
            event: event_name.to_string(),
            data,
        }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId {
        "observability".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        let entry = self.create_log_entry(event, ctx);
        // 只写入 run log
        self.logger.log(&entry, &ctx.run_id).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react::plugin::PluginContext;
    use crate::react::run_context::RunContext;
    use crate::react::{plugin_context_from_run_ctx, AgentConfig};
    use crate::session::{InMemoryMessageStore, InMemorySessionStore, Session};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_plugin_context() -> PluginContext {
        let (ctx, _rx, _approval_rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        );
        plugin_context_from_run_ctx(&ctx)
    }

    #[tokio::test]
    async fn test_observability_plugin_logs_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin =
            ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_plugin_context();

        let event = AgentStreamEvent::agent_start("test".to_string());

        plugin.listen(&event, &ctx).await;

        // Wait for async file write to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify log file was created
        let agent_path = temp_dir.path().join("test_agent");
        let runs_path = agent_path.join("runs");
        assert!(runs_path.exists());

        // Check run log contains expected entry
        let run_log_path = runs_path.join("test-run.jsonl");
        let content = std::fs::read_to_string(&run_log_path).unwrap();
        assert!(content.contains("AgentStart"));
    }

    #[tokio::test]
    async fn test_observability_plugin_logs_all_event_types() {
        let temp_dir = TempDir::new().unwrap();
        let plugin =
            ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_plugin_context();

        // Test all event types
        let events = vec![
            AgentStreamEvent::agent_start("test input".to_string()),
            AgentStreamEvent::thinking_complete("thought".to_string()),
            AgentStreamEvent::tool_call_begin(
                "call_123".to_string(),
                "test_tool".to_string(),
                "{\"key\": \"value\"}".to_string(),
            ),
            AgentStreamEvent::tool_call_complete(
                "call_123".to_string(),
                "test_tool".to_string(),
                "tool result".to_string(),
                None,
            ),
            AgentStreamEvent::iteration_complete(1, vec![], Some("answer".to_string())),
            AgentStreamEvent::agent_complete(),
            AgentStreamEvent::agent_aborted("test abort reason".to_string()),
            AgentStreamEvent::plugin_event("test_plugin_event".to_string(), serde_json::Map::new()),
        ];

        for event in events {
            plugin.listen(&event, &ctx).await;
        }

        // Wait for async file writes to complete
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify logs were created
        let agent_path = temp_dir.path().join("test_agent");
        assert!(agent_path.exists());
        assert!(agent_path.join("runs").exists());

        // Verify run logs contain ALL event types
        let run_log_path = agent_path.join("runs").join("test-run.jsonl");
        let run_content = std::fs::read_to_string(&run_log_path).unwrap();

        // All 8 events should be in run logs
        assert!(run_content.contains(r#""event":"AgentStart""#));
        assert!(run_content.contains(r#""event":"ThinkingComplete""#));
        assert!(run_content.contains(r#""event":"ToolCallBegin""#));
        assert!(run_content.contains(r#""event":"ToolCallComplete""#));
        assert!(run_content.contains(r#""event":"IterationComplete""#));
        assert!(run_content.contains(r#""event":"AgentComplete""#));
        assert!(run_content.contains(r#""event":"AgentAborted""#));
        assert!(run_content.contains(r#""event":"PluginEvent""#));
    }
}
