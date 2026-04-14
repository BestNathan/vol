# vol-llm-observability Crate Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the observability module from `vol-llm-agent/src/observability/` into a standalone `vol-llm-observability` crate, add metrics collection (TTFT + tool latency), integrate Rust tracing spans, and remove the stale `plugins/observability.rs` duplicate.

**Architecture:**
```
crates/vol-llm-observability/
├── Cargo.toml
└── src/
    ├── lib.rs          → Re-exports: ObservabilityPlugin, ObservabilityConfig,
    │                      RunLogLogger, LogEntry, cleanup functions
    ├── config.rs       → ObservabilityConfig with enable/disable switches
    ├── plugin.rs       → AgentPlugin impl (intercept + listen), moved from vol-llm-agent
    ├── metrics/
    │   ├── mod.rs      → MetricsCollector entry point
    │   ├── state.rs    → Runtime state tracking (TTFT, tool latency, token usage)
    │   └── summary.rs  → Final summary output via tracing::info!
    ├── tracing/
    │   ├── mod.rs      → Module exports
    │   └── spans.rs    → Span creation helpers (llm_call_span, tool_call_span)
    └── run_log/
        ├── mod.rs      → Re-export existing code
        ├── logger.rs   → Moved from vol-llm-agent/src/observability/run_log/logger.rs
        └── cleanup.rs  → Moved from vol-llm-agent/src/observability/run_log/cleanup.rs
```

**Tech Stack:** Rust, tokio, tracing, chrono, serde/serde_json, async-trait
**Tests:** Unit tests for MetricsCollector, integration tests for ObservabilityPlugin
**Approach:** TDD where applicable — write tests first for new MetricsCollector, verify they fail, implement, verify they pass.

---

### Task 1: Create Crate Skeleton

**Goal:** Create `vol-llm-observability` crate with Cargo.toml, workspace registration, and empty modules.

**Files to create:**

1. **`crates/vol-llm-observability/Cargo.toml`**:
```toml
[package]
name = "vol-llm-observability"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
tempfile = "3"

[dev-dependencies]
tracing-subscriber = "0.3"
```

2. **Update workspace `Cargo.toml`** — add `"crates/vol-llm-observability"` to `members` and add a workspace dependency entry:
```toml
# In [workspace] members, add after "crates/vol-llm-tui":
    "crates/vol-llm-observability",

# In [workspace.dependencies], add:
vol-llm-observability = { path = "crates/vol-llm-observability" }
```

3. **`crates/vol-llm-observability/src/lib.rs`**:
```rust
//! vol-llm-observability: Tracing, metrics, and audit logging for LLM agents.
//!
//! Provides an `ObservabilityPlugin` that implements `AgentPlugin` to:
//! - Record structured run logs (JSONL)
//! - Collect metrics (TTFT, tool latency, token usage)
//! - Create tracing spans for LLM calls and tool executions
//! - Clean up old logs based on retention policy

pub mod config;
pub mod metrics;
pub mod plugin;
pub mod run_log;
pub mod tracing;

pub use config::ObservabilityConfig;
pub use metrics::MetricsCollector;
pub use plugin::ObservabilityPlugin;
pub use run_log::{LogEntry, RunLogLogger};
pub use run_log::cleanup::{cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogError};
```

4. **`crates/vol-llm-observability/src/config.rs`**:
```rust
//! Observability configuration.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for the observability plugin.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    /// Enable/disable run log recording.
    pub enable_run_log: bool,
    /// Enable/disable metrics collection.
    pub enable_metrics: bool,
    /// Enable/disable tracing spans.
    pub enable_tracing: bool,
    /// Base path for agent logs.
    pub log_base_path: PathBuf,
    /// Maximum number of run log files to retain per agent.
    pub max_run_logs: usize,
    /// Number of days to retain session logs.
    pub session_retention_days: u32,
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            enable_run_log: true,
            enable_metrics: true,
            enable_tracing: true,
            log_base_path: PathBuf::from("logs/agents"),
            max_run_logs: 10,
            session_retention_days: 7,
        }
    }
}
```

