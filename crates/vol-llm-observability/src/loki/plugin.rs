//! LokiPlugin - Sends agent events to Loki via HTTP.
//!
//! Implements `AgentPlugin` to intercept agent run events and forward them
//! to Loki. Runs alongside `LoggerPlugin` (dual-write: local JSONL + Loki).
//!
//! # Labels
//!
//! Each entry is sent to Loki with labels:
//! - `namespace`: `"agent"` (fixed)
//! - `agent`: From `AgentDef.r#type` (via `RunContext.config.def`)
//! - `agent_id`: From `AgentDef.name` (via `RunContext.config.def`)
//!
//! High-cardinality fields (`run_id`, `session_id`) are placed in the log
//! line content, not as labels, to avoid Loki performance issues.

use std::sync::Arc;

use chrono::Utc;
use serde_json::{json, Value};
use vol_llm_core::stream::AgentStreamEvent;

use crate::loki::client::{LokiEntry, LokiWriter};
use crate::loki::config::LokiConfig;
use crate::loki::labels::LokiLabels;

/// Plugin that sends agent events to Loki.
///
/// Uses a shared `LokiWriter` so that multiple clones of the plugin
/// (as happens with `Arc<dyn AgentPlugin>`) write to the same background task.
///
/// Agent identity (type, id) is derived from `RunContext.config.def` at runtime.
pub struct LokiPlugin {
    writer: Arc<LokiWriter>,
}

impl LokiPlugin {
    /// Create a new LokiPlugin.
    pub fn new(config: LokiConfig) -> Self {
        let writer = LokiWriter::spawn(
            config.url,
            config.batch_size,
            config.flush_interval_ms,
        );
        Self {
            writer: Arc::new(writer),
        }
    }

