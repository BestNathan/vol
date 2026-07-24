# Observability Pull-Metrics Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor observability from OTLP-push + TDengine-ingest to Prometheus pull + stdout log discovery, consolidate two crates into one, fix missing LLMCall events, and move JSONL run logging into the agent crate.

**Architecture:** Three plugins (LoggingPlugin, MetricsPlugin, RunLogPlugin) sit on the agent event bus. LoggingPlugin outputs structured JSON to stdout via tracing (Alloy discovers it). MetricsPlugin uses OTel Meter backed by `opentelemetry-prometheus`; agent-server exposes `/metrics` on its existing HTTP port. RunLogPlugin (moved to `vol-llm-agent`) writes JSONL files. The `vol-llm-observability` crate is deleted and its contents merged into `vol-observability` (converted from binary to lib). The `vol-observability` binary (ingest API + TDengine + Loki writer) is removed entirely.

**Tech Stack:** Rust, axum, OTel SDK, `opentelemetry-prometheus`, tracing, Cargo workspace

---

### Task 1: Move run_log module into vol-llm-agent

**Files:**
- Create: `crates/vol-llm-agent/src/run_log/mod.rs`
- Create: `crates/vol-llm-agent/src/run_log/logger.rs`
- Delete: `crates/vol-llm-observability/src/run_log/`
- Modify: `crates/vol-llm-agent/src/lib.rs` (add `pub mod run_log`)
- Modify: `crates/vol-llm-agent/Cargo.toml` (add `chrono, serde_json, serde, tokio` deps if not already present; add `tempfile` dev-dep)

- [ ] **Step 1: Create `crates/vol-llm-agent/src/run_log/mod.rs`**

```rust
//! Run log sub-package for structured JSONL logging.

pub mod logger;

pub use logger::{append_log, LogEntry};
```

- [ ] **Step 2: Create `crates/vol-llm-agent/src/run_log/logger.rs`**

Copy the content from `crates/vol-llm-observability/src/run_log/logger.rs`. Add `session_id` field to `LogEntry`:

```rust
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub session_id: String,  // NEW: was missing
    pub event: String,
    pub data: Value,
}

impl LogEntry {
    pub fn to_json_line(&self) -> String {
        let mut map = serde_json::Map::new();
        map.insert("timestamp".to_string(), json!(self.timestamp.to_rfc3339()));
        map.insert("event".to_string(), json!(self.event));
        map.insert("run_id".to_string(), json!(self.run_id));
        map.insert("session_id".to_string(), json!(self.session_id));  // NEW
        if let Some(data_map) = self.data.as_object() {
            for (k, v) in data_map {
                map.insert(k.clone(), v.clone());
            }
        }
        json!(map).to_string()
    }

    pub fn format_event_summary(&self) -> String {
        // Same as before — unchanged
        match self.event.as_str() {
            "AgentStart" => format!(
                "Agent started - input: {:?}",
                self.data.get("input").and_then(|v| v.as_str()).unwrap_or("")
            ),
            "ThinkingComplete" => "Thinking complete".to_string(),
            "ToolCallBegin" => format!(
                "Tool call: {}",
                self.data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown")
            ),
            "ToolCallComplete" => format!(
                "Tool result: {}",
                self.data.get("result").and_then(|v| v.as_str()).unwrap_or("")
            ),
            "IterationComplete" => format!(
                "Iteration {} complete",
                self.data.get("iteration").and_then(serde_json::Value::as_u64).unwrap_or(0)
            ),
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => format!(
                "Agent aborted: {}",
                self.data.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown")
            ),
            "PluginEvent" => format!(
                "Plugin event: {}",
                self.data.get("name").and_then(|v| v.as_str()).unwrap_or("unknown")
            ),
            _ => self.event.clone(),
        }
    }
}

pub async fn append_log(path: &std::path::Path, line: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    use tokio::io::AsyncWriteExt;
    let mut file = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .await?;
    let mut buf = line.as_bytes().to_vec();
    buf.push(b'\n');
    file.write_all(&buf).await?;
    file.flush().await?;
    Ok(())
}
```

Tests: copy the existing tests but update assertions to verify `session_id` is present:
- `test_log_entry_serialization` — assert `session_id` appears in the JSON line
- `test_format_event_summary` — unchanged
- `test_append_log_creates_dirs_and_writes` — unchanged
- `test_append_log_appends` — unchanged

- [ ] **Step 3: Add `pub mod run_log` to `vol-llm-agent/src/lib.rs`**

Find the existing mod declarations and add `pub mod run_log;` (or `mod run_log;` if not publicly exported — it should be `pub` since `LogEntry` is used by TUI).

- [ ] **Step 4: Verify `vol-llm-agent/Cargo.toml` has the needed deps**

Check that `chrono`, `serde`, `serde_json`, `tokio` are present. If not, add them. Add `tempfile` under `[dev-dependencies]`.

- [ ] **Step 5: Run tests for the moved module**