5. **Create empty module files:**
```
crates/vol-llm-observability/src/metrics/mod.rs
crates/vol-llm-observability/src/metrics/state.rs
crates/vol-llm-observability/src/metrics/summary.rs
crates/vol-llm-observability/src/tracing/mod.rs
crates/vol-llm-observability/src/tracing/spans.rs
crates/vol-llm-observability/src/plugin.rs
crates/vol-llm-observability/src/run_log/mod.rs
crates/vol-llm-observability/src/run_log/logger.rs
crates/vol-llm-observability/src/run_log/cleanup.rs
```

With stub contents:
```rust
// metrics/mod.rs
pub mod state;
pub mod summary;
pub use state::RunMetrics;
pub use summary::MetricsSummary;

// metrics/state.rs
use std::time::Instant;

#[derive(Debug, Default)]
pub struct RunMetrics {
    pub llm_call_count: u32,
    pub tool_call_count: u32,
    pub ttft_samples_ms: Vec<u128>,
    pub tool_latency_samples_ms: Vec<u128>,
    llm_call_start: Option<Instant>,
    thinking_start: Option<Instant>,
    content_start: Option<Instant>,
    active_tool_starts: std::collections::HashMap<String, Instant>,
}

impl RunMetrics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_llm_call_start(&mut self) {
        self.llm_call_start = Some(Instant::now());
    }

    pub fn record_thinking_start(&mut self) {
        self.thinking_start = Some(Instant::now());
    }

    pub fn record_content_start(&mut self) {
        self.content_start = Some(Instant::now());
        // TTFT = time from LLMCallStart to ContentStart (or ThinkingStart)
        if let Some(llm_start) = self.llm_call_start.take() {
            let ttft = llm_start.elapsed().as_millis();
            self.ttft_samples_ms.push(ttft);
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

// metrics/summary.rs
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
            run_id = %run_id,
            agent_id = %agent_id,
            llm_calls = self.llm_call_count,
            tool_calls = self.tool_call_count,
            avg_ttft_ms = ?self.avg_ttft_ms,
            p50_ttft_ms = ?self.p50_ttft_ms,
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

// tracing/mod.rs
pub mod spans;
pub use spans::{llm_call_span, tool_call_span, tool_call_span_with_result};

// tracing/spans.rs
use tracing::Span;

/// Create a tracing span for an LLM call.
pub fn llm_call_span(run_id: &str, agent_id: &str, iteration: u32) -> Span {
    tracing::info_span!(
        "llm_call",
        run_id = %run_id,
        agent_id = %agent_id,
        iteration = iteration,
    )
}

/// Create a tracing span for a tool call (begin).
pub fn tool_call_span(run_id: &str, agent_id: &str, tool_name: &str, tool_call_id: &str) -> Span {
    tracing::info_span!(
        "tool_call",
        run_id = %run_id,
        agent_id = %agent_id,
        tool_name = %tool_name,
        tool_call_id = %tool_call_id,
    )
}

/// Create a tracing span for a tool call result.
pub fn tool_call_span_with_result(
    run_id: &str, agent_id: &str, tool_name: &str, tool_call_id: &str, success: bool,
) -> Span {
    tracing::info_span!(
        "tool_call_result",
        run_id = %run_id,
        agent_id = %agent_id,
        tool_name = %tool_name,
        tool_call_id = %tool_call_id,
        success = success,
    )
}

// run_log/mod.rs — re-export from existing code (moved in Task 2)
mod logger;
pub mod cleanup;
pub use logger::{LogEntry, RunLogLogger};
pub use cleanup::{cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogError};

// run_log/logger.rs — copy from vol-llm-agent/src/observability/run_log/logger.rs (Task 2)
// run_log/cleanup.rs — copy from vol-llm-agent/src/observability/run_log/cleanup.rs (Task 2)
```

**Verification:**
```bash
cargo check -p vol-llm-observability
```
Expected: Compiles successfully (or has expected unresolved imports that will be filled in Task 2).

---

### Task 2: Move run_log Module

