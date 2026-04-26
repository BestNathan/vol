# LoggerPlugin Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace monolithic `ObservabilityPlugin` with a focused `LoggerPlugin` that writes all agent events to JSONL files.

**Architecture:** `LoggerPlugin` implements `AgentPlugin`, listens to all `AgentStreamEvent` variants, and writes structured `LogEntry` JSONL to `{base_dir}/logs/{run_id}.jsonl`. `PluginEvent` events are routed to `{base_dir}/logs/{plugin_name}/{run_id}.jsonl`. All metrics/tracing/config code is deleted.

**Tech Stack:** Rust, cargo, tokio, serde_json, chrono, tempfile

---

### Task 1: Write LoggerPlugin and LogEntry

**Files:**
- Modify: `crates/vol-llm-observability/src/plugin.rs`
- Modify: `crates/vol-llm-observability/src/run_log/logger.rs`

- [ ] **Step 1: Rewrite `run_log/logger.rs`**

Replace the entire file with:

```rust
//! LogEntry and file append utilities for LoggerPlugin.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,
    pub data: Value,
}

impl LogEntry {
    pub fn to_json_line(&self) -> String {
        json!({
            "timestamp": self.timestamp.to_rfc3339(),
            "run_id": self.run_id,
            "agent_id": self.agent_id,
            "event": self.event,
            "data": self.data,
        }).to_string()
    }

    pub fn format_event_summary(&self) -> String {
        match self.event.as_str() {
            "AgentStart" => format!("Agent started - input: {:?}", self.data.get("input").and_then(|v| v.as_str()).unwrap_or("")),
            "ThinkingComplete" => "Thinking complete".to_string(),
            "ToolCallBegin" => format!("Tool call: {}", self.data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("unknown")),
            "ToolCallComplete" => format!("Tool result: {}", self.data.get("result").and_then(|v| v.as_str()).unwrap_or("")),
            "IterationComplete" => format!("Iteration {} complete", self.data.get("iteration").and_then(|v| v.as_u64()).unwrap_or(0)),
            "AgentComplete" => "Agent completed".to_string(),
            "AgentAborted" => format!("Agent aborted: {}", self.data.get("reason").and_then(|v| v.as_str()).unwrap_or("unknown")),
            "PluginEvent" => format!("Plugin event: {}", self.data.get("name").and_then(|v| v.as_str()).unwrap_or("unknown")),
            _ => self.event.clone(),
        }
    }
}

/// Append a line to a file, creating it if it doesn't exist.
/// Creates parent directories as needed.
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
    file.write_all(line.as_bytes()).await?;
    file.write_all(b"\n").await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_log_entry_serialization() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "r1".to_string(),
            agent_id: "a1".to_string(),
            event: "AgentStart".to_string(),
            data: json!({"input": "hello"}),
        };
        let line = entry.to_json_line();
        assert!(line.contains("AgentStart"));
        assert!(line.contains("r1"));
        assert!(line.contains("a1"));
        assert!(line.contains("hello"));
    }

    #[test]
    fn test_format_event_summary() {
        let entry = LogEntry {
            timestamp: Utc::now(),
            run_id: "r1".to_string(),
            agent_id: "a1".to_string(),
            event: "ToolCallBegin".to_string(),
            data: json!({"tool_name": "bash"}),
        };
        assert!(entry.format_event_summary().contains("bash"));
    }

    #[tokio::test]
    async fn test_append_log_creates_dirs_and_writes() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("logs/subdir/test.jsonl");
        append_log(&path, "hello").await.unwrap();
        assert!(path.exists());
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.trim(), "hello");
    }

    #[tokio::test]
    async fn test_append_log_appends() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("logs/test.jsonl");
        append_log(&path, "line1").await.unwrap();
        append_log(&path, "line2").await.unwrap();
        let content = std::fs::read_to_string(&path).unwrap();
        assert_eq!(content.lines().count(), 2);
    }
}
```

- [ ] **Step 2: Rewrite `plugin.rs`**

