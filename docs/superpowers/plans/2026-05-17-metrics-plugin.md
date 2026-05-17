# Metrics Plugin + ObservabilityPlugin Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove unused `ObservabilityPlugin` code and add a `MetricsPlugin` that records OTel metrics from `AgentStreamEvent`s.

**Architecture:** `MetricsPlugin` implements `AgentPlugin` trait, runs alongside `LoggerPlugin` and `LokiPlugin` in the plugin registry. It consumes events from the agent's `listen()` hook, records metrics using OTel SDK global meter, and cleans up state on run completion.

**Tech Stack:** Rust, `opentelemetry` 0.29, `async_trait`, `tokio`, `vol-llm-agent` plugin system

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/vol-llm-observability/src/agent_plugin.rs` | DELETE | Dead code — no consumers |
| `crates/vol-llm-observability/src/agent_client.rs` | DELETE | Dead code — no consumers |
| `crates/vol-llm-observability/src/agent_config.rs` | DELETE | Dead code — no consumers |
| `crates/vol-llm-observability/src/lib.rs` | MODIFY | Remove deleted module refs, add new `metrics_plugin` module |
| `crates/vol-llm-observability/src/metrics_plugin.rs` | CREATE | `MetricsPlugin` implementation |
| `crates/vol-llm-observability/Cargo.toml` | MODIFY | Remove `reqwest`, add `opentelemetry-otlp` metrics feature |
| `crates/vol-llm-yaml-agent/src/plugins.rs` | MODIFY | Add `"metrics"` case to `register_plugin_by_name` |

---

### Task 1: Remove dead ObservabilityPlugin code

**Files:**
- Delete: `crates/vol-llm-observability/src/agent_plugin.rs`
- Delete: `crates/vol-llm-observability/src/agent_client.rs`
- Delete: `crates/vol-llm-observability/src/agent_config.rs`
- Modify: `crates/vol-llm-observability/src/lib.rs`
- Modify: `crates/vol-llm-observability/Cargo.toml`

- [ ] **Step 1.1: Delete the three dead code files**

```bash
rm crates/vol-llm-observability/src/agent_plugin.rs
rm crates/vol-llm-observability/src/agent_client.rs
rm crates/vol-llm-observability/src/agent_config.rs
```

- [ ] **Step 1.2: Update `lib.rs` — remove deleted module refs**

Current `lib.rs`:
```rust
pub mod agent_config;
pub mod agent_client;
pub mod agent_plugin;
...
pub use agent_config::ObservabilityAgentConfig;
pub use agent_plugin::ObservabilityPlugin;
```

Remove these lines. The updated `lib.rs` should be:

```rust
//! vol-llm-observability: JSONL event logging and observability for LLM agents.
//!
//! Provides:
//! - A `LoggerPlugin` that writes structured run logs as JSONL files
//! - An `init_otel_logs()` helper to initialize the OTel log layer
//! - A `LokiPlugin` that sends agent events to OTel via tracing macros

pub mod plugin;
pub mod run_log;
pub mod otel_init;

pub use plugin::LoggerPlugin;
pub use run_log::{LogEntry, append_log};
pub use otel_init::init_otel_logs;

pub mod loki_plugin;
pub use loki_plugin::LokiPlugin;
```

- [ ] **Step 1.3: Update `Cargo.toml` — remove `reqwest`**

`reqwest` is only used by `agent_client.rs` (deleted). Remove it:

```toml
[package]
name = "vol-llm-observability"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
async-trait = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-appender = { workspace = true }
tracing-subscriber = { workspace = true, features = ["env-filter"] }
chrono = { workspace = true }
opentelemetry = { workspace = true }
opentelemetry_sdk = { workspace = true }
opentelemetry-otlp = { workspace = true, features = ["tokio", "grpc-tonic", "logs"] }
opentelemetry-appender-tracing = { workspace = true }
tempfile = "3"
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-agent = { path = "../vol-llm-agent" }