**Goal:** Move `logger.rs` and `cleanup.rs` from `vol-llm-agent/src/observability/run_log/` to the new crate.

**Files to create:**

1. **`crates/vol-llm-observability/src/run_log/logger.rs`** — copy exact content from `/root/nq-deribit/.worktrees/lifecycle-events/crates/vol-llm-agent/src/observability/run_log/logger.rs` with no changes (it is self-contained).

2. **`crates/vol-llm-observability/src/run_log/cleanup.rs`** — copy exact content from `/root/nq-deribit/.worktrees/lifecycle-events/crates/vol-llm-agent/src/observability/run_log/cleanup.rs` with no changes (it is self-contained).

**Files to update in vol-llm-agent:**

3. **`crates/vol-llm-agent/src/observability/run_log/mod.rs`** — replace with re-exports:
```rust
//! Run log sub-package — re-exported from vol-llm-observability.

pub use vol_llm_observability::run_log::{LogEntry, RunLogLogger};
pub use vol_llm_observability::run_log::cleanup::{
    cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogError,
};
```

4. **`crates/vol-llm-agent/src/observability/mod.rs`** — update to re-export from new crate:
```rust
//! Observability plugin for structured logging and log retention.
//!
//! Core implementation lives in `vol-llm-observability` crate.
//! This module re-exports for backward compatibility.

pub mod plugin;
pub mod run_log;

// Re-export cleanup from vol-llm-observability
pub use vol_llm_observability::run_log::cleanup::{
    cleanup_old_logs, cleanup_run_logs, cleanup_session_logs,
};
// Re-export types
pub use vol_llm_observability::{LogEntry, RunLogLogger};
pub use plugin::ObservabilityPlugin;
```

5. **`crates/vol-llm-agent/Cargo.toml`** — add dependency:
```toml
vol-llm-observability = { path = "../vol-llm-observability" }
```

**Verification:**
```bash
cargo check -p vol-llm-observability
cargo check -p vol-llm-agent
cargo test -p vol-llm-observability
```

---

### Task 3: Implement MetricsCollector with State Tracking

**Goal:** Implement the full `MetricsCollector` that tracks TTFT, tool latency, and produces summaries.

**Files to implement (exact code already in Task 1 stubs, but need to wire them together):**

1. **`crates/vol-llm-observability/src/metrics/mod.rs`** — the entry point:
```rust
//! Metrics collection for agent runs.

pub mod state;
pub mod summary;

pub use state::RunMetrics;
pub use summary::MetricsSummary;

/// High-level metrics collector that wraps RunMetrics and produces summaries.
pub struct MetricsCollector {
    run_id: String,
    agent_id: String,
    metrics: RunMetrics,
}

impl MetricsCollector {
    pub fn new(run_id: String, agent_id: String) -> Self {
        Self {
            run_id,
            agent_id,
            metrics: RunMetrics::new(),
        }
    }

    // === LLM lifecycle ===

    pub fn record_llm_call_start(&mut self) {
        self.metrics.record_llm_call_start();
    }

    pub fn record_thinking_start(&mut self) {
        self.metrics.record_thinking_start();
    }

    pub fn record_content_start(&mut self) {
        self.metrics.record_content_start();
    }

    pub fn record_llm_call_complete(&mut self) {
        self.metrics.record_llm_call_complete();
    }

    // === Tool lifecycle ===

    pub fn record_tool_call_begin(&mut self, tool_call_id: String) {
        self.metrics.record_tool_call_begin(tool_call_id);
    }

    pub fn record_tool_call_complete(&mut self, tool_call_id: String) {
        self.metrics.record_tool_call_complete(tool_call_id);
    }

    // === Summary ===

    pub fn summarize(&self) -> MetricsSummary {
        MetricsSummary::from_metrics(&self.metrics)
    }

    pub fn log_summary(&self) {
        self.summarize().log_summary(&self.run_id, &self.agent_id);
    }
}
```