    /// Whether an event should be sent to Loki.
    /// Skips high-frequency streaming delta events.
    pub fn should_send(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    /// Convert an event to a Loki entry.
    pub fn create_loki_entry(event: &AgentStreamEvent, ctx: &RunContext) -> LokiEntry {
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());
        let run_id = &ctx.run_id;
        let session_id = &ctx.session_id;

        let labels = LokiLabels::new(agent_type, agent_id);
        let mut labels = labels.into_inner();

        // Build the log line as a compact JSON object.
        let event_name = event_name(event);
        let data = event_data(event);

        let mut line_map = serde_json::Map::new();
        line_map.insert("timestamp".to_string(), json!(Utc::now().to_rfc3339()));
        line_map.insert("event".to_string(), json!(&event_name));
        line_map.insert("run_id".to_string(), json!(run_id));
        line_map.insert("session_id".to_string(), json!(session_id));
        line_map.insert("agent_id".to_string(), json!(agent_id));

        // Include model if available from the event.
        if let AgentStreamEvent::LLMCallComplete { model, .. } = event {
            line_map.insert("model".to_string(), json!(model));
            labels.insert("model".to_string(), model.clone());
        }

        // Include tool_name for tool events.
        if let Some(tool_name) = event_tool_name(event) {
            line_map.insert("tool_name".to_string(), json!(tool_name));
        }

        // Merge event data into the line.
        if let Value::Object(obj) = data {
            for (k, v) in obj {
                line_map.insert(k, v);
            }
        }

        let line = json!(line_map).to_string();

        let timestamp_nanos = Utc::now().timestamp_nanos_opt().unwrap_or(0);

        LokiEntry {
            timestamp_nanos,
            line,
            labels,
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
        AgentStreamEvent::ThinkingDelta { .. }
        | AgentStreamEvent::ContentDelta { .. }
        | AgentStreamEvent::ToolCallArgumentDelta { .. } => {
            unreachable!("delta events are filtered by should_send()")
        }
    }
}

fn event_data(event: &AgentStreamEvent) -> Value {
    match event {
        AgentStreamEvent::AgentStart { input, .. } => json!({ "input": input }),
        AgentStreamEvent::AgentComplete { response, .. } => json!({ "response": response }),
        AgentStreamEvent::AgentAborted { reason, .. } => json!({ "reason": reason }),
        AgentStreamEvent::LLMCallStart { iteration, .. } => json!({ "iteration": iteration }),
        AgentStreamEvent::LLMCallComplete { model, usage, .. } => json!({ "model": model, "usage": usage }),
        AgentStreamEvent::LLMCallError { error, .. } => json!({ "error": error }),
        AgentStreamEvent::ThinkingStart { .. } => json!({}),
        AgentStreamEvent::ThinkingComplete { thinking, .. } => json!({ "thinking": thinking }),
        AgentStreamEvent::ContentStart { .. } => json!({}),
        AgentStreamEvent::ContentComplete { content, .. } => json!({ "content": content }),
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
                json!({ "id": &tc.id, "name": &tc.name, "arguments": &tc.arguments, "type": &tc.r#type })
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
        AgentStreamEvent::ThinkingDelta { .. }
        | AgentStreamEvent::ContentDelta { .. }
        | AgentStreamEvent::ToolCallArgumentDelta { .. } => {
            unreachable!("delta events are filtered by should_send()")
        }
    }
}

fn event_tool_name(event: &AgentStreamEvent) -> Option<&str> {
    match event {
        AgentStreamEvent::ToolCallBegin { tool_name, .. }
        | AgentStreamEvent::ToolCallComplete { tool_name, .. }
        | AgentStreamEvent::ToolCallError { tool_name, .. }
        | AgentStreamEvent::ToolCallSkipped { tool_name, .. }
        | AgentStreamEvent::ToolCallArgumentDelta { tool_name, .. } => Some(tool_name),
        _ => None,
    }
}

use async_trait::async_trait;
use vol_llm_agent::react::{AgentPlugin, PluginDecision, RunContext};

#[async_trait]
impl AgentPlugin for LokiPlugin {
    fn id(&self) -> String {
        "loki".to_string()
    }

    fn priority(&self) -> u32 {
        20
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if !Self::should_send(event) {
            return;
        }
        let entry = Self::create_loki_entry(event, ctx);
        self.writer.send(entry).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    fn make_test_context(run_id: &str, session_id: &str, agent_name: &str, agent_type: &str) -> RunContext {
        use vol_llm_agent::agent_def::AgentDef;
        use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
        use vol_llm_context::ContextBuilderBuilder;
        use vol_session::{InMemoryEntryStore, Session};
        use vol_llm_tool::ToolRegistry;

        let def = AgentDef::new(agent_name, "prompt").with_type(agent_type);
        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
        let tools = Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: Some(def),
            llm: Arc::new(DummyLlm),
            tools: tools.clone(),
            session: session.clone(),
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
        };

        let (ctx, _rx) = RunContext::new(
            run_id.to_string(),
            "test input".to_string(),
            session_id.to_string(),
            session,
            tools,
            config,
            20,
        );
        ctx
    }

    struct DummyLlm;
    #[async_trait::async_trait]
    impl vol_llm_core::LLMClient for DummyLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
        fn model(&self) -> &str { "test" }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
        async fn converse(&self, _: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> { unimplemented!() }
        async fn converse_stream(&self, _: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> { unimplemented!() }
    }

    #[tokio::test]
    async fn test_plugin_id() {
        let config = LokiConfig::with_url("http://loki:3100".to_string());
        let plugin = LokiPlugin::new(config);
        assert_eq!(plugin.id(), "loki");
    }

    #[tokio::test]
    async fn test_plugin_priority() {
        let config = LokiConfig::with_url("http://loki:3100".to_string());
        let plugin = LokiPlugin::new(config);
        assert_eq!(plugin.priority(), 20);
    }

    #[test]
    fn test_should_send_skips_delta_events() {
        assert!(!LokiPlugin::should_send(&AgentStreamEvent::ThinkingDelta {
            timestamp: Utc::now(),
            delta: "chunk".to_string(),
        }));
        assert!(!LokiPlugin::should_send(&AgentStreamEvent::ContentDelta {
            timestamp: Utc::now(),
            delta: "partial".to_string(),
        }));
        assert!(!LokiPlugin::should_send(&AgentStreamEvent::ToolCallArgumentDelta {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            delta: "arg".to_string(),
        }));
        assert!(LokiPlugin::should_send(&AgentStreamEvent::ThinkingStart {
            timestamp: Utc::now(),
        }));
        assert!(LokiPlugin::should_send(&AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_loki_entry_tool_call() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.line.contains("bash"));
        assert!(entry.line.contains("run-1"));
        assert!(entry.line.contains("sess-1"));
        assert!(entry.line.contains("agent-001"));
        assert!(entry.line.contains("ToolCallBegin"));
    }

    #[test]
    fn test_loki_entry_llm_complete_includes_model_label() {
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: Utc::now(),
            model: "qwen3.5-plus".to_string(),
            usage: None,
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.labels.contains_key("model"));
        assert_eq!(entry.labels["model"], "qwen3.5-plus");
    }

    #[test]
    fn test_loki_entry_plugin_event() {
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.line.contains("PluginEvent"));
        assert!(entry.line.contains("my_plugin"));
    }

    #[test]
    fn test_loki_entry_fallback_no_agent_def() {
        use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
        use vol_llm_context::ContextBuilderBuilder;
        use vol_session::{InMemoryEntryStore, Session};
        use vol_llm_tool::ToolRegistry;

        // Build context with def = None
        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
        let tools = Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: None,
            llm: Arc::new(DummyLlm),
            tools: tools.clone(),
            session: session.clone(),
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
        };

        let (ctx, _rx) = RunContext::new(
            "run-x".to_string(),
            "test".to_string(),
            "sess-x".to_string(),
            session,
            tools,
            config,
            20,
        );

        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.labels.contains_key("agent"));
        assert_eq!(entry.labels["agent"], "unknown");
        assert_eq!(entry.labels["agent_id"], "unknown");
    }
}
