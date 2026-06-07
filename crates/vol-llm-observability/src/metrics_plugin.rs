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
    /// (run_id, iteration) → Instant for TTFT calculation
    llm_call_starts: Vec<(String, u32, Instant)>,
    /// tool_call_id → Instant for duration calculation
    tool_call_starts: Vec<(String, Instant)>,
    /// Track which (run_id, iteration) already had TTFT measured
    ttft_measured: HashSet<(String, u32)>,
}

impl MetricsState {
    fn new() -> Self {
        Self {
            llm_call_starts: Vec::new(),
            tool_call_starts: Vec::new(),
            ttft_measured: HashSet::new(),
        }
    }

    fn cleanup(&mut self) {
        self.llm_call_starts.clear();
        self.tool_call_starts.clear();
        self.ttft_measured.clear();
    }
}

/// OTel instruments shared across all event processing.
struct Instruments {
    tool_calls_total: opentelemetry::metrics::Counter<u64>,
    tool_call_duration: opentelemetry::metrics::Histogram<f64>,
    ttft_seconds: opentelemetry::metrics::Histogram<f64>,
    tokens_used_total: opentelemetry::metrics::Counter<u64>,
    llm_call_errors_total: opentelemetry::metrics::Counter<u64>,
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
            KeyValue::new("agent_id", ctx.config.def.as_ref()
                .map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string())),
            KeyValue::new("agent_type", ctx.config.def.as_ref()
                .map(|d| d.r#type.clone()).unwrap_or_else(|| "unknown".to_string())),
        ];
        labels.extend_from_slice(extra);
        labels
    }

    fn handle_llm_call_start(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if let AgentStreamEvent::LLMCallStart { iteration, .. } = event {
            let mut state = self.state.lock().unwrap();
            state.llm_call_starts.push((ctx.run_id.clone(), *iteration, Instant::now()));
        }
    }

    fn handle_first_token(&self, _event: &AgentStreamEvent, ctx: &RunContext) {
        let iteration = ctx.current_iteration();
        let key = (ctx.run_id.clone(), iteration);

        let mut state = self.state.lock().unwrap();
        if state.ttft_measured.contains(&key) {
            return;
        }

        if let Some(pos) = state.llm_call_starts.iter().rposition(
            |(run_id, iter, _)| run_id == &ctx.run_id && *iter == iteration
        ) {
            let (_, _, start_time) = &state.llm_call_starts[pos];
            let ttft = start_time.elapsed().as_secs_f64();
            state.ttft_measured.insert(key);

            let model = &ctx.model;
            self.instruments.ttft_seconds.record(
                ttft,
                &[
                    KeyValue::new("model", model.clone()),
                    KeyValue::new("agent_id", ctx.config.def.as_ref()
                        .map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string())),
                ],
            );
        }
    }

    fn handle_llm_call_complete_cleanup(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.llm_call_starts.is_empty() {
            state.llm_call_starts.pop();
        }
    }

    fn handle_llm_call_error(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.llm_call_starts.is_empty() {
            state.llm_call_starts.pop();
        }
    }

    fn handle_tool_call_begin(&self, event: &AgentStreamEvent) {
        if let AgentStreamEvent::ToolCallBegin { tool_call_id, .. } = event {
            let mut state = self.state.lock().unwrap();
            state.tool_call_starts.push((tool_call_id.clone(), Instant::now()));
        }
    }

    /// Extract tool call fields from any tool-related event.
    fn extract_tool_call_info(event: &AgentStreamEvent) -> Option<(&str, &str, &Option<u64>)> {
        match event {
            AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, duration_ms, .. }
            | AgentStreamEvent::ToolCallError { tool_call_id, tool_name, duration_ms, .. }
            | AgentStreamEvent::ToolCallSkipped { tool_call_id, tool_name, duration_ms, .. } => {
                Some((tool_call_id, tool_name, duration_ms))
            }
            _ => None,
        }
    }

    fn handle_tool_call_complete(&self, event: &AgentStreamEvent, ctx: &RunContext, status: &str) {
        let Some((tool_call_id, tool_name, duration_ms)) = Self::extract_tool_call_info(event) else {
            return;
        };

        let duration = duration_ms
            .map(|ms| ms as f64 / 1000.0)
            .unwrap_or(0.0);

        self.instruments.tool_calls_total.add(
            1,
            &self.labels(ctx, &[
                KeyValue::new("tool_name", tool_name.to_string()),
                KeyValue::new("status", status.to_string()),
            ]),
        );

        self.instruments.tool_call_duration.record(
            duration,
            &self.labels(ctx, &[
                KeyValue::new("tool_name", tool_name.to_string()),
            ]),
        );

        let mut state = self.state.lock().unwrap();
        if let Some(pos) = state.tool_call_starts.iter().rposition(
            |(id, _)| id == tool_call_id
        ) {
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
            AgentStreamEvent::LLMCallStart { .. } => {
                self.handle_llm_call_start(event, ctx);
            }
            AgentStreamEvent::ThinkingStart { .. }
            | AgentStreamEvent::ContentStart { .. } => {
                self.handle_first_token(event, ctx);
            }
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
                self.handle_llm_call_complete_cleanup();
                if let Some(usage) = usage {
                    let agent_id = ctx.config.def.as_ref()
                        .map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string());
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
                self.handle_llm_call_error();
                let model = &ctx.model;
                let agent_id = ctx.config.def.as_ref()
                    .map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string());
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
            AgentStreamEvent::AgentComplete { .. }
            | AgentStreamEvent::AgentAborted { .. } => {
                self.state.lock().unwrap().cleanup();
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
        use vol_llm_agent::react::AgentConfig;

        let plugin = MetricsPlugin::new();
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            AgentConfig::default().into(),
        );

        let rt = tokio::runtime::Runtime::new().unwrap();
        let decision = rt.block_on(plugin.intercept(
            &AgentStreamEvent::agent_start("test".to_string()),
            &ctx,
        ));
        assert!(matches!(decision, PluginDecision::Continue));
    }

    #[test]
    fn test_state_cleanup_on_complete() {
        let plugin = MetricsPlugin::new();

        {
            let mut state = plugin.state.lock().unwrap();
            state.llm_call_starts.push(("run-1".to_string(), 1, Instant::now()));
            state.tool_call_starts.push(("tc-1".to_string(), Instant::now()));
            state.ttft_measured.insert(("run-1".to_string(), 1));
            assert!(!state.llm_call_starts.is_empty());
        }

        {
            let mut state = plugin.state.lock().unwrap();
            state.cleanup();
            assert!(state.llm_call_starts.is_empty());
            assert!(state.tool_call_starts.is_empty());
            assert!(state.ttft_measured.is_empty());
        }
    }
}