Run: `cargo test -p vol-llm-agent -- run_log`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/run_log/ crates/vol-llm-agent/src/lib.rs
git commit -m "refactor: move run_log module into vol-llm-agent, add session_id to LogEntry"
```

---

### Task 2: Move RunLogPlugin (formerly LoggerPlugin) into vol-llm-agent

**Files:**
- Create: `crates/vol-llm-agent/src/run_log_plugin.rs`
- Delete: `crates/vol-llm-observability/src/plugin.rs`
- Modify: `crates/vol-llm-agent/src/lib.rs` (add `mod run_log_plugin`)
- Modify: `crates/vol-llm-agent/Cargo.toml` (add `vol-llm-core` if not already a dep)

- [ ] **Step 1: Write `crates/vol-llm-agent/src/run_log_plugin.rs`**

Based on `vol-llm-observability/src/plugin.rs` but with:
- Uses `crate::run_log::{LogEntry, append_log}` instead of `crate::run_log::logger::LogEntry`
- `create_log_entry` now takes `session_id: &str` as a parameter and includes it in the LogEntry
- Remove event_name() — it's currently duplicated from `AgentStreamEvent::event_name()`; use that instead

```rust
//! RunLogPlugin — Writes agent events to JSONL files.
//!
//! File layout:
//!   {base_dir}/logs/{run_id}.jsonl          (regular events)
//!   {base_dir}/logs/{plugin_name}/{run_id}.jsonl  (PluginEvent)

use std::path::PathBuf;
use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use vol_llm_core::stream::AgentStreamEvent;
use crate::react::{AgentPlugin, PluginDecision, RunContext};
use crate::run_log::{append_log, LogEntry};

/// Writes all agent events to JSONL files.
pub struct RunLogPlugin {
    base_dir: PathBuf,
}

impl RunLogPlugin {
    pub fn new(base_dir: PathBuf) -> Self {
        let logs_dir = base_dir.join("logs");
        if let Err(e) = std::fs::create_dir_all(&logs_dir) {
            tracing::warn!(error = %e, "Failed to create logs directory");
        }
        Self { base_dir }
    }

    pub fn base_dir(&self) -> &std::path::Path {
        &self.base_dir
    }

    fn log_path(&self, event: &AgentStreamEvent, run_id: &str) -> PathBuf {
        match event {
            AgentStreamEvent::PluginEvent { name, .. } =>
                self.base_dir.join("logs").join(name).join(format!("{run_id}.jsonl")),
            _ => self.base_dir.join("logs").join(format!("{run_id}.jsonl")),
        }
    }

    /// Whether an event should be logged — skips high-frequency delta events.
    pub fn should_log(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    fn create_log_entry(event: &AgentStreamEvent, run_id: &str, session_id: &str) -> LogEntry {
        // Same data construction as current LoggerPlugin::create_log_entry
        // (copy the full match arm from vol-llm-observability/src/plugin.rs:58-201)
        // but add session_id to the LogEntry.
        let data = match event {
            AgentStreamEvent::AgentStart { input, .. } => json!({ "input": input }),
            AgentStreamEvent::AgentComplete { response, .. } => json!({ "response": response }),
            AgentStreamEvent::AgentAborted { reason, .. } => json!({ "reason": reason }),
            AgentStreamEvent::LLMCallStart { iteration, messages, .. } => {
                let last_n: Vec<_> = messages.iter().rev().take(5).rev().collect();
                let msgs: Vec<Value> = last_n.iter().map(|m| {
                    let content = m.content.as_ref()
                        .map(|c| {
                            let s = c.as_str();
                            if s.chars().count() > 100 {
                                let truncated: String = s.chars().take(100).collect();
                                format!("{truncated}...")
                            } else { s.to_string() }
                        })
                        .unwrap_or_default();
                    json!({ "role": m.role, "content": content })
                }).collect();
                json!({ "iteration": iteration, "message_count": messages.len(), "messages": msgs })
            }
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => json!({ "model": model, "usage": usage }),
            AgentStreamEvent::LLMCallError { error, .. } => json!({ "error": error }),
            AgentStreamEvent::ThinkingStart { .. } => json!({}),
            AgentStreamEvent::ThinkingComplete { thinking, .. } => json!({ "thinking": thinking }),
            AgentStreamEvent::ContentStart { .. } => json!({}),
            AgentStreamEvent::ContentComplete { content, .. } => json!({ "content": content }),
            AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, arguments, .. } =>
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "arguments": arguments }),
            AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } =>
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "result": result, "duration_ms": duration_ms }),
            AgentStreamEvent::ToolCallError { tool_call_id, tool_name, error, duration_ms, .. } =>
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "error": error, "duration_ms": duration_ms }),
            AgentStreamEvent::ToolCallSkipped { tool_call_id, tool_name, reason, duration_ms, .. } =>
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "reason": reason, "duration_ms": duration_ms }),
            AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer, .. } => {
                let tc: Vec<Value> = tool_calls.iter().map(|tc| json!({
                    "id": &tc.id, "name": &tc.name, "arguments": &tc.arguments, "type": &tc.r#type,
                })).collect();
                json!({ "iteration": iteration, "tool_calls": tc, "final_answer": final_answer })
            }
            AgentStreamEvent::PluginEvent { name, data, .. } => {
                let mut map = serde_json::Map::new();
                map.insert("name".to_string(), Value::String(name.clone()));
                for (k, v) in data { map.insert(k.clone(), v.clone()); }
                Value::Object(map)
            }
            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } =>
                json!({ "current_iteration": current_iteration, "max_iterations": max_iterations }),
            AgentStreamEvent::IterationContinued { from_iteration, .. } => json!({ "from_iteration": from_iteration }),
            AgentStreamEvent::ThinkingDelta { .. } | AgentStreamEvent::ContentDelta { .. } | AgentStreamEvent::ToolCallArgumentDelta { .. } => {
                unreachable!("delta events should be filtered by should_log()")
            }
        };

        LogEntry {
            timestamp: Utc::now(),
            run_id: run_id.to_string(),
            session_id: session_id.to_string(),  // NEW
            event: event.event_name().to_string(),
            data,
        }
    }
}

