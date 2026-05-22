//! LokiPlugin - Sends agent events to OTel Collector via tracing macros.
//!
//! Implements `AgentPlugin` to intercept agent run events and forward them
//! as structured logs via `tracing::info!`. The tracing-subscriber stack,
//! extended with opentelemetry-appender-tracing, routes logs to the OTel Collector.
//!
//! This plugin is stateless — it holds no endpoint, writer, or config.
//! It relies on the tracing layer being properly initialized (see vol-monitor tracing_setup).
//!
//! # Structured Fields
//!
//! Each log entry carries:
//! - `namespace`: `"agent"` (fixed)
//! - `session_id`: Session ID from RunContext
//! - `agent_id`: Agent instance name
//! - `agent_type`: Agent type (e.g., "coding", "qa")
//! - `run_id`: Run ID
//! - `model`: Model used for this run
//! - `event`: Full serialized AgentStreamEvent variant content

use async_trait::async_trait;
use serde_json::{json, Value};
use vol_llm_agent::react::{AgentPlugin, PluginDecision, RunContext};
use vol_llm_core::stream::AgentStreamEvent;

/// Plugin that sends agent events to OTel via tracing macros.
///
/// Stateless — no fields, no config. Clone is trivial.
pub struct LokiPlugin;

impl LokiPlugin {
    /// Create a new LokiPlugin.
    pub fn new() -> Self {
        Self
    }

    /// Whether an event should be sent to OTel.
    /// Skips high-frequency streaming delta events.
    pub fn should_send(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    /// Convert an event to a flat JSON object with metadata.
    /// Same flattening logic as the previous Loki-based implementation.
    fn create_event_json(event: &AgentStreamEvent, ctx: &RunContext) -> String {
        let def = ctx.config.def.as_ref();
        let _agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());
        let run_id = &ctx.run_id;
        let session_id = &ctx.session_id;

        let line_map = match serde_json::to_value(event) {
            Ok(Value::Object(mut map)) => {
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
                    map.insert("run_id".to_string(), json!(run_id));
                    map.insert("session_id".to_string(), json!(session_id));
                    map.insert("agent_id".to_string(), json!(agent_id));
                    map
                }
            }
            _ => {
                let mut map = serde_json::Map::new();
                map.insert("event".to_string(), json!(event.event_name()));
                map.insert("run_id".to_string(), json!(run_id));
                map.insert("session_id".to_string(), json!(session_id));
                map.insert("agent_id".to_string(), json!(agent_id));
                map
            }
        };

        json!(line_map).to_string()
    }
}

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
        let event_json = Self::create_event_json(event, ctx);
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());

        tracing::info!(
            session_id = ctx.session_id,
            agent_id = agent_id,
            agent_type = agent_type,
            agent_model = ctx.model,
            run_id = ctx.run_id,
            "{}", event_json
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::Map;
    use vol_llm_agent::agent_def::AgentDef;
    use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
    use vol_llm_context::ContextBuilderBuilder;
    use vol_llm_core::LLMClient;
    use vol_llm_core::{ConversationRequest, ConversationResponse, LLMProvider, Result as LlmResult, StreamReceiver};
    use vol_session::{InMemoryEntryStore, Session};
    use vol_llm_tool::ToolRegistry;

    struct DummyLlm;
    #[async_trait::async_trait]
    impl LLMClient for DummyLlm {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "test" }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
        async fn converse(&self, _: ConversationRequest) -> LlmResult<ConversationResponse> { unimplemented!() }
        async fn converse_stream(&self, _: ConversationRequest) -> LlmResult<StreamReceiver> { unimplemented!() }
    }

    fn make_test_context(run_id: &str, session_id: &str, agent_name: &str, agent_type: &str) -> RunContext {
        let def = AgentDef::new(agent_name, "prompt").with_type(agent_type);
        let session = std::sync::Arc::new(Session::new(std::sync::Arc::new(InMemoryEntryStore::new())));
        let tools = std::sync::Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: Some(def),
            llm: std::sync::Arc::new(DummyLlm),
            tools: tools.clone(),
            session: session.clone(),
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
            mcp_manager: None,
        };

        let (ctx, _rx) = RunContext::new(
            run_id.to_string(),
            "test input".to_string(),
            session_id.to_string(),
            session,
            tools,
            config,
            20,
            "test-model".to_string(),
        );
        ctx
    }

    #[tokio::test]
    async fn test_plugin_id() {
        let plugin = LokiPlugin::new();
        assert_eq!(plugin.id(), "loki");
    }

    #[tokio::test]
    async fn test_plugin_priority() {
        let plugin = LokiPlugin::new();
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
    fn test_event_json_tool_call_has_all_fields() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let json_str = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["event"], "ToolCallBegin");
        assert_eq!(parsed["tool_call_id"], "c1");
        assert_eq!(parsed["tool_name"], "bash");
        assert_eq!(parsed["arguments"], "{}");
        assert_eq!(parsed["run_id"], "run-1");
        assert_eq!(parsed["session_id"], "sess-1");
        assert_eq!(parsed["agent_id"], "agent-001");
    }

    #[test]
    fn test_event_json_llm_complete_includes_model() {
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: Utc::now(),
            model: "qwen3.5-plus".to_string(),
            usage: None,
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let _json_str = LokiPlugin::create_event_json(&event, &ctx);
        // Model is now from RunContext, not the event.
    }

    #[test]
    fn test_event_json_plugin_event() {
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let json_str = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["event"], "PluginEvent");
        assert_eq!(parsed["name"], "my_plugin");
    }

    #[test]
    fn test_event_json_fallback_no_agent_def() {
        let session = std::sync::Arc::new(Session::new(std::sync::Arc::new(InMemoryEntryStore::new())));
        let tools = std::sync::Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: None,
            llm: std::sync::Arc::new(DummyLlm),
            tools: tools.clone(),
            session: session.clone(),
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
            mcp_manager: None,
        };

        let (ctx, _rx) = RunContext::new(
            "run-x".to_string(),
            "test".to_string(),
            "sess-x".to_string(),
            session,
            tools,
            config,
            20,
            "test-model".to_string(),
        );

        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        let json_str = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["agent_id"], "unknown");
    }

    #[test]
    fn test_event_json_llm_call_start_includes_messages() {
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
        let json_str = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json_str).unwrap();
        assert!(parsed.get("messages").is_some());
        assert!(parsed["messages"].as_array().unwrap().len() > 0);
    }
}