2. **Add tests to `crates/vol-llm-observability/src/metrics/state.rs`** (append to existing):
```rust
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
    fn test_ttft_is_recorded_on_thinking_start() {
        let mut m = RunMetrics::new();
        m.record_llm_call_start();
        thread::sleep(Duration::from_millis(30));
        m.record_thinking_start();
        // TTFT should NOT be recorded on thinking_start (only content_start)
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
        assert_eq!(m.tool_latency_samples_ms.len(), 0); // no begin, no latency
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
}
```

3. **Add tests to `crates/vol-llm-observability/src/metrics/summary.rs`** (append):
```rust
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
}
```

**Verification (TDD approach):**
```bash
# First write the tests (already included above), then verify they fail without implementation:
# After implementing state.rs and summary.rs:
cargo test -p vol-llm-observability metrics
```
Expected: All tests pass.

---

### Task 4: Implement Tracing Spans Integration

**Goal:** Implement tracing span helpers and integrate them into the plugin's `intercept()` method.

**Files to implement:**

1. **`crates/vol-llm-observability/src/tracing/mod.rs`** (already in Task 1):
```rust
pub mod spans;
pub use spans::{llm_call_span, tool_call_span, tool_call_span_with_result};
```

2. **`crates/vol-llm-observability/src/tracing/spans.rs`** (already in Task 1):
```rust
use tracing::Span;

/// Create a tracing span for an LLM call.
pub fn llm_call_span(run_id: &str, agent_id: &str, iteration: u32) -> Span {
    tracing::info_span!(
        "llm_call",
        run_id = %run_id,
        agent_id = %agent_id,
        iteration = iteration,
    )
}

/// Create a tracing span for a tool call (begin).
pub fn tool_call_span(run_id: &str, agent_id: &str, tool_name: &str, tool_call_id: &str) -> Span {
    tracing::info_span!(
        "tool_call",
        run_id = %run_id,
        agent_id = %agent_id,
        tool_name = %tool_name,
        tool_call_id = %tool_call_id,
    )
}

/// Create a tracing span for a tool call result.
pub fn tool_call_span_with_result(
    run_id: &str, agent_id: &str, tool_name: &str, tool_call_id: &str, success: bool,
) -> Span {
    tracing::info_span!(
        "tool_call_result",
        run_id = %run_id,
        agent_id = %agent_id,
        tool_name = %tool_name,
        tool_call_id = %tool_call_id,
        success = success,
    )
}
```

No tests needed — these are thin wrappers around `tracing::info_span!`.

**Verification:**
```bash
cargo check -p vol-llm-observability
```

---

### Task 5: Implement ObservabilityPlugin (AgentPlugin impl)

**Goal:** Create the plugin that ties together run_log, metrics, and tracing spans.

**File to implement:**