#[async_trait]
impl AgentPlugin for RunLogPlugin {
    fn id(&self) -> String { "run_log".to_string() }
    fn priority(&self) -> u32 { 10 }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if !Self::should_log(event) { return; }
        let entry = Self::create_log_entry(event, &ctx.run_id, &ctx.session_id);
        let path = self.log_path(event, &ctx.run_id);
        let line = entry.to_json_line();
        if let Err(e) = append_log(&path, &line).await {
            tracing::warn!(path = %path.display(), error = %e, "Failed to write log entry");
        }
    }
}
```

Tests: Copy the existing test suite from `vol-llm-observability/src/plugin.rs` but update callers to pass `session_id`:
- `create_test_context()` — read `session_id` from ctx
- All `create_log_entry` calls need a `session_id` parameter now
- `test_log_path_regular_event` — unchanged
- `test_log_path_plugin_event` — unchanged
- `test_log_entry_all_variants` — update to assert `session_id` in the JSON

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-agent -- run_log_plugin`
Expected: all tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/run_log_plugin.rs
git commit -m "refactor: move RunLogPlugin into vol-llm-agent crate"
```

---

### Task 3: Create LoggingPlugin in vol-observability (merge LokiPlugin + LoggerPlugin formatting)

**Files:**
- Create: `crates/vol-observability/src/lib.rs`
- Create: `crates/vol-observability/src/logging_plugin.rs`
- Create: `crates/vol-observability/src/metrics_plugin.rs` (copy from vol-llm-observability)
- Create: `crates/vol-observability/src/otel_init.rs` (copy from vol-llm-observability)
- Delete: `crates/vol-llm-observability/src/loki_plugin.rs`
- Delete: `crates/vol-llm-observability/src/lib.rs`
- Delete: `crates/vol-llm-observability/src/metrics_plugin.rs` (will be recreated in Task 4)
- Delete: `crates/vol-llm-observability/src/otel_init.rs`
- Delete: `crates/vol-llm-observability/src/run_log/` (moved in Task 1)
- Delete: `crates/vol-llm-observability/Cargo.toml`
- Modify: `crates/vol-observability/Cargo.toml` (add deps from vol-llm-observability)
- Modify: `crates/vol-observability/src/main.rs` (remove or convert to unused)
- Modify: `Cargo.toml` (workspace members: remove `vol-llm-observability`)

- [ ] **Step 1: Rewrite `crates/vol-observability/Cargo.toml`**

Replace the current binary-oriented deps with lib-oriented deps. Keep only what's needed:

```toml
[package]
name = "vol-observability"
version.workspace = true
edition.workspace = true

[lints]
workspace = true

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
opentelemetry-prometheus = "0.29"
prometheus = { workspace = true }
tracing-opentelemetry = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-agent = { path = "../vol-llm-agent" }

[dev-dependencies]
tempfile = "3"
```

Key changes from current:
- Remove `reqwest`, `vol-tdengine`, `vol-config`, `toml` (ingest binary deps)
- Remove `axum` (metrics endpoint is mounted in agent-server, not here)
- Remove `opentelemetry-otlp` `metrics` feature (keep only `tokio`, `grpc-tonic`, `logs`)
- Add `opentelemetry-prometheus = "0.29"`
- Add `vol-llm-core`, `vol-llm-agent` (plugin trait dependency)
- Add `async-trait`, `serde`, `serde_json`, `tracing-appender`

- [ ] **Step 2: Create `crates/vol-observability/src/lib.rs`**

```rust
//! vol-observability: Agent observability plugins and OTel initialization.
//!
//! Provides:
//! - `LoggingPlugin` — emits structured JSON agent events to stdout via tracing
//! - `MetricsPlugin` — records OTel metrics (tool calls, TTFT, tokens, LLM errors, run-level)
//! - `otel_init` — full OTel initialization (traces via OTLP push, logs via OTLP push,
//!   metrics via Prometheus pull)
//! - `build_metrics_router` — axum Router exposing GET /metrics

pub mod logging_plugin;
pub mod metrics_plugin;
pub mod otel_init;

pub use logging_plugin::LoggingPlugin;
pub use metrics_plugin::MetricsPlugin;
pub use otel_init::{init, OtelConfig, OtelGuards};
```

- [ ] **Step 3: Create `crates/vol-observability/src/logging_plugin.rs`**

Merge the formatting logic from `vol-llm-observability/src/loki_plugin.rs` (tracing::info! output) with the field expansion from `LoggerPlugin::create_log_entry` (data detail). The key differences from LokiPlugin:

- Use `event.event_name()` for the `event` field (available from `AgentStreamEvent`)
- Flatten all event fields into the JSON line, like `LoggerPlugin::create_log_entry` does, but output via `tracing::info!` instead of writing to a file
- Include `run_id`, `session_id`, `agent_id`, `agent_type`, `model` as top-level fields

```rust
//! LoggingPlugin — Emits structured JSON agent events to stdout via tracing.
//!
//! Alloy discovers these log lines from stdout and forwards them to Loki.

use async_trait::async_trait;
use serde_json::{json, Value};
use vol_llm_agent::react::{AgentPlugin, PluginDecision, RunContext};
use vol_llm_core::AgentStreamEvent;

pub struct LoggingPlugin;

impl Default for LoggingPlugin {
    fn default() -> Self { Self::new() }
}

