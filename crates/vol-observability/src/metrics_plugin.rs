//! MetricsPlugin — records OTel metrics by listening to AgentStreamEvents.
//!
//! Metrics recorded:
//! - `agent_tool_calls_total` (Counter): tool call attempts by tool_name and status
//! - `agent_tool_call_duration_seconds` (Histogram): tool call latency
//! - `agent_ttft_seconds` (Histogram): time to first token (thinking or content, whichever first)
//! - `agent_tokens_used_total` (Counter): input/output/total token usage
//! - `agent_llm_call_errors_total` (Counter): LLM call errors

use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use async_trait::async_trait;
use opentelemetry::{global, metrics::Meter, KeyValue};
use vol_llm_agent::react::{AgentPlugin, PluginDecision, RunContext};
use vol_llm_core::AgentStreamEvent;

/// Internal state for tracking timing correlations.
struct MetricsState {
    /// (agent_id, run_id, iteration) → Instant for TTFT calculation
    llm_call_starts: Vec<(String, String, u32, Instant)>,
    /// tool_call_id → Instant for duration calculation
    tool_call_starts: Vec<(String, Instant)>,
    /// Track which (agent_id, run_id, iteration) already had TTFT measured
    ttft_measured: HashSet<(String, String, u32)>,
    /// (agent_id, run_id) → Instant for run duration tracking
    run_starts: Vec<(String, String, Instant)>,
}

impl MetricsState {
    fn new() -> Self {
        Self {
            llm_call_starts: Vec::new(),
            tool_call_starts: Vec::new(),
            ttft_measured: HashSet::new(),
            run_starts: Vec::new(),
        }
    }

    fn cleanup(&mut self) {
        self.llm_call_starts.clear();
        self.tool_call_starts.clear();
        self.ttft_measured.clear();
        self.run_starts.clear();
    }
}

/// OTel instruments shared across all event processing.
struct Instruments {
    tool_calls_total: opentelemetry::metrics::Counter<u64>,
    tool_call_duration: opentelemetry::metrics::Histogram<f64>,
    ttft_seconds: opentelemetry::metrics::Histogram<f64>,
    tokens_used_total: opentelemetry::metrics::Counter<u64>,
    llm_call_errors_total: opentelemetry::metrics::Counter<u64>,
    runs_total: opentelemetry::metrics::Counter<u64>,
    run_duration: opentelemetry::metrics::Histogram<f64>,
}

impl Instruments {
    fn new(meter: &Meter) -> Self {
        Self {
            tool_calls_total: meter
                .u64_counter("agent_tool_calls_total")
                .with_description("Total tool call attempts")
                .build(),
            tool_call_duration: meter
                .f64_histogram("agent_tool_call_duration_seconds")
                .with_description("Tool call execution latency")
                .build(),
            ttft_seconds: meter
                .f64_histogram("agent_ttft_seconds")
                .with_description("Time to first token (thinking or content, whichever first)")
                .build(),
            tokens_used_total: meter
                .u64_counter("agent_tokens_used_total")
                .with_description("Token usage by type (input/output/total)")
                .build(),
            llm_call_errors_total: meter
                .u64_counter("agent_llm_call_errors_total")
                .with_description("LLM call errors")
                .build(),
            runs_total: meter
                .u64_counter("agent_runs_total")
                .with_description("Total agent runs by status")
                .build(),
            run_duration: meter
                .f64_histogram("agent_run_duration_seconds")
                .with_description("Agent run duration in seconds")
                .build(),
        }
    }
}

/// Plugin that records OTel metrics from agent events.
pub struct MetricsPlugin {
    state: Arc<Mutex<MetricsState>>,
    instruments: Arc<Instruments>,
}

impl MetricsPlugin {
    /// Create a new MetricsPlugin.
    pub fn new() -> Self {
        let meter = global::meter("vol-llm-agent");
        Self {
            state: Arc::new(Mutex::new(MetricsState::new())),
            instruments: Arc::new(Instruments::new(&meter)),
        }
    }