**`crates/vol-llm-observability/src/plugin.rs`**:
```rust
//! ObservabilityPlugin implementation — integrates run_log, metrics, and tracing spans.

use crate::config::ObservabilityConfig;
use crate::metrics::MetricsCollector;
use crate::run_log::LogEntry;
use crate::run_log::RunLogLogger;
use chrono::Utc;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

use vol_llm_agent::react::plugin::{AgentPlugin, PluginDecision, PluginId};
use vol_llm_agent::react::run_context::PluginContext;
use vol_llm_agent::AgentStreamEvent;

pub struct ObservabilityPlugin {
    config: ObservabilityConfig,
    logger: Arc<RunLogLogger>,
    metrics: Arc<Mutex<MetricsCollector>>,
}

impl ObservabilityPlugin {
    pub fn new(agent_id: String, log_base_path: PathBuf) -> Self {
        let config = ObservabilityConfig {
            log_base_path: log_base_path.clone(),
            ..Default::default()
        };
        Self::with_config(agent_id, config)
    }

    pub fn with_config(agent_id: String, config: ObservabilityConfig) -> Self {
        let logger = Arc::new(RunLogLogger::new(
            agent_id.clone(),
            config.log_base_path.clone(),
        ));
        let metrics = Arc::new(Mutex::new(MetricsCollector::new(
            String::new(), // run_id set at runtime
            agent_id,
        )));
        Self { config, logger, metrics }
    }

    fn get_or_create_metrics(&self, run_id: &str) -> Arc<Mutex<MetricsCollector>> {
        // If metrics collector has empty run_id, update it
        // This is a simplification — in production you'd use a RwLock<HashMap<run_id, MetricsCollector>>
        self.metrics.clone()
    }

    fn create_log_entry(&self, event: &AgentStreamEvent, ctx: &PluginContext) -> LogEntry {
        let (event_name, data) = match event {
            AgentStreamEvent::AgentStart { input } => ("AgentStart", json!({ "input": input })),
            AgentStreamEvent::ThinkingComplete { thinking } => {
                ("ThinkingComplete", json!({ "thinking": thinking }))
            }
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
            } => (
                "ToolCallBegin",
                json!({
                    "tool_call_id": tool_call_id,
                    "tool_name": tool_name,
                    "arguments": arguments
                }),
            ),
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
            } => (
                "ToolCallComplete",
                json!({
                    "tool_call_id": tool_call_id,
                    "tool_name": tool_name,
                    "result": result
                }),
            ),
            AgentStreamEvent::IterationComplete {
                iteration,
                tool_calls,
                final_answer,
            } => (
                "IterationComplete",
                json!({
                    "iteration": iteration,
                    "tool_calls": tool_calls,
                    "final_answer": final_answer,
                }),
            ),
            AgentStreamEvent::AgentComplete => ("AgentComplete", json!({})),
            AgentStreamEvent::AgentAborted { reason } => {
                ("AgentAborted", json!({ "reason": reason }))
            }
            AgentStreamEvent::PluginEvent { name, data } => {
                ("PluginEvent", json!({ "name": name, "data": data }))
            }
            AgentStreamEvent::LLMCallStart { iteration } => {
                ("LLMCallStart", json!({ "iteration": iteration }))
            }
            AgentStreamEvent::LLMCallComplete { model, usage } => {
                ("LLMCallComplete", json!({ "model": model, "usage": usage }))
            }
            AgentStreamEvent::LLMCallError { error } => {
                ("LLMCallError", json!({ "error": error }))
            }
            AgentStreamEvent::ThinkingStart => ("ThinkingStart", json!({})),
            AgentStreamEvent::ThinkingDelta { delta } => {
                ("ThinkingDelta", json!({ "delta_len": delta.len() }))
            }
            AgentStreamEvent::ContentStart => ("ContentStart", json!({})),
            AgentStreamEvent::ContentDelta { delta } => {
                ("ContentDelta", json!({ "delta_len": delta.len() }))
            }
            AgentStreamEvent::ContentComplete { content } => {
                ("ContentComplete", json!({ "content_len": content.len() }))
            }
            AgentStreamEvent::ToolCallError {
                tool_call_id,
                tool_name,
                error,
            } => (
                "ToolCallError",
                json!({
                    "tool_call_id": tool_call_id,
                    "tool_name": tool_name,
                    "error": error
                }),
            ),
            AgentStreamEvent::ToolCallSkipped {
                tool_call_id,
                tool_name,
                reason,
            } => (
                "ToolCallSkipped",
                json!({
                    "tool_call_id": tool_call_id,
                    "tool_name": tool_name,
                    "reason": reason
                }),
            ),
        };

        LogEntry {
            timestamp: Utc::now(),
            run_id: ctx.run_id.clone(),
            agent_id: ctx.config.agent_id.clone(),
            event: event_name.to_string(),
            data,
        }
    }

    async fn record_metrics(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        if !self.config.enable_metrics {
            return;
        }
        let mut metrics = self.metrics.lock().await;
        match event {
            AgentStreamEvent::LLMCallStart { .. } => {
                metrics.record_llm_call_start();
            }
            AgentStreamEvent::ThinkingStart => {
                metrics.record_thinking_start();
            }
            AgentStreamEvent::ContentStart => {
                metrics.record_content_start();
            }
            AgentStreamEvent::LLMCallComplete { .. } | AgentStreamEvent::LLMCallError { .. } => {
                metrics.record_llm_call_complete();
            }
            AgentStreamEvent::ToolCallBegin { tool_call_id, .. } => {
                metrics.record_tool_call_begin(tool_call_id.clone());
            }
            AgentStreamEvent::ToolCallComplete { tool_call_id, .. }
            | AgentStreamEvent::ToolCallError { tool_call_id, .. }
            | AgentStreamEvent::ToolCallSkipped { tool_call_id, .. } => {
                metrics.record_tool_call_complete(tool_call_id.clone());
            }
            _ => {}
        }
    }
}

#[async_trait::async_trait]
impl AgentPlugin for ObservabilityPlugin {
    fn id(&self) -> PluginId {
        "observability".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, event: &AgentStreamEvent, ctx: &PluginContext) -> PluginDecision {
        // Record metrics
        self.record_metrics(event, ctx).await;

        // Create tracing span for LLM calls and tool calls
        if self.config.enable_tracing {
            match event {
                AgentStreamEvent::LLMCallStart { iteration } => {
                    let span = crate::tracing::llm_call_span(
                        &ctx.run_id,
                        &ctx.config.agent_id,
                        *iteration,
                    );
                    let _enter = span.enter();
                    tracing::info!("LLM call starting");
                }
                AgentStreamEvent::ToolCallBegin {
                    tool_call_id,
                    tool_name,
                    ..
                } => {
                    let span = crate::tracing::tool_call_span(
                        &ctx.run_id,
                        &ctx.config.agent_id,
                        tool_name,
                        tool_call_id,
                    );
                    let _enter = span.enter();
                    tracing::info!("Tool call starting");
                }
                _ => {}
            }
        }

        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        // Record run log
        if self.config.enable_run_log {
            let entry = self.create_log_entry(event, ctx);
            self.logger.log(&entry, &ctx.run_id).await;
        }

        // Log metrics summary on completion
        if self.config.enable_metrics {
            if matches!(event, AgentStreamEvent::AgentComplete | AgentStreamEvent::AgentAborted { .. }) {
                let metrics = self.metrics.lock().await;
                metrics.log_summary();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::react::run_context::{PluginContext, RunContext};
    use vol_llm_agent::react::AgentConfig;
    use vol_llm_agent::session::{InMemoryMessageStore, InMemorySessionStore, Session};
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_plugin_context() -> PluginContext {
        let (ctx, _rx, _approval_rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(vol_llm_tool::ToolRegistry::new()),
            AgentConfig::default(),
        );
        PluginContext::from_run_ctx(&ctx)
    }

    #[tokio::test]
    async fn test_plugin_id_and_priority() {
        let temp_dir = TempDir::new().unwrap();
        let plugin =
            ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        assert_eq!(plugin.id(), "observability");
        assert_eq!(plugin.priority(), 10);
    }

    #[tokio::test]
    async fn test_plugin_logs_all_event_types() {
        let temp_dir = TempDir::new().unwrap();
        let plugin =
            ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_plugin_context();

        let events = vec![
            AgentStreamEvent::AgentStart { input: "test".to_string() },
            AgentStreamEvent::LLMCallStart { iteration: 1 },
            AgentStreamEvent::ThinkingStart,
            AgentStreamEvent::ContentStart,
            AgentStreamEvent::ContentComplete { content: "answer".to_string() },
            AgentStreamEvent::LLMCallComplete { model: "test".to_string(), usage: None },
            AgentStreamEvent::ToolCallBegin { tool_call_id: "c1".to_string(), tool_name: "grep".to_string(), arguments: "{}".to_string() },
            AgentStreamEvent::ToolCallComplete { tool_call_id: "c1".to_string(), tool_name: "grep".to_string(), result: "found".to_string() },
            AgentStreamEvent::AgentComplete,
        ];

        for event in events {
            plugin.listen(&event, &ctx).await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let run_log_path = temp_dir.path().join("test_agent").join("runs").join("test-run.jsonl");
        assert!(run_log_path.exists());
        let content = std::fs::read_to_string(&run_log_path).unwrap();
        assert!(content.contains("AgentStart"));
        assert!(content.contains("LLMCallStart"));
        assert!(content.contains("ToolCallBegin"));
        assert!(content.contains("AgentComplete"));
    }

    #[tokio::test]
    async fn test_plugin_metrics_collect_through_events() {
        let temp_dir = TempDir::new().unwrap();
        let plugin =
            ObservabilityPlugin::new("test_agent".to_string(), temp_dir.path().to_path_buf());
        let ctx = create_test_plugin_context();

        let events = vec![
            AgentStreamEvent::LLMCallStart { iteration: 1 },
            AgentStreamEvent::ThinkingStart,
            AgentStreamEvent::ContentStart,
            AgentStreamEvent::LLMCallComplete { model: "test".to_string(), usage: None },
        ];

        for event in &events {
            plugin.intercept(event, &ctx).await;
        }

        let metrics = plugin.metrics.lock().await;
        assert_eq!(metrics.summarize().llm_call_count, 1);
        assert_eq!(metrics.summarize().ttft_samples_ms.len(), 1);
    }

    #[tokio::test]
    async fn test_plugin_disabled_run_log() {
        let temp_dir = TempDir::new().unwrap();
        let config = ObservabilityConfig {
            enable_run_log: false,
            ..Default::default()
        };
        let plugin = ObservabilityPlugin::with_config(
            "test_agent".to_string(),
            config,
        );
        let ctx = create_test_plugin_context();

        let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
        plugin.listen(&event, &ctx).await;

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let run_log_path = temp_dir.path().join("test_agent").join("runs").join("test-run.jsonl");
        assert!(!run_log_path.exists());
    }
}
```

