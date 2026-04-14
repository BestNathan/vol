use std::time::Instant;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct RunMetrics {
    pub llm_call_count: u32,
    pub tool_call_count: u32,
    pub ttft_samples_ms: Vec<u128>,
    pub tool_latency_samples_ms: Vec<u128>,
    llm_call_start: Option<Instant>,
    thinking_start: Option<Instant>,
    content_start: Option<Instant>,
    active_tool_starts: HashMap<String, Instant>,
}

impl RunMetrics {
    pub fn new() -> Self { Self::default() }
    pub fn record_llm_call_start(&mut self) { self.llm_call_start = Some(Instant::now()); }
    pub fn record_thinking_start(&mut self) { self.thinking_start = Some(Instant::now()); }
    pub fn record_content_start(&mut self) {
        self.content_start = Some(Instant::now());
        if let Some(llm_start) = self.llm_call_start.take() {
            self.ttft_samples_ms.push(llm_start.elapsed().as_millis());
        }
    }
    pub fn record_tool_call_begin(&mut self, tool_call_id: String) {
        self.active_tool_starts.insert(tool_call_id, Instant::now());
    }
    pub fn record_tool_call_complete(&mut self, tool_call_id: String) {
        self.tool_call_count += 1;
        if let Some(start) = self.active_tool_starts.remove(&tool_call_id) {
            self.tool_latency_samples_ms.push(start.elapsed().as_millis());
        }
    }
    pub fn record_llm_call_complete(&mut self) {
        self.llm_call_count += 1;
        self.llm_call_start = None;
        self.thinking_start = None;
        self.content_start = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn test_ttft_is_recorded_on_content_start() {
        let mut m = RunMetrics::new();
        m.record_llm_call_start();
        thread::sleep(Duration::from_millis(50));
        m.record_content_start();
        assert_eq!(m.ttft_samples_ms.len(), 1);
        assert!(m.ttft_samples_ms[0] >= 50);
    }

    #[test]
    fn test_ttft_is_not_recorded_without_llm_call_start() {
        let mut m = RunMetrics::new();
        m.record_content_start();
        assert_eq!(m.ttft_samples_ms.len(), 0);
    }

    #[test]
    fn test_thinking_start_does_not_record_ttft() {
        let mut m = RunMetrics::new();
        m.record_llm_call_start();
        thread::sleep(Duration::from_millis(30));
        m.record_thinking_start();
        assert_eq!(m.ttft_samples_ms.len(), 0);
    }

    #[test]
    fn test_tool_latency_is_recorded() {
        let mut m = RunMetrics::new();
        m.record_tool_call_begin("call_1".to_string());
        thread::sleep(Duration::from_millis(20));
        m.record_tool_call_complete("call_1".to_string());
        assert_eq!(m.tool_latency_samples_ms.len(), 1);
        assert!(m.tool_latency_samples_ms[0] >= 20);
        assert_eq!(m.tool_call_count, 1);
    }

    #[test]
    fn test_unknown_tool_call_complete_is_safe() {
        let mut m = RunMetrics::new();
        m.record_tool_call_complete("unknown".to_string());
        assert_eq!(m.tool_call_count, 1);
        assert_eq!(m.tool_latency_samples_ms.len(), 0);
    }

    #[test]
    fn test_llm_call_count_increments() {
        let mut m = RunMetrics::new();
        m.record_llm_call_start();
        m.record_llm_call_complete();
        m.record_llm_call_start();
        m.record_llm_call_complete();
        assert_eq!(m.llm_call_count, 2);
    }

    #[test]
    fn test_llm_call_complete_clears_starts() {
        let mut m = RunMetrics::new();
        m.record_llm_call_start();
        m.record_thinking_start();
        m.record_content_start();
        m.record_llm_call_complete();
        // After complete, these should be None (checked via TTFT not being double-recorded)
        // The next content_start should not record TTFT since llm_call_start was cleared
        m.record_content_start();
        assert_eq!(m.ttft_samples_ms.len(), 1); // only the first one
    }

    #[test]
    fn test_multiple_tool_calls() {
        let mut m = RunMetrics::new();
        m.record_tool_call_begin("call_1".to_string());
        m.record_tool_call_begin("call_2".to_string());
        thread::sleep(Duration::from_millis(10));
        m.record_tool_call_complete("call_1".to_string());
        m.record_tool_call_complete("call_2".to_string());
        assert_eq!(m.tool_call_count, 2);
        assert_eq!(m.tool_latency_samples_ms.len(), 2);
    }
}