    fn labels(&self, ctx: &RunContext, extra: &[KeyValue]) -> Vec<KeyValue> {
        let mut labels = vec![
            KeyValue::new(
                "agent_id",
                ctx.config
                    .def
                    .as_ref()
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
            KeyValue::new(
                "agent_type",
                ctx.config
                    .def
                    .as_ref()
                    .map(|d| d.r#type.clone())
                    .unwrap_or_else(|| "unknown".to_string()),
            ),
        ];
        labels.extend_from_slice(extra);
        labels
    }

    fn handle_llm_call_start(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if let AgentStreamEvent::LLMCallStart { iteration, .. } = event {
            let agent_id = ctx
                .config
                .def
                .as_ref()
                .map(|d| d.name.clone())
                .unwrap_or_else(|| "unknown".to_string());
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state
                .llm_call_starts
                .push((agent_id, ctx.run_id.clone(), *iteration, Instant::now()));
        }
    }

    fn handle_first_token(&self, _event: &AgentStreamEvent, ctx: &RunContext) {
        let agent_id = ctx
            .config
            .def
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let iteration = ctx.current_iteration();
        let key = (agent_id.clone(), ctx.run_id.clone(), iteration);

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.ttft_measured.contains(&key) {
            return;
        }

        if let Some(pos) = state
            .llm_call_starts
            .iter()
            .rposition(|(aid, rid, iter, _)| {
                aid == &agent_id && rid == &ctx.run_id && *iter == iteration
            })
        {
            let (_, _, _, start_time) = state.llm_call_starts.remove(pos);
            let ttft = start_time.elapsed().as_secs_f64();
            state.ttft_measured.insert(key);

            let model = &ctx.model;
            self.instruments.ttft_seconds.record(
                ttft,
                &[
                    KeyValue::new("model", model.clone()),
                    KeyValue::new("agent_id", agent_id),
                ],
            );
        }
    }

    fn handle_llm_call_complete_cleanup(&self, ctx: &RunContext) {
        let agent_id = ctx
            .config
            .def
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.llm_call_starts.retain(|(aid, rid, iter, _)| {
            !(aid == &agent_id && rid == &ctx.run_id && *iter == ctx.current_iteration())
        });
    }

    fn handle_llm_call_error(&self, ctx: &RunContext) {
        let agent_id = ctx
            .config
            .def
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.llm_call_starts.retain(|(aid, rid, iter, _)| {
            !(aid == &agent_id && rid == &ctx.run_id && *iter == ctx.current_iteration())
        });
    }

    fn record_run_metric(&self, ctx: &RunContext, status: &str) {
        let agent_id = ctx
            .config
            .def
            .as_ref()
            .map(|d| d.name.clone())
            .unwrap_or_else(|| "unknown".to_string());
        let agent_type = ctx
            .config
            .def
            .as_ref()
            .map(|d| d.r#type.clone())
            .unwrap_or_else(|| "unknown".to_string());

        self.instruments.runs_total.add(
            1,
            &[
                KeyValue::new("agent_id", agent_id.clone()),
                KeyValue::new("agent_type", agent_type.clone()),
                KeyValue::new("status", status.to_string()),
            ],
        );

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = state
            .run_starts
            .iter()
            .rposition(|(aid, rid, _)| aid == &agent_id && rid == &ctx.run_id)
        {
            let (_, _, start_time) = state.run_starts.remove(pos);
            let duration = start_time.elapsed().as_secs_f64();
            self.instruments.run_duration.record(
                duration,
                &[
                    KeyValue::new("agent_id", agent_id),
                    KeyValue::new("agent_type", agent_type),
                ],
            );
        }
    }

    fn handle_tool_call_begin(&self, event: &AgentStreamEvent) {
        if let AgentStreamEvent::ToolCallBegin { tool_call_id, .. } = event {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            state
                .tool_call_starts
                .push((tool_call_id.clone(), Instant::now()));
        }
    }

    /// Extract tool call fields from any tool-related event.
    fn extract_tool_call_info(event: &AgentStreamEvent) -> Option<(&str, &str, &Option<u64>)> {
        match event {
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                duration_ms,
                ..
            }
            | AgentStreamEvent::ToolCallError {
                tool_call_id,
                tool_name,
                duration_ms,
                ..
            }
            | AgentStreamEvent::ToolCallSkipped {
                tool_call_id,
                tool_name,
                duration_ms,
                ..
            } => Some((tool_call_id, tool_name, duration_ms)),
            _ => None,
        }
    }

    fn handle_tool_call_complete(&self, event: &AgentStreamEvent, ctx: &RunContext, status: &str) {
        let Some((tool_call_id, tool_name, duration_ms)) = Self::extract_tool_call_info(event)
        else {
            return;
        };

        let duration = duration_ms.map(|ms| ms as f64 / 1000.0).unwrap_or(0.0);

        self.instruments.tool_calls_total.add(
            1,
            &self.labels(
                ctx,
                &[
                    KeyValue::new("tool_name", tool_name.to_string()),
                    KeyValue::new("status", status.to_string()),
                ],
            ),
        );

        self.instruments.tool_call_duration.record(
            duration,
            &self.labels(ctx, &[KeyValue::new("tool_name", tool_name.to_string())]),
        );

        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(pos) = state
            .tool_call_starts
            .iter()
            .rposition(|(id, _)| id == tool_call_id)
        {
            state.tool_call_starts.remove(pos);
        }
    }
}

#[async_trait]
impl AgentPlugin for MetricsPlugin {
    fn id(&self) -> String {
        "metrics".to_string()
    }

    fn priority(&self) -> u32 {
        30
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        match event {
            AgentStreamEvent::AgentStart { .. } => {
                let agent_id = ctx
                    .config
                    .def
                    .as_ref()
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                state
                    .run_starts
                    .push((agent_id, ctx.run_id.clone(), Instant::now()));
            }
            AgentStreamEvent::LLMCallStart { .. } => {
                self.handle_llm_call_start(event, ctx);
            }
            AgentStreamEvent::ThinkingStart { .. } | AgentStreamEvent::ContentStart { .. } => {
                self.handle_first_token(event, ctx);
            }
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
                self.handle_llm_call_complete_cleanup(ctx);
                if let Some(usage) = usage {
                    let agent_id = ctx
                        .config
                        .def
                        .as_ref()
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "unknown".to_string());
                    self.instruments.tokens_used_total.add(
                        usage.prompt_tokens as u64,
                        &[
                            KeyValue::new("model", model.clone()),
                            KeyValue::new("token_type", "input"),
                            KeyValue::new("agent_id", agent_id.clone()),
                        ],
                    );
                    self.instruments.tokens_used_total.add(
                        usage.completion_tokens as u64,
                        &[
                            KeyValue::new("model", model.clone()),
                            KeyValue::new("token_type", "output"),
                            KeyValue::new("agent_id", agent_id.clone()),
                        ],
                    );
                    self.instruments.tokens_used_total.add(
                        usage.total_tokens as u64,
                        &[
                            KeyValue::new("model", model.clone()),
                            KeyValue::new("token_type", "total"),
                            KeyValue::new("agent_id", agent_id),
                        ],
                    );
                }
            }
            AgentStreamEvent::LLMCallError { .. } => {
                self.handle_llm_call_error(ctx);
                let model = &ctx.model;
                let agent_id = ctx
                    .config
                    .def
                    .as_ref()
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| "unknown".to_string());
                self.instruments.llm_call_errors_total.add(
                    1,
                    &[
                        KeyValue::new("model", model.clone()),
                        KeyValue::new("agent_id", agent_id),
                    ],
                );
            }
            AgentStreamEvent::ToolCallBegin { .. } => {
                self.handle_tool_call_begin(event);
            }
            AgentStreamEvent::ToolCallComplete { .. } => {
                self.handle_tool_call_complete(event, ctx, "success");
            }
            AgentStreamEvent::ToolCallError { .. } => {
                self.handle_tool_call_complete(event, ctx, "error");
            }
            AgentStreamEvent::ToolCallSkipped { .. } => {
                self.handle_tool_call_complete(event, ctx, "skipped");
            }
            AgentStreamEvent::AgentComplete { .. } => {
                self.record_run_metric(ctx, "completed");
                self.state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .cleanup();
            }
            AgentStreamEvent::AgentAborted { .. } => {
                self.record_run_metric(ctx, "aborted");
                self.state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .cleanup();
            }
            _ => {}
        }
    }
}