impl LoggingPlugin {
    pub fn new() -> Self { Self }

    /// Filter out high-frequency delta events.
    pub fn should_send(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    /// Convert an event to a flat JSON object with metadata.
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

        // Flatten event-specific fields
        use AgentStreamEvent::*;
        match event {
            AgentStart { input, .. } => { map.insert("input".to_string(), json!(input)); }
            AgentComplete { response, .. } => { map.insert("response".to_string(), json!(response)); }
            AgentAborted { reason, .. } => { map.insert("reason".to_string(), json!(reason)); }
            LLMCallStart { iteration, .. } => { map.insert("iteration".to_string(), json!(iteration)); }
            LLMCallComplete { model, usage, .. } => {
                map.insert("model".to_string(), json!(model));
                if let Some(u) = usage {
                    map.insert("input_tokens".to_string(), json!(u.input_tokens));
                    map.insert("output_tokens".to_string(), json!(u.output_tokens));
                    map.insert("total_tokens".to_string(), json!(u.total_tokens));
                }
            }
            LLMCallError { error, .. } => { map.insert("error".to_string(), json!(error)); }
            ThinkingStart { .. } => {}
            ThinkingComplete { thinking, .. } => { map.insert("thinking".to_string(), json!(thinking)); }
            ContentStart { .. } => {}
            ContentComplete { content, .. } => { map.insert("content".to_string(), json!(content)); }
            ToolCallBegin { tool_call_id, tool_name, arguments, .. } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("arguments".to_string(), json!(arguments));
            }
            ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("result".to_string(), json!(result));
                if let Some(d) = duration_ms { map.insert("duration_ms".to_string(), json!(d)); }
            }
            ToolCallError { tool_call_id, tool_name, error, duration_ms, .. } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("error".to_string(), json!(error));
                if let Some(d) = duration_ms { map.insert("duration_ms".to_string(), json!(d)); }
            }
            ToolCallSkipped { tool_call_id, tool_name, reason, duration_ms, .. } => {
                map.insert("tool_call_id".to_string(), json!(tool_call_id));
                map.insert("tool_name".to_string(), json!(tool_name));
                map.insert("reason".to_string(), json!(reason));
                if let Some(d) = duration_ms { map.insert("duration_ms".to_string(), json!(d)); }
            }
            IterationComplete { iteration, tool_calls, final_answer, .. } => {
                map.insert("iteration".to_string(), json!(iteration));
                map.insert("tool_calls".to_string(), json!(tool_calls));
                if let Some(fa) = final_answer { map.insert("final_answer".to_string(), json!(fa)); }
            }
            PluginEvent { name, data, .. } => {
                map.insert("plugin_name".to_string(), json!(name));
                for (k, v) in data { map.insert(k.clone(), v.clone()); }
            }
            MaxIterationsReached { current_iteration, max_iterations, .. } => {
                map.insert("current_iteration".to_string(), json!(current_iteration));
                map.insert("max_iterations".to_string(), json!(max_iterations));
            }
            IterationContinued { from_iteration, .. } => {
                map.insert("from_iteration".to_string(), json!(from_iteration));
            }
            _ => {} // delta events — should not reach here due to should_send
        }

        json!(map).to_string()
    }
}

#[async_trait]
impl AgentPlugin for LoggingPlugin {
    fn id(&self) -> String { "logging".to_string() }
    fn priority(&self) -> u32 { 20 }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if !Self::should_send(event) { return; }
        let event_json = Self::create_event_json(event, ctx);
        tracing::info!("{}", event_json);
    }
}
```

Tests:
- `should_send` filters delta events
- `create_event_json` produces JSON containing `run_id`, `session_id`, `agent_id`, `event`
- AgentStart JSON contains `input` field
- ToolCallComplete JSON contains `tool_call_id`, `tool_name`, `result`, `duration_ms`

- [ ] **Step 4: Copy `otel_init.rs` from `vol-llm-observability` to `vol-observability`**

Copy `crates/vol-llm-observability/src/otel_init.rs` → `crates/vol-observability/src/otel_init.rs`.

Remove the `metrics` feature from `opentelemetry-otlp` builder (lines 163-173 in the original). Replace with `opentelemetry-prometheus` exporter registered against the global default registry (so `metrics_router` reads the same registry):

```rust
// Replace the metrics exporter block (original lines 163-173):
use opentelemetry_prometheus::exporter;

let prometheus_exporter = exporter()
    .with_registry(prometheus::default_registry().clone())
    .build()?;

let meter_provider = SdkMeterProvider::builder()
    .with_resource(resource)
    .with_reader(prometheus_exporter)
    .build();