**Verification:**
```bash
cargo test -p vol-llm-observability
```
Expected: All tests pass.

---

### Task 6: Update vol-llm-agent to Depend on New Crate

**Goal:** Update `vol-llm-agent` to use `vol-llm-observability` instead of its own observability module.

**Files to update:**

1. **`crates/vol-llm-agent/Cargo.toml`** — add dependency:
```toml
vol-llm-observability = { path = "../vol-llm-observability" }
```

2. **`crates/vol-llm-agent/src/lib.rs`** — update re-exports:
```rust
//! vol-llm-agent: ReAct Agent and RAG Agent workflow orchestration.

pub mod embedding;
pub mod observability;
pub mod plugins;
pub mod prompt_context;
pub mod rag;
pub mod react;
pub mod session;

// Re-export vol-session types
pub use embedding::{DashScopeConfig, DashScopeEmbedder, DashScopeModel, Embedder};
// Re-export observability from vol-llm-observability (backward compatible)
pub use vol_llm_observability::{
    cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogEntry, ObservabilityPlugin,
    ObservabilityConfig, MetricsCollector, RunLogLogger,
};
pub use plugins::{CliApprovalChannel, SimpleHttpApprovalChannel};
pub use prompt_context::{
    FragmentType, MessageAssembler, PromptContext, PromptFragment, PromptTemplate,
};
pub use rag::{Document, EmbeddingStore, RagAgent, RagConfig, RagResponse};
pub use react::state::{ReasoningStep, ToolCallRecord};
pub use react::{
    AgentBuilder, AgentConfig, AgentError, AgentResponse, AgentStreamEvent, AgentStreamReceiver,
    ReActAgent,
};
pub use vol_session::{
    FileMessageStore, InMemoryMessageStore, InMemorySessionStore, MessageStore, Result, Session,
    SessionError, SessionListener, SessionMessage, SessionStore,
};
```

