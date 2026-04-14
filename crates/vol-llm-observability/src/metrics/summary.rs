use super::state::RunMetrics;

#[derive(Debug)]
pub struct MetricsSummary {
    pub llm_call_count: u32,
    pub tool_call_count: u32,
    pub avg_ttft_ms: Option<f64>,
    pub p50_ttft_ms: Option<u128>,
    pub p99_ttft_ms: Option<u128>,
    pub avg_tool_latency_ms: Option<f64>,
    pub p50_tool_latency_ms: Option<u128>,
    pub p99_tool_latency_ms: Option<u128>,
}

impl MetricsSummary {
    pub fn from_metrics(metrics: &RunMetrics) -> Self {
        Self {
            llm_call_count: metrics.llm_call_count,
            tool_call_count: metrics.tool_call_count,
            avg_ttft_ms: avg(&metrics.ttft_samples_ms),
            p50_ttft_ms: percentile(&metrics.ttft_samples_ms, 50),
            p99_ttft_ms: percentile(&metrics.ttft_samples_ms, 99),
            avg_tool_latency_ms: avg(&metrics.tool_latency_samples_ms),
            p50_tool_latency_ms: percentile(&metrics.tool_latency_samples_ms, 50),
            p99_tool_latency_ms: percentile(&metrics.tool_latency_samples_ms, 99),
        }
    }
    pub fn log_summary(&self, run_id: &str, agent_id: &str) {
        tracing::info!(
            run_id = %run_id, agent_id = %agent_id,
            llm_calls = self.llm_call_count, tool_calls = self.tool_call_count,
            avg_ttft_ms = ?self.avg_ttft_ms, p50_ttft_ms = ?self.p50_ttft_ms,
            p99_ttft_ms = ?self.p99_ttft_ms,
            avg_tool_latency_ms = ?self.avg_tool_latency_ms,
            "Agent run metrics summary"
        );
    }
}

fn avg(samples: &[u128]) -> Option<f64> {
    if samples.is_empty() { return None; }
    Some(samples.iter().sum::<u128>() as f64 / samples.len() as f64)
}

fn percentile(samples: &[u128], pct: usize) -> Option<u128> {
    if samples.is_empty() { return None; }
    let mut sorted = samples.to_vec();
    sorted.sort();
    let idx = (pct as f64 / 100.0 * (sorted.len() - 1) as f64).round() as usize;
    Some(sorted[idx])
}