```

Note: `opentelemetry-prometheus` 0.29's `exporter()` returns an `ExporterBuilder` whose `.build()` yields a reader (`PrometheusExporter`) that implements `MetricReader`. Pass it to `.with_reader(...)`, NOT `.with_periodic_exporter(...)`. Verify the exact builder function name against the 0.29 crate docs; if `exporter()` is not the entry point, use `PrometheusExporter::builder()`.

Remove any `OTEL_METRICS_EXPORTER` env var override (not needed — always Prometheus).

- [ ] **Step 5: Delete `vol-llm-observability` crate files**

Delete these files:
- `crates/vol-llm-observability/src/lib.rs`
- `crates/vol-llm-observability/src/plugin.rs`
- `crates/vol-llm-observability/src/loki_plugin.rs`
- `crates/vol-llm-observability/src/metrics_plugin.rs`
- `crates/vol-llm-observability/src/otel_init.rs`
- `crates/vol-llm-observability/src/run_log/mod.rs`
- `crates/vol-llm-observability/src/run_log/logger.rs`
- `crates/vol-llm-observability/Cargo.toml`
- `crates/vol-llm-observability/src/lib.rs` (already deleted above)

- [ ] **Step 6: Remove `vol-llm-observability` from workspace `Cargo.toml`**

Remove the member line `"crates/vol-llm-observability"` and the workspace dependency `vol-llm-observability = { path = "crates/vol-llm-observability" }`.

- [ ] **Step 7: Delete `vol-observability/src/main.rs` and remove ingest/TDengine/writer code**

Delete `crates/vol-observability/src/main.rs`, `ingest.rs`, `tdengine_writer.rs`, `loki_writer.rs`, `event.rs`, `config.rs` if they exist.

- [ ] **Step 8: Run workspace check**

Run: `cargo check -p vol-observability`
Expected: compilation succeeds (may have warnings about unused imports in otel_init.rs, fix those)

- [ ] **Step 9: Commit**

```bash
git add -A crates/vol-observability/ crates/vol-llm-observability/ Cargo.toml
git rm -r crates/vol-llm-observability/
git commit -m "refactor: consolidate observability crates, create LoggingPlugin, remove ingest/TDengine"
```

---

### Task 4: Update downstream crate references from vol-llm-observability to vol-observability

**Files:**
- Modify: `crates/vol-agent-server/Cargo.toml`
- Modify: `crates/vol-agent-server/src/data_plane/core.rs`
- Modify: `crates/vol-agent-server/src/main.rs`
- Modify: `crates/vol-llm-agents/Cargo.toml`
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-agents/tests/agent_loki_integration.rs`
- Modify: `crates/vol-llm-agents/examples/agent_loki_example.rs`
- Modify: `crates/vol-llm-yaml-agent/Cargo.toml`
- Modify: `crates/vol-llm-yaml-agent/src/plugins.rs`
- Modify: `crates/vol-mcp-servers/Cargo.toml`
- Modify: `crates/vol-mcp-servers/src/bin/cli_tools_mcp.rs`
- Modify: `crates/vol-mcp-servers/src/bin/docs_rs.rs`
- Modify: `crates/vol-llm-tui/Cargo.toml`
- Modify: `crates/vol-llm-tui/src/app.rs`
- Modify: `crates/vol-llm-ui/Cargo.toml` (optional dep)

- [ ] **Step 1: Update agent-server dependencies**

In `crates/vol-agent-server/Cargo.toml`, change:
```toml
vol-llm-observability = { path = "../vol-llm-observability" }
```
to:
```toml
vol-observability = { path = "../vol-observability" }
```

In `crates/vol-agent-server/src/data_plane/core.rs`, change:
```rust
config.plugin_registry.register(vol_llm_observability::MetricsPlugin::new());
config.plugin_registry.register(vol_llm_observability::LokiPlugin::new());
```
to:
```rust
config.plugin_registry.register(vol_observability::MetricsPlugin::new());
config.plugin_registry.register(vol_observability::LoggingPlugin::new());
```

In `crates/vol-agent-server/src/main.rs`, change:
```rust
use vol_llm_observability::OtelConfig;
use vol_llm_observability::init;
```
to:
```rust
use vol_observability::OtelConfig;
use vol_observability::init;
```

- [ ] **Step 2: Update vol-llm-agents dependencies**

In `crates/vol-llm-agents/Cargo.toml`, change `vol-llm-observability` to `vol-observability`.

In `crates/vol-llm-agents/src/coding/agent.rs`, update:
- `vol_llm_observability::LoggerPlugin` → `vol_llm_agent::run_log_plugin::RunLogPlugin` (moved into agent crate)
- `vol_llm_observability::LokiPlugin` → `vol_observability::LoggingPlugin`

In `crates/vol-llm-agents/tests/agent_loki_integration.rs`:
- `vol_llm_observability::LokiPlugin` → `vol_observability::LoggingPlugin`

In `crates/vol-llm-agents/examples/agent_loki_example.rs`:
- `vol_llm_observability::{init_otel_logs, LokiPlugin}` → `vol_observability::{init, LoggingPlugin}`

- [ ] **Step 3: Update vol-llm-yaml-agent plugin registration**

In `crates/vol-llm-yaml-agent/Cargo.toml`, change `vol-llm-observability` to `vol-observability`.

In `crates/vol-llm-yaml-agent/src/plugins.rs`, update:
- `vol_llm_observability::LoggerPlugin` → `vol_llm_agent::run_log_plugin::RunLogPlugin`
- `vol_llm_observability::LokiPlugin` → `vol_observability::LoggingPlugin`
- `vol_llm_observability::MetricsPlugin` → `vol_observability::MetricsPlugin`
- Update plugin name string from `"loki"` to `"logging"` (and update the error message comment)

- [ ] **Step 4: Update vol-mcp-servers**

In `crates/vol-mcp-servers/Cargo.toml`, change `vol-llm-observability` to `vol-observability`.

In `crates/vol-mcp-servers/src/bin/cli_tools_mcp.rs`, change `vol_llm_observability::OtelConfig` → `vol_observability::OtelConfig`, `vol_llm_observability::init` → `vol_observability::init`.

In `crates/vol-mcp-servers/src/bin/docs_rs.rs`, same changes.

- [ ] **Step 5: Update vol-llm-tui LogEntry import**