Replace the entire file with:

```rust
//! LoggerPlugin - Writes agent events to JSONL files.

use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use serde_json::{json, Value};
use vol_llm_core::plugin::{AgentPlugin, PluginContext, PluginDecision};
use vol_llm_core::stream::AgentStreamEvent;

use crate::run_log::logger::{LogEntry, append_log};

/// Writes all agent events to JSONL files.
///
/// File layout:
///   {base_dir}/logs/{run_id}.jsonl          (regular events)
///   {base_dir}/logs/{plugin_name}/{run_id}.jsonl  (PluginEvent)
pub struct LoggerPlugin {
    base_dir: PathBuf,
}

impl LoggerPlugin {
    pub fn new(base_dir: PathBuf) -> Self {
        let logs_dir = base_dir.join("logs");
        if let Err(e) = std::fs::create_dir_all(&logs_dir) {
            tracing::warn!(error = %e, "Failed to create logs directory");
        }
        Self { base_dir }
    }

    fn log_path(&self, event: &AgentStreamEvent, run_id: &str) -> PathBuf {
        match event {
            AgentStreamEvent::PluginEvent { name, .. } => {
                self.base_dir.join("logs").join(name).join(format!("{run_id}.jsonl"))
            }
            _ => self.base_dir.join("logs").join(format!("{run_id}.jsonl")),
        }
    }

    fn create_log_entry(event: &AgentStreamEvent, run_id: &str, agent_id: &str) -> LogEntry {
        let data = match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                json!({ "input": input })
            }
            AgentStreamEvent::AgentComplete { response, .. } => {
                json!({ "response": response })
            }
            AgentStreamEvent::AgentAborted { reason, .. } => {
                json!({ "reason": reason })
            }
            AgentStreamEvent::LLMCallStart { iteration, messages, .. } => {
                json!({ "iteration": iteration, "message_count": messages.len() })
            }
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
                json!({ "model": model, "usage": usage })
            }
            AgentStreamEvent::LLMCallError { error, .. } => {
                json!({ "error": error })
            }
            AgentStreamEvent::ThinkingStart { .. } => {
                json!({})
            }
            AgentStreamEvent::ThinkingDelta { delta, .. } => {
                json!({ "delta": delta })
            }
            AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                json!({ "thinking": thinking })
            }
            AgentStreamEvent::ContentStart { .. } => {
                json!({})
            }
            AgentStreamEvent::ContentDelta { delta, .. } => {
                json!({ "delta": delta })
            }
            AgentStreamEvent::ContentComplete { content, .. } => {
                json!({ "content": content })
            }
            AgentStreamEvent::ToolCallBegin { tool_call_id, tool_name, arguments, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "arguments": arguments })
            }
            AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "result": result, "duration_ms": duration_ms })
            }
            AgentStreamEvent::ToolCallError { tool_call_id, tool_name, error, duration_ms, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "error": error, "duration_ms": duration_ms })
            }
            AgentStreamEvent::ToolCallSkipped { tool_call_id, tool_name, reason, duration_ms, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "reason": reason, "duration_ms": duration_ms })
            }
            AgentStreamEvent::ToolCallArgumentDelta { tool_call_id, tool_name, delta, .. } => {
                json!({ "tool_call_id": tool_call_id, "tool_name": tool_name, "delta": delta })
            }
            AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer, .. } => {
                let tc: Vec<Value> = tool_calls.iter().map(|tc| {
                    json!({
                        "id": &tc.id,
                        "name": &tc.name,
                        "arguments": &tc.arguments,
                        "type": &tc.r#type,
                    })
                }).collect();
                json!({ "iteration": iteration, "tool_calls": tc, "final_answer": final_answer })
            }
            AgentStreamEvent::PluginEvent { name, data, .. } => {
                let mut map = serde_json::Map::new();
                map.insert("name".to_string(), Value::String(name.clone()));
                for (k, v) in data {
                    map.insert(k.clone(), v.clone());
                }
                Value::Object(map)
            }
            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                json!({ "current_iteration": current_iteration, "max_iterations": max_iterations })
            }
            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                json!({ "from_iteration": from_iteration })
            }
        };

        let event_name = event_name(event);
        LogEntry {
            timestamp: Utc::now(),
            run_id: run_id.to_string(),
            agent_id: agent_id.to_string(),
            event: event_name,
            data,
        }
    }
}

fn event_name(event: &AgentStreamEvent) -> String {
    match event {
        AgentStreamEvent::AgentStart { .. } => "AgentStart".to_string(),
        AgentStreamEvent::AgentComplete { .. } => "AgentComplete".to_string(),
        AgentStreamEvent::AgentAborted { .. } => "AgentAborted".to_string(),
        AgentStreamEvent::LLMCallStart { .. } => "LLMCallStart".to_string(),
        AgentStreamEvent::LLMCallComplete { .. } => "LLMCallComplete".to_string(),
        AgentStreamEvent::LLMCallError { .. } => "LLMCallError".to_string(),
        AgentStreamEvent::ThinkingStart { .. } => "ThinkingStart".to_string(),
        AgentStreamEvent::ThinkingDelta { .. } => "ThinkingDelta".to_string(),
        AgentStreamEvent::ThinkingComplete { .. } => "ThinkingComplete".to_string(),
        AgentStreamEvent::ContentStart { .. } => "ContentStart".to_string(),
        AgentStreamEvent::ContentDelta { .. } => "ContentDelta".to_string(),
        AgentStreamEvent::ContentComplete { .. } => "ContentComplete".to_string(),
        AgentStreamEvent::ToolCallBegin { .. } => "ToolCallBegin".to_string(),
        AgentStreamEvent::ToolCallComplete { .. } => "ToolCallComplete".to_string(),
        AgentStreamEvent::ToolCallError { .. } => "ToolCallError".to_string(),
        AgentStreamEvent::ToolCallSkipped { .. } => "ToolCallSkipped".to_string(),
        AgentStreamEvent::ToolCallArgumentDelta { .. } => "ToolCallArgumentDelta".to_string(),
        AgentStreamEvent::IterationComplete { .. } => "IterationComplete".to_string(),
        AgentStreamEvent::PluginEvent { .. } => "PluginEvent".to_string(),
        AgentStreamEvent::MaxIterationsReached { .. } => "MaxIterationsReached".to_string(),
        AgentStreamEvent::IterationContinued { .. } => "IterationContinued".to_string(),
    }
}

#[async_trait]
impl AgentPlugin for LoggerPlugin {
    fn id(&self) -> String {
        "logger".to_string()
    }

    fn priority(&self) -> u32 {
        10
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        let entry = Self::create_log_entry(event, &ctx.run_id, "agent");
        let path = self.log_path(event, &ctx.run_id);
        let line = entry.to_json_line();
        if let Err(e) = append_log(&path, &line).await {
            tracing::warn!(path = %path.display(), error = %e, "Failed to write log entry");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;
    use tempfile::TempDir;

    fn create_test_plugin(temp_dir: &TempDir) -> LoggerPlugin {
        LoggerPlugin::new(temp_dir.path().to_path_buf())
    }

    fn create_test_context() -> PluginContext {
        use std::collections::HashMap;
        use std::sync::Arc;
        use tokio::sync::RwLock;
        PluginContext {
            run_id: "test-run".to_string(),
            user_input: "test input".to_string(),
            session_id: "session-1".to_string(),
            all_tool_calls: Arc::new(RwLock::new(Vec::new())),
            current_tool_calls: Arc::new(RwLock::new(Vec::new())),
            data: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    #[test]
    fn test_plugin_id() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        assert_eq!(plugin.id(), "logger");
    }

    #[test]
    fn test_plugin_priority() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        assert_eq!(plugin.priority(), 10);
    }

    #[test]
    fn test_log_path_regular_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        let path = plugin.log_path(&event, "run-1");
        assert_eq!(path, temp_dir.path().join("logs/run-1.jsonl"));
    }

    #[test]
    fn test_log_path_plugin_event() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let path = plugin.log_path(&event, "run-1");
        assert_eq!(path, temp_dir.path().join("logs/my_plugin/run-1.jsonl"));
    }

    #[test]
    fn test_log_entry_all_variants() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_context();

        let events = vec![
            AgentStreamEvent::AgentStart {
                timestamp: Utc::now(),
                input: "hello".to_string(),
            },
            AgentStreamEvent::AgentComplete {
                timestamp: Utc::now(),
                response: None,
            },
            AgentStreamEvent::AgentAborted {
                timestamp: Utc::now(),
                reason: "stop".to_string(),
            },
            AgentStreamEvent::LLMCallStart {
                timestamp: Utc::now(),
                iteration: 1,
                messages: vec![],
            },
            AgentStreamEvent::LLMCallComplete {
                timestamp: Utc::now(),
                model: "test".to_string(),
                usage: None,
            },
            AgentStreamEvent::LLMCallError {
                timestamp: Utc::now(),
                error: "timeout".to_string(),
            },
            AgentStreamEvent::ThinkingStart { timestamp: Utc::now() },
            AgentStreamEvent::ThinkingDelta {
                timestamp: Utc::now(),
                delta: "thinking...".to_string(),
            },
            AgentStreamEvent::ThinkingComplete {
                timestamp: Utc::now(),
                thinking: "done".to_string(),
            },
            AgentStreamEvent::ContentStart { timestamp: Utc::now() },
            AgentStreamEvent::ContentDelta {
                timestamp: Utc::now(),
                delta: "partial".to_string(),
            },
            AgentStreamEvent::ContentComplete {
                timestamp: Utc::now(),
                content: "final".to_string(),
            },
            AgentStreamEvent::ToolCallBegin {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                arguments: "{}".to_string(),
            },
            AgentStreamEvent::ToolCallComplete {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                result: "ok".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::ToolCallError {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                error: "fail".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::ToolCallSkipped {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                reason: "not allowed".to_string(),
                duration_ms: None,
            },
            AgentStreamEvent::ToolCallArgumentDelta {
                timestamp: Utc::now(),
                tool_call_id: "c1".to_string(),
                tool_name: "bash".to_string(),
                delta: "arg".to_string(),
            },
            AgentStreamEvent::IterationComplete {
                timestamp: Utc::now(),
                iteration: 1,
                tool_calls: vec![],
                final_answer: Some("done".to_string()),
            },
            AgentStreamEvent::PluginEvent {
                timestamp: Utc::now(),
                name: "custom".to_string(),
                data: {
                    let mut m = Map::new();
                    m.insert("k".to_string(), json!("v"));
                    m
                },
            },
            AgentStreamEvent::MaxIterationsReached {
                timestamp: Utc::now(),
                current_iteration: 10,
                max_iterations: 10,
            },
            AgentStreamEvent::IterationContinued {
                timestamp: Utc::now(),
                from_iteration: 11,
            },
        ];

        for event in events {
            let entry = LoggerPlugin::create_log_entry(&event, "run-1", "test-agent");
            let line = entry.to_json_line();
            let path = plugin.log_path(&event, "run-1");
            // Verify log_path is deterministic
            let path2 = plugin.log_path(&event, "run-1");
            assert_eq!(path, path2);
            // Verify JSON serialization works
            assert!(line.contains("run-1"));
            assert!(line.contains("test-agent"));
        }
    }

    #[tokio::test]
    async fn test_listen_writes_log_file() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_context();

        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let log_path = temp_dir.path().join("logs/test-run.jsonl");
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("AgentStart"));
    }

    #[tokio::test]
    async fn test_listen_writes_plugin_event_log() {
        let temp_dir = TempDir::new().unwrap();
        let plugin = create_test_plugin(&temp_dir);
        let ctx = create_test_context();

        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        plugin.listen(&event, &ctx).await;

        let log_path = temp_dir.path().join("logs/my_plugin/test-run.jsonl");
        assert!(log_path.exists());
        let content = std::fs::read_to_string(&log_path).unwrap();
        assert!(content.contains("PluginEvent"));
        assert!(content.contains("my_plugin"));
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-observability -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-observability/src/plugin.rs crates/vol-llm-observability/src/run_log/logger.rs
git commit -m "feat: replace ObservabilityPlugin with LoggerPlugin, rewrite LogEntry"
```