impl Default for MetricsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::react::AgentConfig;

    #[test]
    fn test_plugin_id() {
        let plugin = MetricsPlugin::new();
        assert_eq!(plugin.id(), "metrics");
    }

    #[test]
    fn test_plugin_priority() {
        let plugin = MetricsPlugin::new();
        assert_eq!(plugin.priority(), 30);
    }

    #[test]
    fn test_intercept_always_continues() {
        let plugin = MetricsPlugin::new();
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            AgentConfig::default().into(),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let decision =
            rt.block_on(plugin.intercept(&AgentStreamEvent::agent_start("test".to_string()), &ctx));
        assert!(matches!(decision, PluginDecision::Continue));
    }

    #[test]
    fn test_state_cleanup_on_complete() {
        let plugin = MetricsPlugin::new();

        {
            let mut state = plugin.state.lock().unwrap();
            state.llm_call_starts.push((
                "agent-1".to_string(),
                "run-1".to_string(),
                1,
                Instant::now(),
            ));
            state
                .tool_call_starts
                .push(("tc-1".to_string(), Instant::now()));
            state
                .ttft_measured
                .insert(("agent-1".to_string(), "run-1".to_string(), 1));
            state
                .run_starts
                .push(("agent-1".to_string(), "run-1".to_string(), Instant::now()));
            assert!(!state.llm_call_starts.is_empty());
        }

        {
            let mut state = plugin.state.lock().unwrap();
            state.cleanup();
            assert!(state.llm_call_starts.is_empty());
            assert!(state.tool_call_starts.is_empty());
            assert!(state.ttft_measured.is_empty());
            assert!(state.run_starts.is_empty());
        }
    }

    fn make_ctx() -> RunContext {
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            AgentConfig::default().into(),
        );
        ctx
    }

    #[tokio::test]
    async fn test_listen_agent_start_adds_run_starts() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(&AgentStreamEvent::agent_start("hello".to_string()), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert_eq!(state.run_starts.len(), 1);
        let (aid, rid, _) = &state.run_starts[0];
        assert_eq!(aid, "unknown");
        assert_eq!(rid, "test-run");
    }

    #[tokio::test]
    async fn test_listen_llm_call_start_adds_entry() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert_eq!(state.llm_call_starts.len(), 1);
        let (aid, rid, iter, _) = &state.llm_call_starts[0];
        assert_eq!(aid, "unknown");
        assert_eq!(rid, "test-run");
        assert_eq!(*iter, 0);
    }

    #[tokio::test]
    async fn test_listen_thinking_start_records_ttft() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        // First seed a llm_call_starts entry
        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        {
            let state = plugin.state.lock().unwrap();
            assert_eq!(state.llm_call_starts.len(), 1);
        }

        // ThinkingStart should trigger handle_first_token
        plugin
            .listen(&AgentStreamEvent::thinking_start(), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        // llm_call_starts entry should be removed
        assert!(state.llm_call_starts.is_empty());
        // ttft_measured should have the key
        assert!(state
            .ttft_measured
            .contains(&("unknown".to_string(), "test-run".to_string(), 0)));
    }

    #[tokio::test]
    async fn test_listen_thinking_start_does_not_double_measure() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        // Seed and consume once
        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::thinking_start(), &ctx)
            .await;

        // Second thinking_start should be a no-op (ttft already measured)
        plugin
            .listen(&AgentStreamEvent::thinking_start(), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.llm_call_starts.is_empty());
        assert!(state
            .ttft_measured
            .contains(&("unknown".to_string(), "test-run".to_string(), 0)));
    }

    #[tokio::test]
    async fn test_listen_content_start_records_ttft() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::content_start(), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.llm_call_starts.is_empty());
        assert!(state
            .ttft_measured
            .contains(&("unknown".to_string(), "test-run".to_string(), 0)));
    }

    #[tokio::test]
    async fn test_listen_llm_call_complete_cleanup() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        {
            let state = plugin.state.lock().unwrap();
            assert_eq!(state.llm_call_starts.len(), 1);
        }

        // Complete without usage cleanup
        plugin
            .listen(
                &AgentStreamEvent::llm_call_complete("model".to_string(), None),
                &ctx,
            )
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.llm_call_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_llm_call_complete_with_usage() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();
        let usage = vol_llm_core::TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
            cached_tokens: None,
        };

        // Should not panic even without a prior llm_call_start
        plugin
            .listen(
                &AgentStreamEvent::llm_call_complete("model".to_string(), Some(usage)),
                &ctx,
            )
            .await;

        // State should be clean (no matching entry to remove)
        let state = plugin.state.lock().unwrap();
        assert!(state.llm_call_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_llm_call_error() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        plugin
            .listen(
                &AgentStreamEvent::llm_call_error("timeout".to_string()),
                &ctx,
            )
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.llm_call_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_tool_call_begin_adds_starts() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(
                &AgentStreamEvent::tool_call_begin(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "{}".to_string(),
                ),
                &ctx,
            )
            .await;

        let state = plugin.state.lock().unwrap();
        assert_eq!(state.tool_call_starts.len(), 1);
        assert_eq!(state.tool_call_starts[0].0, "tc-1");
    }

    #[tokio::test]
    async fn test_listen_tool_call_complete_removes_starts() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(
                &AgentStreamEvent::tool_call_begin(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "{}".to_string(),
                ),
                &ctx,
            )
            .await;
        {
            let state = plugin.state.lock().unwrap();
            assert_eq!(state.tool_call_starts.len(), 1);
        }

        plugin
            .listen(
                &AgentStreamEvent::tool_call_complete(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "ok".to_string(),
                    Some(100),
                ),
                &ctx,
            )
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.tool_call_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_tool_call_error_removes_starts() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(
                &AgentStreamEvent::tool_call_begin(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "{}".to_string(),
                ),
                &ctx,
            )
            .await;
        plugin
            .listen(
                &AgentStreamEvent::tool_call_error(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "err".to_string(),
                    None,
                ),
                &ctx,
            )
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.tool_call_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_tool_call_skipped_removes_starts() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(
                &AgentStreamEvent::tool_call_begin(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "{}".to_string(),
                ),
                &ctx,
            )
            .await;
        plugin
            .listen(
                &AgentStreamEvent::tool_call_skipped(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "skip".to_string(),
                    None,
                ),
                &ctx,
            )
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.tool_call_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_agent_complete_cleans_all_state() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        // Seed some state
        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        plugin
            .listen(
                &AgentStreamEvent::tool_call_begin(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "{}".to_string(),
                ),
                &ctx,
            )
            .await;
        plugin
            .listen(&AgentStreamEvent::agent_start("hi".to_string()), &ctx)
            .await;

        plugin
            .listen(&AgentStreamEvent::agent_complete(), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.llm_call_starts.is_empty());
        assert!(state.tool_call_starts.is_empty());
        assert!(state.ttft_measured.is_empty());
        assert!(state.run_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_agent_aborted_cleans_all_state() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(&AgentStreamEvent::agent_start("hi".to_string()), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::agent_aborted("err".to_string()), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.run_starts.is_empty());
    }

    #[tokio::test]
    async fn test_full_sequence_state_clean() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        plugin
            .listen(&AgentStreamEvent::agent_start("hi".to_string()), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::thinking_start(), &ctx)
            .await;
        plugin
            .listen(
                &AgentStreamEvent::llm_call_complete("model".to_string(), None),
                &ctx,
            )
            .await;
        plugin
            .listen(
                &AgentStreamEvent::tool_call_begin(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "{}".to_string(),
                ),
                &ctx,
            )
            .await;
        plugin
            .listen(
                &AgentStreamEvent::tool_call_complete(
                    "tc-1".to_string(),
                    "search".to_string(),
                    "ok".to_string(),
                    Some(50),
                ),
                &ctx,
            )
            .await;
        plugin
            .listen(&AgentStreamEvent::agent_complete(), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert!(state.llm_call_starts.is_empty());
        assert!(state.tool_call_starts.is_empty());
        assert!(state.ttft_measured.is_empty());
        assert!(state.run_starts.is_empty());
    }

    #[tokio::test]
    async fn test_listen_delta_events_are_noop() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        // delta events hit the `_ => {}` arm
        plugin
            .listen(&AgentStreamEvent::thinking_delta("x".to_string()), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::content_delta("y".to_string()), &ctx)
            .await;
        plugin
            .listen(
                &AgentStreamEvent::tool_call_argument_delta(
                    "id".to_string(),
                    "tool".to_string(),
                    "d".to_string(),
                ),
                &ctx,
            )
            .await;
        plugin
            .listen(&AgentStreamEvent::iteration_complete(0, vec![], None), &ctx)
            .await;
        plugin
            .listen(
                &AgentStreamEvent::plugin_event("p".to_string(), serde_json::Map::new()),
                &ctx,
            )
            .await;
        plugin
            .listen(&AgentStreamEvent::max_iterations_reached(5, 5), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::iteration_continued(3), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::thinking_complete("x".to_string()), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::content_complete("x".to_string()), &ctx)
            .await;

        // no panic = pass
    }

    #[tokio::test]
    async fn test_listen_multiple_llm_call_starts() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        // Simulate 2 iterations
        plugin
            .listen(&AgentStreamEvent::llm_call_start(0, vec![]), &ctx)
            .await;
        plugin
            .listen(&AgentStreamEvent::llm_call_start(1, vec![]), &ctx)
            .await;

        let state = plugin.state.lock().unwrap();
        assert_eq!(state.llm_call_starts.len(), 2);
    }

    #[tokio::test]
    async fn test_listen_llm_call_error_without_prior_start() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();

        // No prior start, should not panic
        plugin
            .listen(&AgentStreamEvent::llm_call_error("fail".to_string()), &ctx)
            .await;
    }

    #[test]
    fn test_labels_default() {
        let plugin = MetricsPlugin::new();
        let ctx = make_ctx();
        let labels = plugin.labels(&ctx, &[]);
        assert!(labels.iter().any(|kv| kv.value.as_str() == "unknown"));
    }

    #[test]
    fn test_extract_tool_call_info_none() {
        let event = AgentStreamEvent::agent_start("x".to_string());
        let result = MetricsPlugin::extract_tool_call_info(&event);
        assert!(result.is_none());
    }

    #[test]
    fn test_extract_tool_call_info_complete() {
        let event = AgentStreamEvent::tool_call_complete(
            "id-1".to_string(),
            "search".to_string(),
            "ok".to_string(),
            Some(100),
        );
        let result = MetricsPlugin::extract_tool_call_info(&event);
        assert!(result.is_some());
        let (id, name, dur) = result.unwrap();
        assert_eq!(id, "id-1");
        assert_eq!(name, "search");
        assert_eq!(*dur, Some(100));
    }

    #[test]
    fn test_extract_tool_call_info_error() {
        let event = AgentStreamEvent::tool_call_error(
            "id-2".to_string(),
            "calc".to_string(),
            "err".to_string(),
            None,
        );
        let result = MetricsPlugin::extract_tool_call_info(&event);
        assert!(result.is_some());
        let (id, name, dur) = result.unwrap();
        assert_eq!(id, "id-2");
        assert_eq!(name, "calc");
        assert!(dur.is_none());
    }

    #[test]
    fn test_extract_tool_call_info_skipped() {
        let event = AgentStreamEvent::tool_call_skipped(
            "id-3".to_string(),
            "db".to_string(),
            "skip".to_string(),
            Some(5),
        );
        let result = MetricsPlugin::extract_tool_call_info(&event);
        assert!(result.is_some());
        let (id, name, dur) = result.unwrap();
        assert_eq!(id, "id-3");
        assert_eq!(name, "db");
        assert_eq!(*dur, Some(5));
    }

    #[test]
    fn test_default_impl() {
        let _plugin = MetricsPlugin::default();
    }
}
