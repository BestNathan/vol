//! ObservabilityPlugin implementation.

use crate::react::plugin::{AgentPlugin, PluginDecision, PluginId};
use crate::react::run_context::RunContext;
use crate::AgentStreamEvent;
use super::logger::{ObservabilityLogger, LogEntry, LogType};
use std::sync::Arc;
use std::path::PathBuf;
use chrono::Utc;
use serde_json::json;

pub struct ObservabilityPlugin {
    logger: Arc<ObservabilityLogger>,
}

impl ObservabilityPlugin {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        let logger = Arc::new(ObservabilityLogger::new(agent_id, log_base_path));
        Self { logger }
    }

    fn create_log_entry(&self, event: &AgentStreamEvent, ctx: &RunContext) -> LogEntry {
        let (event_name, data) = match event {
            AgentStreamEvent::AgentStart { input } => {
                ("AgentStart", json!({"input": input}))
            }
            AgentStreamEvent::ThinkingComplete { thinking } => {
                ("ThinkingComplete", json!({"thinking_length": thinking.len()}))
            }
            AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
                ("ToolCallBegin", json!({"tool_name": tool_name, "arguments": arguments}))
            }
            AgentStreamEvent::ToolCallComplete { tool_name, result } => {
                ("ToolCallComplete", json!({"tool_name": tool_name, "result": result}))
            }
            AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer } => {
                ("IterationComplete", json!({
                    "iteration": iteration,
                    "tool_calls_count": tool_calls.len(),
                    "has_final_answer": final_answer.is_some(),
                }))
            }
            AgentStreamEvent::AgentComplete { response } => {
                ("AgentComplete", json!({
                    "iterations": response.iterations,
                    "tool_calls_count": response.tool_calls.len(),
                }))
            }
            AgentStreamEvent::AgentAborted { reason } => {
                ("AgentAborted", json!({"reason": reason}))
            }
            AgentStreamEvent::PluginEvent { name, data } => {
                ("PluginEvent", json!({"name": name, "data": data}))
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

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        let entry = self.create_log_entry(event, ctx);

        // Determine log type based on event
        let log_type = match event {
            AgentStreamEvent::AgentStart { .. } |
            AgentStreamEvent::AgentComplete { .. } |
            AgentStreamEvent::AgentAborted { .. } |
            AgentStreamEvent::ThinkingComplete { .. } => {
                LogType::Run { run_id: ctx.run_id.clone() }
            }
            _ => {
                // Session logs use session_id + date
                let date = Utc::now().format("%Y%m%d").to_string();
                LogType::Session {
                    session_id: ctx.session_id.clone(),
                    date,
                }
            }
        };

        self.logger.log(entry, log_type).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::react::run_context::RunContext;
    use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};
    use crate::react::AgentConfig;
    use std::sync::Arc;
    use tempfile::TempDir;
    use crate::AgentResponse;

    fn create_test_context() -> RunContext {
        let (ctx, _rx) = RunContext::new(
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
        ctx
    }

    #[tokio::test]
    async fn test_observability_plugin_logs_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_context();

        let event = AgentStreamEvent::AgentStart {
            input: "test".to_string(),
        };

        plugin.listen(&event, &ctx).await;

        // Verify log file was created
        let agent_path = temp_dir.path().join("test_agent");
        let runs_path = agent_path.join("runs");
        assert!(runs_path.exists());

        // Check run log contains expected entry
        let run_log_path = runs_path.join("run_test-run.jsonl");
        let content = std::fs::read_to_string(&run_log_path).unwrap();
        assert!(content.contains("AgentStart"));
    }

    #[tokio::test]
    async fn test_observability_plugin_logs_all_event_types() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_context();

        // Test all event types
        let events = vec![
            AgentStreamEvent::AgentStart { input: "test input".to_string() },
            AgentStreamEvent::ThinkingComplete { thinking: "thought".to_string() },
            AgentStreamEvent::ToolCallBegin { tool_name: "test_tool".to_string(), arguments: "{\"key\": \"value\"}".to_string() },
            AgentStreamEvent::ToolCallComplete { tool_name: "test_tool".to_string(), result: "tool result".to_string() },
            AgentStreamEvent::IterationComplete { iteration: 1, tool_calls: vec![], final_answer: Some("answer".to_string()) },
            AgentStreamEvent::AgentComplete { response: AgentResponse { content: "done".to_string(), reasoning: String::new(), iterations: 1, tool_calls: vec![] } },
            AgentStreamEvent::AgentAborted { reason: "test abort reason".to_string() },
            AgentStreamEvent::PluginEvent { name: "test_plugin_event".to_string(), data: serde_json::Map::new() },
        ];

        for event in events {
            plugin.listen(&event, &ctx).await;
        }

        // Verify logs were created
        let agent_path = temp_dir.path().join("test_agent");
        assert!(agent_path.exists());
        assert!(agent_path.join("sessions").exists());
        assert!(agent_path.join("runs").exists());

        // Verify run logs contain expected content
        let run_log_path = agent_path.join("runs").join("run_test-run.jsonl");
        let run_content = std::fs::read_to_string(&run_log_path).unwrap();
        let run_lines: Vec<&str> = run_content.lines().collect();

        // AgentStart, ThinkingComplete, AgentComplete, AgentAborted go to run logs
        assert!(run_content.contains(r#""event":"AgentStart""#));
        assert!(run_content.contains(r#""event":"ThinkingComplete""#));
        assert!(run_content.contains(r#""event":"AgentComplete""#));
        assert!(run_content.contains(r#""event":"AgentAborted""#));

        // Verify AgentStart data
        let agent_start_line = run_lines.iter().find(|l| l.contains(r#""event":"AgentStart""#)).unwrap();
        assert!(agent_start_line.contains(r#""input":"test input""#));

        // Verify ThinkingComplete data
        let thinking_line = run_lines.iter().find(|l| l.contains(r#""event":"ThinkingComplete""#)).unwrap();
        assert!(thinking_line.contains(r#""thinking_length":7"#)); // "thought".len()

        // Verify AgentComplete data
        let agent_complete_line = run_lines.iter().find(|l| l.contains(r#""event":"AgentComplete""#)).unwrap();
        assert!(agent_complete_line.contains(r#""iterations":1"#));
        assert!(agent_complete_line.contains(r#""tool_calls_count":0"#));

        // Verify AgentAborted data
        let agent_aborted_line = run_lines.iter().find(|l| l.contains(r#""event":"AgentAborted""#)).unwrap();
        assert!(agent_aborted_line.contains(r#""reason":"test abort reason""#));

        // Verify session logs contain expected content
        let session_files: Vec<_> = std::fs::read_dir(agent_path.join("sessions"))
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().starts_with("session_session-1_"))
            .collect();
        assert!(!session_files.is_empty(), "Expected session log file");

        let session_log_path = session_files.first().unwrap().path();
        let session_content = std::fs::read_to_string(&session_log_path).unwrap();

        // ToolCallBegin, ToolCallComplete, IterationComplete, PluginEvent go to session logs
        assert!(session_content.contains(r#""event":"ToolCallBegin""#));
        assert!(session_content.contains(r#""event":"ToolCallComplete""#));
        assert!(session_content.contains(r#""event":"IterationComplete""#));
        assert!(session_content.contains(r#""event":"PluginEvent""#));

        // Verify ToolCallBegin data
        let tool_begin_line = session_content.lines().find(|l| l.contains(r#""event":"ToolCallBegin""#)).unwrap();
        assert!(tool_begin_line.contains(r#""tool_name":"test_tool""#));
        assert!(tool_begin_line.contains(r#""arguments":"{\"key\": \"value\"}""#));

        // Verify ToolCallComplete data
        let tool_complete_line = session_content.lines().find(|l| l.contains(r#""event":"ToolCallComplete""#)).unwrap();
        assert!(tool_complete_line.contains(r#""tool_name":"test_tool""#));
        assert!(tool_complete_line.contains(r#""result":"tool result""#));

        // Verify IterationComplete data
        let iteration_line = session_content.lines().find(|l| l.contains(r#""event":"IterationComplete""#)).unwrap();
        assert!(iteration_line.contains(r#""iteration":1"#));
        assert!(iteration_line.contains(r#""tool_calls_count":0"#));
        assert!(iteration_line.contains(r#""has_final_answer":true"#));

        // Verify PluginEvent data
        let plugin_event_line = session_content.lines().find(|l| l.contains(r#""event":"PluginEvent""#)).unwrap();
        assert!(plugin_event_line.contains(r#""name":"test_plugin_event""#));
    }
}