3. **`crates/vol-llm-agent/src/observability/mod.rs`** — update to delegate to new crate:
```rust
//! Observability plugin for structured logging, metrics, and tracing.
//!
//! Core implementation lives in `vol-llm-observability` crate.
//! This module re-exports and provides backward-compatible wrappers.

pub mod plugin;
pub mod run_log;

// Re-export everything from vol-llm-observability
pub use vol_llm_observability::ObservabilityPlugin;
pub use vol_llm_observability::ObservabilityConfig;
pub use vol_llm_observability::MetricsCollector;
pub use vol_llm_observability::RunLogLogger;
pub use vol_llm_observability::LogEntry;
pub use vol_llm_observability::run_log::cleanup::{
    cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogError,
};
```

4. **`crates/vol-llm-agent/src/observability/plugin.rs`** — replace with a thin wrapper that delegates:
```rust
//! Backward-compatible ObservabilityPlugin wrapper.
//!
//! Delegates to vol_llm_observability::ObservabilityPlugin.

pub use vol_llm_observability::ObservabilityPlugin;
```

5. **`crates/vol-llm-agent/src/observability/run_log/mod.rs`** — re-export from new crate:
```rust
//! Run log sub-package — re-exported from vol-llm-observability.

pub use vol_llm_observability::run_log::{LogEntry, RunLogLogger};
pub use vol_llm_observability::run_log::cleanup::{
    cleanup_old_logs, cleanup_run_logs, cleanup_session_logs, LogError,
};
```

