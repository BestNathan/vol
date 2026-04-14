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
