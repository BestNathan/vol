//! LoggingPlugin — Emits structured JSON agent events to stdout via tracing.

use async_trait::async_trait;
use serde_json::json;
use vol_llm_agent::react::{AgentPlugin, PluginDecision, RunContext};
use vol_llm_core::AgentStreamEvent;

/// Plugin that emits a flattened structured JSON line per agent event via tracing.
///
/// Stateless — no fields, no config. Clone is trivial.
pub struct LoggingPlugin;

impl Default for LoggingPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl LoggingPlugin {
    /// Create a new LoggingPlugin.
    pub fn new() -> Self {
        Self
    }

    /// Whether an event should be logged.
    /// Skips high-frequency streaming delta events.
    pub fn should_send(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    /// Build a flattened JSON object with run metadata + event-specific fields.
    fn create_event_json(event: &AgentStreamEvent, ctx: &RunContext) -> String {
        let def = ctx.config.def.as_ref();
        let agent_id = def.map(|d| d.name.as_str()).unwrap_or("unknown");
        let agent_type = def.map(|d| d.r#type.as_str()).unwrap_or("unknown");

        let mut map = serde_json::Map::new();
        map.insert("event".to_string(), json!(event.event_name()));
        map.insert("run_id".to_string(), json!(&ctx.run_id));
        map.insert("session_id".to_string(), json!(&ctx.session_id));
        map.insert("agent_id".to_string(), json!(agent_id));
        map.insert("agent_type".to_string(), json!(agent_type));
        map.insert("model".to_string(), json!(&ctx.model));

        use AgentStreamEvent::*;
        match event {
            AgentStart { input, .. } => {
                map.insert("input".to_string(), json!(input));
            }
            AgentComplete { response, .. } => {
                map.insert("response".to_string(), json!(response));
            }
            AgentAborted { reason, .. } => {
                map.insert("reason".to_string(), json!(reason));
            }
            LLMCallStart { iteration, .. } => {
                map.insert("iteration".to_string(), json!(iteration));
            }
            LLMCallComplete { model, usage, .. } => {
                map.insert("model".to_string(), json!(model));
                if let Some(u) = usage {
                    map.insert("prompt_tokens".to_string(), json!(u.prompt_tokens));
                    map.insert("completion_tokens".to_string(), json!(u.completion_tokens));
                    map.insert("total_tokens".to_string(), json!(u.total_tokens));
                    if let Some(cached) = u.cached_tokens {
                        map.insert("cached_tokens".to_string(), json!(cached));
                    }
                }
            }
            LLMCallError { error, .. } => {
                map.insert("error".to_string(), json!(error));
            }
            ThinkingStart { .. } => {}
            ThinkingComplete { thinking, .. } => {
                map.insert("thinking".to_string(), json!(thinking));
            }
            ContentStart { .. } => {}
            ContentComplete { content, .. } => {
                map.insert("content".to_string(), json!(content));
            }
            ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("arguments".to_string(), json!(arguments));
            }
            ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
                duration_ms,
                ..
            } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("result".to_string(), json!(result));
                if let Some(d) = duration_ms {
                    map.insert("duration_ms".to_string(), json!(d));
                }
            }
            ToolCallError {
                tool_call_id,
                tool_name,
                error,
                duration_ms,
                ..
            } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("error".to_string(), json!(error));
                if let Some(d) = duration_ms {
                    map.insert("duration_ms".to_string(), json!(d));
                }
            }
            ToolCallSkipped {
                tool_call_id,
                tool_name,
                reason,
                duration_ms,
                ..
            } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("reason".to_string(), json!(reason));
                if let Some(d) = duration_ms {
                    map.insert("duration_ms".to_string(), json!(d));
                }
            }
            IterationComplete {
                iteration,
                tool_calls,
                final_answer,
                ..
            } => {
                map.insert("iteration".to_string(), json!(iteration));
                map.insert("tool_calls".to_string(), json!(tool_calls));
                if let Some(fa) = final_answer {
                    map.insert("final_answer".to_string(), json!(fa));
                }
            }
            PluginEvent { name, data, .. } => {
                map.insert("plugin_name".to_string(), json!(name));
                for (k, v) in data {
                    map.insert(k.clone(), v.clone());
                }
            }
            MaxIterationsReached {
                current_iteration,
                max_iterations,
                ..
            } => {
                map.insert("current_iteration".to_string(), json!(current_iteration));
                map.insert("max_iterations".to_string(), json!(max_iterations));
            }
            IterationContinued { from_iteration, .. } => {
                map.insert("from_iteration".to_string(), json!(from_iteration));
            }
            ThinkingDelta { .. } | ContentDelta { .. } | ToolCallArgumentDelta { .. } => {}
        }

        json!(map).to_string()
    }
}

#[async_trait]
impl AgentPlugin for LoggingPlugin {
    fn id(&self) -> String {
        "logging".to_string()
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
        tracing::info!("{}", event_json);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::react::AgentConfig;

    #[test]
    fn test_should_send_filters_delta() {
        assert!(!LoggingPlugin::should_send(
            &AgentStreamEvent::thinking_delta("x".to_string())
        ));
        assert!(!LoggingPlugin::should_send(
            &AgentStreamEvent::content_delta("x".to_string())
        ));
        assert!(!LoggingPlugin::should_send(
            &AgentStreamEvent::tool_call_argument_delta(
                "id".to_string(),
                "tool".to_string(),
                "d".to_string()
            )
        ));

        assert!(LoggingPlugin::should_send(&AgentStreamEvent::agent_start(
            "hi".to_string()
        )));
        assert!(LoggingPlugin::should_send(
            &AgentStreamEvent::thinking_start()
        ));
    }

    #[test]
    fn test_create_event_json_has_metadata() {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            AgentConfig::default().into(),
        );

        let event = AgentStreamEvent::agent_start("hello world".to_string());
        let json_str = LoggingPlugin::create_event_json(&event, &ctx);
        let value: serde_json::Value = serde_json::from_str(&json_str).unwrap();

        assert_eq!(value["event"], "AgentStart");
        assert!(value.get("run_id").is_some());
        assert!(value.get("session_id").is_some());
        assert!(value.get("agent_id").is_some());
        assert_eq!(value["input"], "hello world");
    }

    #[test]
    fn test_id_and_priority() {
        let plugin = LoggingPlugin::new();
        assert_eq!(plugin.id(), "logging");
        assert_eq!(plugin.priority(), 20);
    }
}