---

### Task 2: Delete obsolete modules and update lib.rs

**Files:**
- Delete: `crates/vol-llm-observability/src/config.rs`
- Delete: `crates/vol-llm-observability/src/metrics/`
- Delete: `crates/vol-llm-observability/src/tracing/`
- Delete: `crates/vol-llm-observability/src/run_log/cleanup.rs`
- Delete: `crates/vol-llm-observability/src/run_log/mod.rs`
- Modify: `crates/vol-llm-observability/src/lib.rs`
- Modify: `crates/vol-llm-observability/Cargo.toml`

- [ ] **Step 1: Delete obsolete files**

```bash
rm crates/vol-llm-observability/src/config.rs
rm -rf crates/vol-llm-observability/src/metrics/
rm -rf crates/vol-llm-observability/src/tracing/
rm crates/vol-llm-observability/src/run_log/cleanup.rs
rm crates/vol-llm-observability/src/run_log/mod.rs
```

- [ ] **Step 2: Update `lib.rs`**

Replace the entire file:

```rust
//! vol-llm-observability: Structured event logging for LLM agents.

pub mod run_log {
    pub mod logger;
}

pub use run_log::logger::LogEntry;
pub use plugin::LoggerPlugin;

mod plugin;
```

- [ ] **Step 3: Update `Cargo.toml`**