[dev-dependencies]
tracing-subscriber = "0.3"
tempfile = "3"
vol-session = { path = "../vol-session" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-context = { path = "../vol-llm-context" }
tokio = { workspace = true }
```

- [ ] **Step 1.4: Verify build still works**

```bash
cargo check -p vol-llm-observability
```

Expected: No errors. `vol-llm-observability` compiles cleanly.

- [ ] **Step 1.5: Commit**

```bash
git add crates/vol-llm-observability/src/agent_plugin.rs \
        crates/vol-llm-observability/src/agent_client.rs \
        crates/vol-llm-observability/src/agent_config.rs \
        crates/vol-llm-observability/src/lib.rs \
        crates/vol-llm-observability/Cargo.toml
git commit -m "chore: remove unused ObservabilityPlugin code

agent_plugin, agent_client, and agent_config have zero external consumers.
Safe to delete."
```

---

### Task 2: Implement MetricsPlugin

**Files:**
- Create: `crates/vol-llm-observability/src/metrics_plugin.rs`
- Modify: `crates/vol-llm-observability/src/lib.rs` (add module + re-export)

- [ ] **Step 2.1: Write the MetricsPlugin implementation**

Create `crates/vol-llm-observability/src/metrics_plugin.rs`:

```rust
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

    fn handle_first_token(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        let iteration = ctx.current_iteration();
        let key = (ctx.run_id.clone(), iteration);

        let mut state = self.state.lock().unwrap();
        if state.ttft_measured.contains(&key) {
            return;
        }

        // Find the most recent LLMCallStart for this (run_id, iteration)
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

        // Record tool call count
        self.instruments.tool_calls_total.add(
            1,
            &self.labels(ctx, &[
                KeyValue::new("tool_name", tool_name.clone()),
                KeyValue::new("status", status.to_string()),
            ]),
        );

        // Record tool call duration
        self.instruments.tool_call_duration.record(
            duration,
            &self.labels(ctx, &[
                KeyValue::new("tool_name", tool_name.clone()),
            ]),
        );

        // Clean up timing state
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
                // Record token usage
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
                            KeyValue::new("agent_id", agent_id.clone()),
                        ],
                    );
                }
            }
            AgentStreamEvent::LLMCallError { model, .. } => {
                self.handle_llm_call_error();
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
        use vol_llm_agent::react::{AgentConfig, RunContext};
        use vol_llm_tool::ToolRegistry;
        use vol_session::{InMemoryEntryStore, Session};

        let plugin = MetricsPlugin::new();
        let (ctx, _rx) = RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            AgentConfig::default(),
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

        // Simulate adding state
        {
            let mut state = plugin.state.lock().unwrap();
            state.llm_call_starts.push(("run-1".to_string(), 1, Instant::now()));
            state.tool_call_starts.push(("tc-1".to_string(), Instant::now()));
            state.ttft_measured.insert(("run-1".to_string(), 1));
            assert!(!state.llm_call_starts.is_empty());
        }

        // Simulate AgentComplete triggering cleanup
        {
            let mut state = plugin.state.lock().unwrap();
            state.cleanup();
            assert!(state.llm_call_starts.is_empty());
            assert!(state.tool_call_starts.is_empty());
            assert!(state.ttft_measured.is_empty());
        }
    }
}
```

- [ ] **Step 2.2: Update `lib.rs` — add metrics_plugin module**

Add to `crates/vol-llm-observability/src/lib.rs`:

```rust
pub mod metrics_plugin;
pub use metrics_plugin::MetricsPlugin;
```

Full updated `lib.rs`:
```rust
//! vol-llm-observability: JSONL event logging and observability for LLM agents.
//!
//! Provides:
//! - A `LoggerPlugin` that writes structured run logs as JSONL files
//! - An `init_otel_logs()` helper to initialize the OTel log layer
//! - A `LokiPlugin` that sends agent events to OTel via tracing macros
//! - A `MetricsPlugin` that records OTel metrics from agent events

pub mod plugin;
pub mod run_log;
pub mod otel_init;
pub mod loki_plugin;
pub mod metrics_plugin;

pub use plugin::LoggerPlugin;
pub use run_log::{LogEntry, append_log};
pub use otel_init::init_otel_logs;
pub use loki_plugin::LokiPlugin;
pub use metrics_plugin::MetricsPlugin;
```

- [ ] **Step 2.3: Verify build and tests**

```bash
cargo check -p vol-llm-observability
cargo test -p vol-llm-observability
```

Expected: All 4 tests pass (`test_plugin_id`, `test_plugin_priority`, `test_intercept_always_continues`, `test_state_cleanup_on_complete`).

- [ ] **Step 2.4: Commit**

```bash
git add crates/vol-llm-observability/src/metrics_plugin.rs \
        crates/vol-llm-observability/src/lib.rs
git commit -m "feat: add MetricsPlugin for OTel metrics from agent events

Records tool call counts/success/duration, TTFT, and token usage
via opentelemetry SDK. Implements AgentPlugin trait."
```

---

### Task 3: Register MetricsPlugin in vol-llm-yaml-agent

**Files:**
- Modify: `crates/vol-llm-yaml-agent/src/plugins.rs`

- [ ] **Step 3.1: Add `"metrics"` case to `register_plugin_by_name`**

Add the `"metrics"` arm to the match statement:

```rust
pub fn register_plugin_by_name(
    registry: &mut PluginRegistry,
    name: &str,
    working_dir: &Path,
) -> Result<(), crate::error::YamlAgentError> {
    use crate::error::YamlAgentError;

    match name {
        "logger" => {
            let logger = vol_llm_observability::LoggerPlugin::new(working_dir.to_path_buf());
            registry.register(logger);
        }
        "loki" => {
            let plugin = vol_llm_observability::LokiPlugin::new();
            registry.register(plugin);
        }
        "metrics" => {
            let plugin = vol_llm_observability::MetricsPlugin::new();
            registry.register(plugin);
        }
        _ => return Err(YamlAgentError::UnknownPlugin(name.to_string())),
    }

    Ok(())
}
```

- [ ] **Step 3.2: Verify build and tests**

```bash
cargo check -p vol-llm-yaml-agent
cargo test -p vol-llm-yaml-agent
```

Expected: All tests pass, including the existing `test_register_logger` and `test_register_unknown_plugin`.

- [ ] **Step 3.3: Commit**

```bash
git add crates/vol-llm-yaml-agent/src/plugins.rs
git commit -m "feat: register metrics plugin by name

Add 'metrics' case to register_plugin_by_name for YAML agent config."
```

---

### Task 4: Full workspace verification

- [ ] **Step 4.1: Full workspace check**

```bash
cargo check --all
```

Expected: No errors across all crates.

- [ ] **Step 4.2: Full workspace test**

```bash
cargo test --all
```

Expected: All tests pass.

- [ ] **Step 4.3: Final commit**

```bash
git commit --allow-empty -m "chore: verify metrics plugin integration"
```
