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

    fn make_ctx() -> RunContext {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            AgentConfig::default().into(),
        );
        ctx
    }

    fn parse(event: &AgentStreamEvent, ctx: &RunContext) -> serde_json::Value {
        let json_str = LoggingPlugin::create_event_json(event, ctx);
        serde_json::from_str(&json_str).unwrap()
    }

    #[test]
    fn test_create_event_json_agent_complete() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::agent_complete_with_response(json!({"answer": 42})),
            &ctx,
        );
        assert_eq!(value["event"], "AgentComplete");
        assert_eq!(value["response"]["answer"], 42);
    }

    #[test]
    fn test_create_event_json_agent_aborted() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::agent_aborted("cancelled".to_string()),
            &ctx,
        );
        assert_eq!(value["event"], "AgentAborted");
        assert_eq!(value["reason"], "cancelled");
    }

    #[test]
    fn test_create_event_json_llm_call_start() {
        let ctx = make_ctx();
        let value = parse(&AgentStreamEvent::llm_call_start(3, vec![]), &ctx);
        assert_eq!(value["event"], "LLMCallStart");
        assert_eq!(value["iteration"], 3);
    }

    #[test]
    fn test_create_event_json_llm_call_complete_with_usage() {
        let ctx = make_ctx();
        let usage = vol_llm_core::TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cached_tokens: Some(5),
        };
        let value = parse(
            &AgentStreamEvent::llm_call_complete("gpt-x".to_string(), Some(usage)),
            &ctx,
        );
        assert_eq!(value["event"], "LLMCallComplete");
        assert_eq!(value["model"], "gpt-x");
        assert_eq!(value["prompt_tokens"], 10);
        assert_eq!(value["completion_tokens"], 20);
        assert_eq!(value["total_tokens"], 30);
        assert_eq!(value["cached_tokens"], 5);
    }

    #[test]
    fn test_create_event_json_llm_call_complete_without_usage() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::llm_call_complete("gpt-x".to_string(), None),
            &ctx,
        );
        assert_eq!(value["event"], "LLMCallComplete");
        assert_eq!(value["model"], "gpt-x");
        assert!(value.get("prompt_tokens").is_none());
    }

    #[test]
    fn test_create_event_json_llm_call_error() {
        let ctx = make_ctx();
        let value = parse(&AgentStreamEvent::llm_call_error("boom".to_string()), &ctx);
        assert_eq!(value["event"], "LLMCallError");
        assert_eq!(value["error"], "boom");
    }

    #[test]
    fn test_create_event_json_thinking_and_content() {
        let ctx = make_ctx();

        let value = parse(&AgentStreamEvent::thinking_start(), &ctx);
        assert_eq!(value["event"], "ThinkingStart");

        let value = parse(
            &AgentStreamEvent::thinking_complete("thoughts".to_string()),
            &ctx,
        );
        assert_eq!(value["event"], "ThinkingComplete");
        assert_eq!(value["thinking"], "thoughts");

        let value = parse(&AgentStreamEvent::content_start(), &ctx);
        assert_eq!(value["event"], "ContentStart");

        let value = parse(
            &AgentStreamEvent::content_complete("body".to_string()),
            &ctx,
        );
        assert_eq!(value["event"], "ContentComplete");
        assert_eq!(value["content"], "body");
    }

    #[test]
    fn test_create_event_json_tool_call_begin() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::tool_call_begin(
                "id-1".to_string(),
                "search".to_string(),
                "{\"q\":1}".to_string(),
            ),
            &ctx,
        );
        assert_eq!(value["event"], "ToolCallBegin");
        assert_eq!(value["tool_call_id"], "id-1");
        assert_eq!(value["tool_name"], "search");
        assert_eq!(value["arguments"], "{\"q\":1}");
    }

    #[test]
    fn test_create_event_json_tool_call_complete_with_duration() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::tool_call_complete(
                "id-1".to_string(),
                "search".to_string(),
                "result-text".to_string(),
                Some(150),
            ),
            &ctx,
        );
        assert_eq!(value["event"], "ToolCallComplete");
        assert_eq!(value["result"], "result-text");
        assert_eq!(value["duration_ms"], 150);
    }

    #[test]
    fn test_create_event_json_tool_call_complete_no_duration() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::tool_call_complete(
                "id-1".to_string(),
                "search".to_string(),
                "r".to_string(),
                None,
            ),
            &ctx,
        );
        assert_eq!(value["event"], "ToolCallComplete");
        assert!(value.get("duration_ms").is_none());
    }

    #[test]
    fn test_create_event_json_tool_call_error() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::tool_call_error(
                "id-1".to_string(),
                "search".to_string(),
                "failed".to_string(),
                Some(50),
            ),
            &ctx,
        );
        assert_eq!(value["event"], "ToolCallError");
        assert_eq!(value["error"], "failed");
        assert_eq!(value["duration_ms"], 50);
    }

    #[test]
    fn test_create_event_json_tool_call_skipped() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::tool_call_skipped(
                "id-1".to_string(),
                "search".to_string(),
                "not-needed".to_string(),
                None,
            ),
            &ctx,
        );
        assert_eq!(value["event"], "ToolCallSkipped");
        assert_eq!(value["reason"], "not-needed");
        assert!(value.get("duration_ms").is_none());
    }

    #[test]
    fn test_create_event_json_iteration_complete() {
        let ctx = make_ctx();
        let value = parse(
            &AgentStreamEvent::iteration_complete(2, vec![], Some("final".to_string())),
            &ctx,
        );
        assert_eq!(value["event"], "IterationComplete");
        assert_eq!(value["iteration"], 2);
        assert_eq!(value["final_answer"], "final");
        assert!(value.get("tool_calls").is_some());
    }

    #[test]
    fn test_create_event_json_iteration_complete_no_final() {
        let ctx = make_ctx();
        let value = parse(&AgentStreamEvent::iteration_complete(1, vec![], None), &ctx);
        assert_eq!(value["event"], "IterationComplete");
        assert!(value.get("final_answer").is_none());
    }

    #[test]
    fn test_create_event_json_plugin_event() {
        let ctx = make_ctx();
        let mut data = serde_json::Map::new();
        data.insert("custom_key".to_string(), json!("custom_val"));
        let value = parse(
            &AgentStreamEvent::plugin_event("my-plugin".to_string(), data),
            &ctx,
        );
        assert_eq!(value["event"], "PluginEvent");
        assert_eq!(value["plugin_name"], "my-plugin");
        assert_eq!(value["custom_key"], "custom_val");
    }

    #[test]
    fn test_create_event_json_max_iterations_reached() {
        let ctx = make_ctx();
        let value = parse(&AgentStreamEvent::max_iterations_reached(5, 5), &ctx);
        assert_eq!(value["event"], "MaxIterationsReached");
        assert_eq!(value["current_iteration"], 5);
        assert_eq!(value["max_iterations"], 5);
    }

    #[test]
    fn test_create_event_json_iteration_continued() {
        let ctx = make_ctx();
        let value = parse(&AgentStreamEvent::iteration_continued(4), &ctx);
        assert_eq!(value["event"], "IterationContinued");
        assert_eq!(value["from_iteration"], 4);
    }

    #[test]
    fn test_create_event_json_delta_variants_only_metadata() {
        let ctx = make_ctx();
        for event in [
            AgentStreamEvent::thinking_delta("x".to_string()),
            AgentStreamEvent::content_delta("y".to_string()),
            AgentStreamEvent::tool_call_argument_delta(
                "id".to_string(),
                "tool".to_string(),
                "d".to_string(),
            ),
        ] {
            let value = parse(&event, &ctx);
            assert!(value.get("event").is_some());
            assert!(value.get("run_id").is_some());
        }
    }

    #[tokio::test]
    async fn test_listen_normal_event() {
        let plugin = LoggingPlugin::new();
        let ctx = make_ctx();
        // Should not panic; exercises async listen path.
        plugin
            .listen(&AgentStreamEvent::agent_start("hi".to_string()), &ctx)
            .await;
    }

    #[tokio::test]
    async fn test_listen_delta_event_is_noop() {
        let plugin = LoggingPlugin::new();
        let ctx = make_ctx();
        plugin
            .listen(&AgentStreamEvent::thinking_delta("x".to_string()), &ctx)
            .await;
    }

    #[tokio::test]
    async fn test_intercept_continues() {
        let plugin = LoggingPlugin::new();
        let ctx = make_ctx();
        let decision = plugin
            .intercept(&AgentStreamEvent::agent_start("hi".to_string()), &ctx)
            .await;
        assert!(matches!(decision, PluginDecision::Continue));
    }

    #[test]
    fn test_default_impl() {
        let _plugin = LoggingPlugin::default();
    }
}