Remove `tempfile` from `[dev-dependencies]` (it's already in `run_log/logger.rs` tests). Remove `regex` from dependencies (no longer used). Final file:

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
vol-llm-core = { path = "../vol-llm-core" }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-observability -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-observability/
git commit -m "chore: remove metrics, tracing, config from observability crate"
```

---

### Task 3: Update `vol-llm-agent` observability re-exports

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/mod.rs`
- Modify: `crates/vol-llm-agent/src/observability/run_log/mod.rs`
- Modify: `crates/vol-llm-agent/Cargo.toml`

- [ ] **Step 1: Update `observability/mod.rs`**

Replace the entire file:

```rust
//! Observability re-exports from vol-llm-observability.
//!
//! LoggerPlugin lives in vol-llm-observability. This module provides
//! convenient re-exports for downstream crates.

pub use vol_llm_observability::LogEntry;
pub use vol_llm_observability::run_log::logger::LogEntry as RunLogLogEntry;
```

- [ ] **Step 2: Update `observability/run_log/mod.rs`**

Replace the entire file:

```rust
//! Run log re-exports.

pub use vol_llm_observability::run_log::logger::LogEntry;
pub use vol_llm_observability::run_log::logger::append_log;
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agent -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/observability/
git commit -m "refactor: update observability re-exports for LoggerPlugin"
```

---

### Task 4: Update callers in vol-llm-agents and vol-llm-tui

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/tests.rs` (if any ObservabilityPlugin refs)
- Modify: `crates/vol-llm-tui/src/main.rs` (if any ObservabilityPlugin refs)
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` (if any refs)

- [ ] **Step 1: Search for remaining references**

Run: `grep -rn "ObservabilityPlugin\|ObservabilityConfig\|MetricsCollector\|RunLogLogger" crates/vol-llm-agents/ crates/vol-llm-tui/ --include="*.rs"`

If any results, update each to use `LoggerPlugin` instead.

- [ ] **Step 2: Build the workspace**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 3: Run all tests**

Run: `cargo test -p vol-llm-observability -p vol-llm-agent -p vol-llm-agents -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/ crates/vol-llm-tui/
git commit -m "refactor: update all callers to use LoggerPlugin"
```