6. **`crates/vol-llm-agent/src/react/builder.rs`** — update `with_observability_plugin`:
```rust
    pub fn with_observability_plugin(mut self) -> Self {
        let plugin = vol_llm_observability::ObservabilityPlugin::new(
            self.config.agent_id.clone(),
            self.config.log_base_path.clone(),
        );
        self.config.plugin_registry.register(plugin);
        self
    }
```

7. **`crates/vol-llm-agent/src/react/agent.rs`** — update cleanup import in `run()`:
```rust
// Change:
// crate::observability::cleanup_old_logs
// To:
// vol_llm_observability::cleanup_old_logs
// (or keep as-is since lib.rs re-exports it)
```
No change needed — `crate::observability::cleanup_old_logs` still works because `observability/mod.rs` re-exports it.

**Verification:**
```bash
cargo check -p vol-llm-agent
cargo test -p vol-llm-agent -- observability
```
Expected: All tests pass.

---

### Task 7: Delete Stale plugins/observability.rs

**Goal:** Remove the stale duplicate at `crates/vol-llm-agent/src/plugins/observability.rs`.

**Files to delete:**
- `crates/vol-llm-agent/src/plugins/observability.rs`

**Files to update:**

1. **`crates/vol-llm-agent/src/plugins/mod.rs`** — remove observability:
```rust
//! Built-in plugins for ReAct Agent.

pub mod caching;
pub mod hitl_cli;
pub mod hitl_http;
pub mod rate_limiter;
pub mod retry;

pub use caching::{CachingPlugin, SemanticCache};
pub use hitl_cli::CliApprovalChannel;
pub use hitl_http::SimpleHttpApprovalChannel;
pub use rate_limiter::RateLimiterPlugin;
pub use retry::{RetryConfig, RetryPlugin};
```

**Verification:**
```bash
cargo check -p vol-llm-agent
```
Expected: No compile errors from removed module.

---

### Task 8: Full Verification

**Goal:** Ensure everything compiles and all tests pass across the workspace.

**Commands:**
```bash
# Full workspace check
cargo check --workspace

# Full workspace tests
cargo test --workspace

# Specifically test the new observability crate
cargo test -p vol-llm-observability

# Test vol-llm-agent still works
cargo test -p vol-llm-agent
```

**Expected output:**
- `cargo check --workspace` — no errors, no new warnings
- `cargo test --workspace` — all tests pass
- `cargo test -p vol-llm-observability` — 15+ tests pass (metrics state, summary, plugin, run_log)

---

### Commit Strategy

Each task should be committed separately:

1. `feat: create vol-llm-observability crate skeleton`
2. `refactor: move run_log module to vol-llm-observability`
3. `feat: implement MetricsCollector with TTFT and tool latency tracking`
4. `feat: add tracing span integration for LLM and tool calls`
5. `feat: implement ObservabilityPlugin with metrics and tracing`
6. `refactor: update vol-llm-agent to depend on vol-llm-observability`
7. `chore: remove stale plugins/observability.rs duplicate`
8. `test: full workspace verification`