In `crates/vol-llm-tui/Cargo.toml`, change `vol-llm-observability` to `vol-observability` (or remove it if LogEntry is now from `vol-llm-agent`).

In `crates/vol-llm-tui/src/app.rs`, change:
- `vol_llm_observability::LogEntry` → `vol_llm_agent::run_log::LogEntry`
- The `LogEntry` is now in `vol-llm-agent::run_log`, so add `vol-llm-agent` as a dependency if not already

- [ ] **Step 6: Update vol-llm-ui optional dep**

In `crates/vol-llm-ui/Cargo.toml`, change `vol-llm-observability` to `vol-observability` (if it's actually used — if only optional and never activated, consider removing it).

- [ ] **Step 7: Workspace check**

Run: `cargo check -p vol-agent-server -p vol-llm-agents -p vol-llm-yaml-agent -p vol-mcp-servers -p vol-llm-tui`
Expected: all crates compile

- [ ] **Step 8: Commit**

```bash
git add -A crates/vol-agent-server/ crates/vol-llm-agents/ crates/vol-llm-yaml-agent/ crates/vol-mcp-servers/ crates/vol-llm-tui/ crates/vol-llm-ui/
git commit -m "refactor: update downstream crates to use vol-observability and RunLogPlugin"
```

---

### Task 5: Emit LLMCallStart/Complete/Error in agent loop

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Emit LLMCallStart before the LLM stream call**

In `agent.rs`, around line 424-430 (before `llm.converse_stream`), add:

```rust
// Emit LLMCallStart
let messages = run_ctx.get_context().await?;
run_ctx.emit(AgentStreamEvent::llm_call_start(iteration, messages.clone())).await;

let request = ConversationRequest::with_history(None, messages)
    .with_tools(tools_defs)
    .with_tool_choice(ToolChoice::Auto);
```

Note: `messages` is moved into `llm_call_start` — we need to clone it. But `messages` is also used in `ConversationRequest::with_history`. So the order should be: clone messages, emit with the clone, pass original to request.

- [ ] **Step 2: Emit LLMCallComplete after consume_llm_stream succeeds**

Around line 443-444 where `consume_llm_stream` returns, the `_model` and `_usage` are currently discarded. Capture them and emit:

```rust
let (thinking, tool_calls, content, model, usage) =
    match consume_llm_stream(llm_stream, &run_ctx).await {
        Ok(data) => {
            run_ctx.emit(AgentStreamEvent::llm_call_complete(model.clone(), usage)).await;
            data
        }
        Err(e) => {
            run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
            run_ctx.emit(AgentStreamEvent::agent_aborted(format!(
                "LLM stream failed: {e}"
            ))).await;
            return Err(e);
        }
    };
```

But wait — `consume_llm_stream` returns a tuple `(thinking, tool_calls, content, model, usage)`. If we emit `llm_call_complete` inside the match, we have `model` and `usage` available. Let me look at the return type more carefully.

The return type is:
```rust
( String, Vec<ToolCall>, String, String, Option<TokenUsage> )
// thinking, tool_calls, content, model, usage
```

So the code should be:
```rust
let (thinking, tool_calls, content, model, usage) =
    match consume_llm_stream(llm_stream, &run_ctx).await {
        Ok(data) => data,
        Err(e) => {
            run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
            run_ctx.emit(AgentStreamEvent::agent_aborted(format!(
                "LLM stream failed: {e}"
            ))).await;
            return Err(e);
        }
    };

// Emit LLMCallComplete with the captured model and usage
run_ctx.emit(AgentStreamEvent::llm_call_complete(model.clone(), usage)).await;
```

- [ ] **Step 3: Emit LLMCallError on LLM request failure**

Around line 430-438 (the `llm.converse_stream` error path), add before the `agent_aborted`:

```rust
let llm_stream = match llm.converse_stream(request).await {
    Ok(stream) => stream,
    Err(e) => {
        run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;  // NEW
        run_ctx.emit(AgentStreamEvent::agent_aborted(format!(
            "LLM request failed: {e}"
        ))).await;
        return Err(crate::AgentError::Llm(e));
    }
};
```

- [ ] **Step 4: Write test for LLMCall events in agent_run_tests**

Add a test in `crates/vol-llm-agent/tests/agent_run_tests.rs` (or a new test file):

```rust
#[tokio::test]
async fn test_agent_run_emits_llm_call_events() {
    // Set up a mock LLM that returns a stream with a simple response
    let (llm, mut stream_tx) = MockStreamLlm::new();
    let config = AgentConfig::builder()
        .with_llm(Arc::new(llm))
        .with_tools(Arc::new(ToolRegistry::new()))
        .with_session(Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))))
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    // Collect events via a plugin
    let events = Arc::new(Mutex::new(Vec::new()));
    let collector = EventCollectorPlugin::new(events.clone());
    // ... (similar pattern to existing test_agent_run_event_emission)

    // Send a stream response that completes without tool calls
    stream_tx.send(Ok(StreamEvent {
        data: StreamEventData::ResponseStart { model: "mock".into() },
        ..Default::default()
    })).await.unwrap();
    // ... send content

    let result = agent.run("hello").await;
    assert!(result.is_ok());

    let collected = events.lock().unwrap();
    let has_start = collected.iter().any(|e| matches!(e, AgentStreamEvent::LLMCallStart { .. }));
    let has_complete = collected.iter().any(|e| matches!(e, AgentStreamEvent::LLMCallComplete { .. }));
    assert!(has_start, "should emit LLMCallStart");
    assert!(has_complete, "should emit LLMCallComplete");
}
```

- [ ] **Step 5: Run agent tests**

Run: `cargo test -p vol-llm-agent -- test_agent_run`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "fix: emit LLMCallStart/Complete/Error events in agent loop"
```

---

### Task 6: Add Prometheus metrics exporter and /metrics endpoint

**Files:**
- Create: `crates/vol-observability/src/metrics_router.rs`
- Modify: `crates/vol-observability/src/lib.rs` (add `pub mod metrics_router`)
- Modify: `crates/vol-observability/src/otel_init.rs` (swap to Prometheus exporter)
- Modify: `crates/vol-observability/Cargo.toml` (add `axum` dep)
- Modify: `crates/vol-agent-server/src/routes.rs` (mount `/metrics`)
- Modify: `crates/vol-agent-server/src/app.rs` (pass metrics router)

- [ ] **Step 1: Add `axum` to `vol-observability/Cargo.toml`**

```toml
axum = { workspace = true }
```

- [ ] **Step 2: Create `crates/vol-observability/src/metrics_router.rs`**

```rust
//! Axum router for Prometheus /metrics endpoint.

use axum::{http::StatusCode, routing::get, Router};

/// Build an axum Router with a GET /metrics endpoint that serves
/// Prometheus metrics. The `opentelemetry-prometheus` exporter registers
/// metrics with the global `prometheus::default_registry()`, so we read
/// from there.
pub fn build_metrics_router() -> Router {
    Router::new().route("/metrics", get(metrics_handler))
}

async fn metrics_handler() -> Result<String, StatusCode> {
    let metric_families = prometheus::default_registry().gather();
    let encoder = prometheus::TextEncoder::new();
    encoder
        .encode_to_string(&metric_families)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
```

- [ ] **Step 3: Mount `/metrics` in agent-server's routes.rs**

In `crates/vol-agent-server/src/routes.rs`, add the metrics route:

```rust
use axum::{routing::get, Router};
use crate::health;

pub fn base_router() -> Router {
    Router::new()
        .route("/health", get(health::health))
        .merge(vol_observability::metrics_router::build_metrics_router())
}
```

- [ ] **Step 4: Add `vol-observability` dep to agent-server Cargo.toml**

Already changed in Task 4, but verify `vol-observability` is in `crates/vol-agent-server/Cargo.toml`.

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-agent-server -p vol-observability`
Expected: compiles

- [ ] **Step 6: Commit**

```bash
git add crates/vol-observability/src/metrics_router.rs crates/vol-observability/Cargo.toml crates/vol-agent-server/src/routes.rs
git commit -m "feat: add Prometheus /metrics endpoint via opentelemetry-prometheus"
```

---

### Task 7: Fix MetricsPlugin concurrent state pollution and add run-level metrics

**Files:**
- Modify: `crates/vol-observability/src/metrics_plugin.rs` (copied from vol-llm-observability in Task 3)

- [ ] **Step 1: Fix `MetricsState` keying**

Change `llm_call_starts` from `Vec<(String, u32, Instant)>` to `Vec<(String, String, u32, Instant)>` where the first String is `agent_id`:

```rust
struct MetricsState {
    /// (agent_id, run_id, iteration) → Instant for TTFT calculation
    llm_call_starts: Vec<(String, String, u32, Instant)>,
    /// tool_call_id → Instant for duration calculation
    tool_call_starts: Vec<(String, Instant)>,
    /// Track which (agent_id, run_id, iteration) already had TTFT measured
    ttft_measured: HashSet<(String, String, u32)>,
}
```

Update `handle_llm_call_start`:
```rust
fn handle_llm_call_start(&self, event: &AgentStreamEvent, ctx: &RunContext) {
    if let AgentStreamEvent::LLMCallStart { iteration, .. } = event {
        let agent_id = ctx.config.def.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string());
        let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        state.llm_call_starts.push((agent_id, ctx.run_id.clone(), *iteration, Instant::now()));
    }
}
```

Update `handle_first_token` to use triple key:
```rust
fn handle_first_token(&self, _event: &AgentStreamEvent, ctx: &RunContext) {
    let agent_id = ctx.config.def.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string());
    let iteration = ctx.current_iteration();
    let key = (agent_id.clone(), ctx.run_id.clone(), iteration);

    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if state.ttft_measured.contains(&key) { return; }

    // Find by (agent_id, run_id, iteration) — NOT by pop()
    if let Some(pos) = state.llm_call_starts.iter().rposition(|(aid, rid, iter, _)| {
        aid == &agent_id && rid == &ctx.run_id && *iter == iteration
    }) {
        let (_, _, _, start_time) = state.llm_call_starts.remove(pos);
        let ttft = start_time.elapsed().as_secs_f64();
        state.ttft_measured.insert(key);
        // record TTFT...
    }
}
```

Update `handle_llm_call_complete_cleanup` and `handle_llm_call_error` to remove by `(agent_id, run_id, iteration)` instead of pop():

```rust
fn handle_llm_call_complete_cleanup(&self, ctx: &RunContext) {
    let agent_id = ctx.config.def.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string());
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    state.llm_call_starts.retain(|(aid, rid, iter, _)| {
        !(aid == &agent_id && rid == &ctx.run_id && *iter == ctx.current_iteration())
    });
}
```

- [ ] **Step 2: Add run-level metrics**

Add new instruments in `Instruments`:

```rust
struct Instruments {
    // ... existing fields ...
    runs_total: opentelemetry::metrics::Counter<u64>,
    run_duration: opentelemetry::metrics::Histogram<f64>,
}
```

Initialize in `Instruments::new`:
```rust
runs_total: meter
    .u64_counter("agent_runs_total")
    .with_description("Total agent runs by status")
    .build(),
run_duration: meter
    .f64_histogram("agent_run_duration_seconds")
    .with_description("Agent run duration in seconds")
    .build(),
```

Add a new field `run_starts: Vec<(String, String, Instant)>` to `MetricsState` for tracking run start times. Record on `AgentStart`, measure on `AgentComplete` / `AgentAborted`.

In `listen()`, handle `AgentStart`:
```rust
AgentStreamEvent::AgentStart { .. } => {
    let agent_id = ctx.config.def.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string());
    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    state.run_starts.push((agent_id, ctx.run_id.clone(), Instant::now()));
}
```

Handle `AgentComplete` / `AgentAborted`:
```rust
AgentStreamEvent::AgentComplete { .. } => {
    self.record_run_metric(ctx, "completed");
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).cleanup();
}
AgentStreamEvent::AgentAborted { .. } => {
    self.record_run_metric(ctx, "aborted");
    self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner).cleanup();
}
```

Add `record_run_metric` method:
```rust
fn record_run_metric(&self, ctx: &RunContext, status: &str) {
    let agent_id = ctx.config.def.as_ref().map(|d| d.name.clone()).unwrap_or_else(|| "unknown".to_string());
    let agent_type = ctx.config.def.as_ref().map(|d| d.r#type.clone()).unwrap_or_else(|| "unknown".to_string());

    self.instruments.runs_total.add(1, &[
        KeyValue::new("agent_id", agent_id.clone()),
        KeyValue::new("agent_type", agent_type.clone()),
        KeyValue::new("status", status.to_string()),
    ]);

    let mut state = self.state.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(pos) = state.run_starts.iter().rposition(|(aid, rid, _)| aid == &agent_id && rid == &ctx.run_id) {
        let (_, _, start_time) = state.run_starts.remove(pos);
        let duration = start_time.elapsed().as_secs_f64();
        self.instruments.run_duration.record(duration, &[
            KeyValue::new("agent_id", agent_id),
            KeyValue::new("agent_type", agent_type),
        ]);
    }
}
```

- [ ] **Step 3: Update `cleanup()` to clear `run_starts`**

```rust
fn cleanup(&mut self) {
    self.llm_call_starts.clear();
    self.tool_call_starts.clear();
    self.ttft_measured.clear();
    self.run_starts.clear();  // NEW
}
```

- [ ] **Step 4: Update the `listen()` match to call new handlers**

Update the `LLMCallComplete` handler to pass `ctx`:
```rust
AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
    self.handle_llm_call_complete_cleanup(ctx);
    // ... token recording unchanged ...
}
```

Update `LLMCallError`:
```rust
AgentStreamEvent::LLMCallError { .. } => {
    self.handle_llm_call_error(ctx);
    // ...
}
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vol-observability -- metrics_plugin`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/vol-observability/src/metrics_plugin.rs
git commit -m "fix: fix MetricsPlugin concurrent state, add run-level metrics"
```

---

### Task 8: Final cleanup and verification

**Files:**
- Modify: `Cargo.toml` (workspace members, deps)
- Possibly: `k8s/` manifests, `deploy/` argocd, Dockerfiles, smoke tests

- [ ] **Step 1: Update workspace Cargo.toml**

Ensure `vol-llm-observability` is removed from workspace members and dependencies.
Add `vol-observability` as a workspace dependency if not already (it is, at line 144).

- [ ] **Step 2: Check for remaining references to `vol_llm_observability`**

Run: `grep -rn "vol_llm_observability" --include="*.rs" --include="*.toml" crates/`
Expected: no results (all references updated in Task 4)

- [ ] **Step 3: Check for remaining references to ingest/TDengine**

Run: `grep -rn "tdengine\|ingest\|IngestBatch\|IngestEvent\|ExtractedMetric\|TdengineCommand\|LokiCommand" --include="*.rs" crates/vol-observability/`
Expected: no results (all deleted)

- [ ] **Step 4: Full workspace check**

Run: `cargo check`
Expected: all crates compile

- [ ] **Step 5: Run full test suite**

Run: `cargo test -p vol-agent-server -p vol-llm-agent -p vol-observability -p vol-llm-yaml-agent -p vol-llm-agents -p vol-llm-tui`
Expected: all tests pass

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: final cleanup after observability refactor"
```

---

### Task 9: wiki-ingest and docs

- [ ] **Step 1: Run `wiki-ingest` skill**

Invoke the wiki-ingest skill to document the refactoring in `docs/wiki/`.

- [ ] **Step 2: Update k8s deploy manifests (if needed)**

Remove the `vol-observability` Deployment / Service from any ArgoCD or Kustomize manifests if it was deployed as a standalone service. If it was never deployed (it's a client-side binary), skip this step.

- [ ] **Step 3: Add Prometheus scrape annotations to agent-server Deployment**

Add annotations to the agent-server pod template:
```yaml
metadata:
  annotations:
    prometheus.io/scrape: "true"
    prometheus.io/path: "/metrics"
    prometheus.io/port: "3001"
```

(Or update the Prometheus scrape config directly — depends on how the cluster is set up.)

- [ ] **Step 4: Final commit**

```bash
git add docs/ deploy/ k8s/
git commit -m "docs: wiki-ingest observability refactor, add prometheus scrape annotations"
```