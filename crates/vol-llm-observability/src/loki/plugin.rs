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

    /// Convert an event to a Loki entry with full event serialization.
    pub fn create_loki_entry(event: &AgentStreamEvent, ctx: &RunContext) -> LokiEntry {
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());
        let run_id = &ctx.run_id;
        let session_id = &ctx.session_id;

        let labels = LokiLabels::new(agent_type, agent_id);
        let mut labels = labels.into_inner();

        // Extract model for label if available.
        if let AgentStreamEvent::LLMCallComplete { model, .. } = event {
            labels.insert("model".to_string(), model.clone());
        }

        // Serialize the event, then flatten the externally-tagged enum variant
        // into a flat object with an "event" key and metadata fields.
        let line_map = match serde_json::to_value(event) {
            Ok(Value::Object(mut map)) => {
                // Externally-tagged enum: {"VariantName": {fields...}}
                // Flatten: extract variant fields to top level, add "event" key.
                if map.len() == 1 {
                    let (variant, fields) = map.iter().next().unwrap();
                    let mut flat = serde_json::Map::new();
                    flat.insert("event".to_string(), json!(variant));
                    if let Value::Object(fields) = fields {
                        for (k, v) in fields {
                            flat.insert(k.clone(), v.clone());
                        }
                    }
                    flat.insert("run_id".to_string(), json!(run_id));
                    flat.insert("session_id".to_string(), json!(session_id));
                    flat.insert("agent_id".to_string(), json!(agent_id));
                    flat
                } else {
                    // Already flat or unexpected format, just add metadata.
                    map.insert("run_id".to_string(), json!(run_id));
                    map.insert("session_id".to_string(), json!(session_id));
                    map.insert("agent_id".to_string(), json!(agent_id));
                    map
                }
            }
            _ => {
                // Fallback if serialization fails.
                let mut map = serde_json::Map::new();
                map.insert("event".to_string(), json!(event.event_name()));
                map.insert("run_id".to_string(), json!(run_id));
                map.insert("session_id".to_string(), json!(session_id));
                map.insert("agent_id".to_string(), json!(agent_id));
                map
            }
        };

        let line = json!(line_map).to_string();

        let timestamp_nanos = event.timestamp().timestamp_nanos_opt().unwrap_or(0);

        LokiEntry {
            timestamp_nanos,
            line,
            labels,
        }
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
    use chrono::Utc;
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
    fn test_loki_entry_tool_call_has_all_fields() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        // Full event serialization includes all fields.
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        assert_eq!(parsed["event"], "ToolCallBegin");
        assert_eq!(parsed["tool_call_id"], "c1");
        assert_eq!(parsed["tool_name"], "bash");
        assert_eq!(parsed["arguments"], "{}");
        assert_eq!(parsed["run_id"], "run-1");
        assert_eq!(parsed["session_id"], "sess-1");
        assert_eq!(parsed["agent_id"], "agent-001");
        // Event's own timestamp should be present.
        assert!(parsed.get("timestamp").is_some());
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
        // Full event serialization includes model in line too.
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        assert_eq!(parsed["model"], "qwen3.5-plus");
    }

    #[test]
    fn test_loki_entry_plugin_event() {
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        assert_eq!(parsed["event"], "PluginEvent");
        assert_eq!(parsed["name"], "my_plugin");
    }

    #[test]
    fn test_loki_entry_fallback_no_agent_def() {
        use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
        use vol_llm_context::ContextBuilderBuilder;
        use vol_session::{InMemoryEntryStore, Session};
        use vol_llm_tool::ToolRegistry;

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

    #[test]
    fn test_loki_entry_timestamp_uses_event_timestamp() {
        use chrono::TimeZone;
        let fixed_ts = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();
        let event = AgentStreamEvent::AgentStart {
            timestamp: fixed_ts,
            input: "test".to_string(),
        };
        let ctx = make_test_context("r1", "s1", "a1", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        // The LokiEntry timestamp_nanos should match the event's timestamp.
        let expected_nanos = fixed_ts.timestamp_nanos_opt().unwrap();
        assert_eq!(entry.timestamp_nanos, expected_nanos);
    }

    #[test]
    fn test_loki_entry_llm_call_start_includes_messages() {
        use vol_llm_core::{Message, message::{MessageContent, MessageRole}};
        let event = AgentStreamEvent::LLMCallStart {
            timestamp: Utc::now(),
            iteration: 1,
            messages: vec![Message {
                role: MessageRole::User,
                content: Some(MessageContent::Text("hello".to_string())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                thinking: None,
            }],
        };
        let ctx = make_test_context("r1", "s1", "a1", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        // Messages array should be present (previously dropped).
        assert!(parsed.get("messages").is_some());
        assert!(parsed["messages"].as_array().unwrap().len() > 0);
    }
}
