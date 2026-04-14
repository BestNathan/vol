pub mod state;
pub mod summary;
pub use state::RunMetrics;
pub use summary::MetricsSummary;

pub struct MetricsCollector {
    run_id: String,
    agent_id: String,
    metrics: RunMetrics,
}

impl MetricsCollector {
    pub fn new(run_id: String, agent_id: String) -> Self {
        Self { run_id, agent_id, metrics: RunMetrics::new() }
    }
    pub fn record_llm_call_start(&mut self) { self.metrics.record_llm_call_start(); }
    pub fn record_thinking_start(&mut self) { self.metrics.record_thinking_start(); }
    pub fn record_content_start(&mut self) { self.metrics.record_content_start(); }
    pub fn record_llm_call_complete(&mut self) { self.metrics.record_llm_call_complete(); }
    pub fn record_tool_call_begin(&mut self, tool_call_id: String) { self.metrics.record_tool_call_begin(tool_call_id); }
    pub fn record_tool_call_complete(&mut self, tool_call_id: String) { self.metrics.record_tool_call_complete(tool_call_id); }
    pub fn summarize(&self) -> MetricsSummary { MetricsSummary::from_metrics(&self.metrics) }
    pub fn log_summary(&self) { self.summarize().log_summary(&self.run_id, &self.agent_id); }
}
