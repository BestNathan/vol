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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metrics::state::RunMetrics;

    #[test]
    fn test_summary_from_empty_metrics() {
        let m = RunMetrics::new();
        let s = MetricsSummary::from_metrics(&m);
        assert_eq!(s.llm_call_count, 0);
        assert_eq!(s.tool_call_count, 0);
        assert!(s.avg_ttft_ms.is_none());
        assert!(s.p50_ttft_ms.is_none());
        assert!(s.p99_ttft_ms.is_none());
        assert!(s.avg_tool_latency_ms.is_none());
    }

    #[test]
    fn test_summary_with_samples() {
        let mut m = RunMetrics::new();
        m.llm_call_count = 3;
        m.tool_call_count = 5;
        m.ttft_samples_ms = vec![100, 200, 300];
        m.tool_latency_samples_ms = vec![10, 20, 30, 40, 50];

        let s = MetricsSummary::from_metrics(&m);
        assert_eq!(s.llm_call_count, 3);
        assert_eq!(s.tool_call_count, 5);
        assert_eq!(s.avg_ttft_ms, Some(200.0));
        assert_eq!(s.p50_ttft_ms, Some(200));
        assert_eq!(s.avg_tool_latency_ms, Some(30.0));
    }

    #[test]
    fn test_percentile_single_value() {
        assert_eq!(percentile(&[42], 50), Some(42));
        assert_eq!(percentile(&[42], 99), Some(42));
    }

    #[test]
    fn test_percentile_empty() {
        assert_eq!(percentile(&[], 50), None);
    }

    #[test]
    fn test_avg_empty() {
        assert_eq!(avg(&[]), None);
    }

    #[test]
    fn test_percentile_many_values() {
        let values: Vec<u128> = (1..=100).collect();
        let p50 = percentile(&values, 50).unwrap();
        let p99 = percentile(&values, 99).unwrap();
        assert!(p50 >= 49 && p50 <= 51);
        assert!(p99 >= 98);
    }

    #[test]
    fn test_avg_many_values() {
        let values = vec![10, 20, 30];
        assert_eq!(avg(&values), Some(20.0));
    }
}
