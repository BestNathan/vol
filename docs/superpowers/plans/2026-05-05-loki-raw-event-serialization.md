# Loki Raw Event Serialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Serialize full `AgentStreamEvent` JSON into Loki log lines instead of hand-assembled subsets, using the event's own timestamp.

**Architecture:** Add `Serialize` derive + `timestamp()` method to `AgentStreamEvent`, then simplify `LokiPlugin::create_loki_entry` to serialize the event directly and merge metadata fields. Remove unused `event_data` and `event_tool_name` helpers.

**Tech Stack:** Rust, serde, serde_json, chrono, vol-llm-observability, vol-llm-core

---

### Task 1: Add `Serialize` derive and `timestamp()` method to `AgentStreamEvent`

**Files:**
- Modify: `crates/vol-llm-core/src/stream.rs:70-71` (derive line)
- Modify: `crates/vol-llm-core/src/stream.rs` (add `timestamp()` method to impl block)

- [ ] **Step 1: Add `Serialize` derive**

Change line 70 from:
```rust
#[derive(Debug, Clone)]
```
to:
```rust
#[derive(Debug, Clone, Serialize)]
```

- [ ] **Step 2: Add `timestamp()` helper method**

At the end of `impl AgentStreamEvent` (after `plugin_event`, before `#[cfg(test)]`), add:

```rust
impl AgentStreamEvent {
    // ... existing methods ...

    /// Extract the timestamp from any event variant.
    pub fn timestamp(&self) -> chrono::DateTime<chrono::Utc> {
        match self {
            Self::AgentStart { timestamp, .. } => *timestamp,
            Self::AgentComplete { timestamp, .. } => *timestamp,
            Self::AgentAborted { timestamp, .. } => *timestamp,
            Self::MaxIterationsReached { timestamp, .. } => *timestamp,
            Self::IterationContinued { timestamp, .. } => *timestamp,
            Self::LLMCallStart { timestamp, .. } => *timestamp,
            Self::LLMCallComplete { timestamp, .. } => *timestamp,
            Self::LLMCallError { timestamp, .. } => *timestamp,
            Self::ThinkingStart { timestamp, .. } => *timestamp,
            Self::ThinkingDelta { timestamp, .. } => *timestamp,
            Self::ThinkingComplete { timestamp, .. } => *timestamp,
            Self::ContentStart { timestamp, .. } => *timestamp,
            Self::ContentDelta { timestamp, .. } => *timestamp,
            Self::ContentComplete { timestamp, .. } => *timestamp,
            Self::ToolCallBegin { timestamp, .. } => *timestamp,
            Self::ToolCallComplete { timestamp, .. } => *timestamp,
            Self::ToolCallError { timestamp, .. } => *timestamp,
            Self::ToolCallSkipped { timestamp, .. } => *timestamp,
            Self::ToolCallArgumentDelta { timestamp, .. } => *timestamp,
            Self::IterationComplete { timestamp, .. } => *timestamp,
            Self::PluginEvent { timestamp, .. } => *timestamp,
        }
    }
}
```

Note: this impl block already exists (line 194), so append the method before the `#[cfg(test)]` block at line 268.

- [ ] **Step 3: Verify compile**

Run: `cargo check -p vol-llm-core`
Expected: no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-core/src/stream.rs
git commit -m "feat: add Serialize derive and timestamp() helper to AgentStreamEvent"
```

---

### Task 2: Simplify `LokiPlugin::create_loki_entry` — serialize full event

**Files:**
- Modify: `crates/vol-llm-observability/src/loki/plugin.rs:61-109` (rewrite `create_loki_entry`)
- Modify: `crates/vol-llm-observability/src/loki/plugin.rs:140-189` (remove `event_data` function)
- Modify: `crates/vol-llm-observability/src/loki/plugin.rs:192-200` (remove `event_tool_name` function)

- [ ] **Step 1: Rewrite `create_loki_entry`**

Replace the entire `create_loki_entry` method (lines 61-109) with:

```rust
    /// Convert an event to a Loki entry with full event serialization.
    pub fn create_loki_entry(event: &AgentStreamEvent, ctx: &RunContext) -> LokiEntry {
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());
        let run_id = &ctx.run_id;
        let session_id = &ctx.session_id;

        let labels = LokiLabels::new(agent_type, agent_id);
        let mut labels = labels.into_inner();

        // Extract model for label if available.
        if let AgentStreamEvent::LLMCallComplete { model, .. } = event {
            labels.insert("model".to_string(), model.clone());
        }

        // Serialize the full event to JSON, then merge metadata fields.
        let mut line_map = match serde_json::to_value(event) {
            Ok(Value::Object(mut map)) => {
                map.insert("run_id".to_string(), json!(run_id));
                map.insert("session_id".to_string(), json!(session_id));
                map.insert("agent_id".to_string(), json!(agent_id));
                map
            }
            _ => {
                // Fallback if serialization fails.
                let mut map = serde_json::Map::new();
                map.insert("event".to_string(), json!(event_name(event)));
                map.insert("run_id".to_string(), json!(run_id));
                map.insert("session_id".to_string(), json!(session_id));
                map.insert("agent_id".to_string(), json!(agent_id));
                map
            }
        };

        let line = json!(line_map).to_string();

        let timestamp_nanos = event.timestamp().timestamp_nanos_opt().unwrap_or(0);

        LokiEntry {
            timestamp_nanos,
            line,
            labels,
        }
    }
```

- [ ] **Step 2: Remove `event_data` function**

Delete the entire `event_data` function (lines 140-189).

- [ ] **Step 3: Remove `event_tool_name` function**

Delete the entire `event_tool_name` function (lines 192-200).

- [ ] **Step 4: Clean up unused imports**

Remove `Value` from the `use serde_json::{json, Value};` import — it's still used in the fallback path, so keep it. Actually `Value` is still used in the match, so no change needed.

- [ ] **Step 5: Verify compile**

Run: `cargo check -p vol-llm-observability`
Expected: no errors (may have unused import warning for `Utc` if `plugin.rs` was using it only in `create_loki_entry`)

If `Utc` import is now unused (since we use `event.timestamp()` instead of `Utc::now()`), remove it from the `use chrono::Utc;` line.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-observability/src/loki/plugin.rs
git commit -m "refactor: serialize full event JSON in Loki log lines, remove event_data helper"
```

---

### Task 3: Update tests in `plugin.rs`

**Files:**
- Modify: `crates/vol-llm-observability/src/loki/plugin.rs:229-399` (update tests)

- [ ] **Step 1: Rewrite tests to match new serialization format**

Replace the entire `#[cfg(test)] mod tests` block (lines 229-399) with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Map;

    fn make_test_context(run_id: &str, session_id: &str, agent_name: &str, agent_type: &str) -> RunContext {
        use vol_llm_agent::agent_def::AgentDef;
        use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
        use vol_llm_context::ContextBuilderBuilder;
        use vol_session::{InMemoryEntryStore, Session};
        use vol_llm_tool::ToolRegistry;

        let def = AgentDef::new(agent_name, "prompt").with_type(agent_type);
        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
        let tools = Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: Some(def),
            llm: Arc::new(DummyLlm),
            tools: tools.clone(),
            session: session.clone(),
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
        };

        let (ctx, _rx) = RunContext::new(
            run_id.to_string(),
            "test input".to_string(),
            session_id.to_string(),
            session,
            tools,
            config,
            20,
        );
        ctx
    }

    struct DummyLlm;
    #[async_trait::async_trait]
    impl vol_llm_core::LLMClient for DummyLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
        fn model(&self) -> &str { "test" }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
        async fn converse(&self, _: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> { unimplemented!() }
        async fn converse_stream(&self, _: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> { unimplemented!() }
    }

    #[tokio::test]
    async fn test_plugin_id() {
        let config = LokiConfig::with_url("http://loki:3100".to_string());
        let plugin = LokiPlugin::new(config);
        assert_eq!(plugin.id(), "loki");
    }

    #[tokio::test]
    async fn test_plugin_priority() {
        let config = LokiConfig::with_url("http://loki:3100".to_string());
        let plugin = LokiPlugin::new(config);
        assert_eq!(plugin.priority(), 20);
    }

    #[test]
    fn test_should_send_skips_delta_events() {
        assert!(!LokiPlugin::should_send(&AgentStreamEvent::ThinkingDelta {
            timestamp: Utc::now(),
            delta: "chunk".to_string(),
        }));
        assert!(!LokiPlugin::should_send(&AgentStreamEvent::ContentDelta {
            timestamp: Utc::now(),
            delta: "partial".to_string(),
        }));
        assert!(!LokiPlugin::should_send(&AgentStreamEvent::ToolCallArgumentDelta {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            delta: "arg".to_string(),
        }));
        assert!(LokiPlugin::should_send(&AgentStreamEvent::ThinkingStart {
            timestamp: Utc::now(),
        }));
        assert!(LokiPlugin::should_send(&AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        }));
    }

    #[test]
    fn test_loki_entry_tool_call_has_all_fields() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        // Full event serialization includes all fields.
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        assert_eq!(parsed["event"], "ToolCallBegin");
        assert_eq!(parsed["tool_call_id"], "c1");
        assert_eq!(parsed["tool_name"], "bash");
        assert_eq!(parsed["arguments"], "{}");
        assert_eq!(parsed["run_id"], "run-1");
        assert_eq!(parsed["session_id"], "sess-1");
        assert_eq!(parsed["agent_id"], "agent-001");
        // Event's own timestamp should be present.
        assert!(parsed.get("timestamp").is_some());
    }

    #[test]
    fn test_loki_entry_llm_complete_includes_model_label() {
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: Utc::now(),
            model: "qwen3.5-plus".to_string(),
            usage: None,
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.labels.contains_key("model"));
        assert_eq!(entry.labels["model"], "qwen3.5-plus");
        // Full event serialization includes model in line too.
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        assert_eq!(parsed["model"], "qwen3.5-plus");
    }

    #[test]
    fn test_loki_entry_plugin_event() {
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        assert_eq!(parsed["event"], "PluginEvent");
        assert_eq!(parsed["name"], "my_plugin");
    }

    #[test]
    fn test_loki_entry_fallback_no_agent_def() {
        use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
        use vol_llm_context::ContextBuilderBuilder;
        use vol_session::{InMemoryEntryStore, Session};
        use vol_llm_tool::ToolRegistry;

        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
        let tools = Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: None,
            llm: Arc::new(DummyLlm),
            tools: tools.clone(),
            session: session.clone(),
            sandbox: None,
            context_builder,
            plugin_registry: PluginRegistry::new(),
        };

        let (ctx, _rx) = RunContext::new(
            "run-x".to_string(),
            "test".to_string(),
            "sess-x".to_string(),
            session,
            tools,
            config,
            20,
        );

        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.labels.contains_key("agent"));
        assert_eq!(entry.labels["agent"], "unknown");
        assert_eq!(entry.labels["agent_id"], "unknown");
    }

    #[test]
    fn test_loki_entry_timestamp_uses_event_timestamp() {
        use chrono::TimeZone;
        let fixed_ts = Utc.with_ymd_and_hms(2026, 1, 15, 10, 30, 0).unwrap();
        let event = AgentStreamEvent::AgentStart {
            timestamp: fixed_ts,
            input: "test".to_string(),
        };
        let ctx = make_test_context("r1", "s1", "a1", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        // The LokiEntry timestamp_nanos should match the event's timestamp.
        let expected_nanos = fixed_ts.timestamp_nanos_opt().unwrap();
        assert_eq!(entry.timestamp_nanos, expected_nanos);
    }

    #[test]
    fn test_loki_entry_llm_call_start_includes_messages() {
        use vol_llm_core::Message;
        let event = AgentStreamEvent::LLMCallStart {
            timestamp: Utc::now(),
            iteration: 1,
            messages: vec![Message {
                role: "user".to_string(),
                content: "hello".to_string(),
            }],
        };
        let ctx = make_test_context("r1", "s1", "a1", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        let parsed: Value = serde_json::from_str(&entry.line).unwrap();
        // Messages array should be present (previously dropped).
        assert!(parsed.get("messages").is_some());
        assert!(parsed["messages"].as_array().unwrap().len() > 0);
    }
}
```

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-observability loki -- --nocapture`
Expected: all 9 tests pass

- [ ] **Step 3: Run full test suite to check no regressions**

Run: `cargo test -p vol-llm-core -p vol-llm-observability -- 2>&1 | tail -20`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-observability/src/loki/plugin.rs
git commit -m "test: update Loki plugin tests for full event serialization"
```

---

### Task 4: Verify example still works

**Files:**
- No file changes (verify only)

- [ ] **Step 1: Check example compiles**

Run: `cargo check --example agent_loki_example`
Expected: no errors

- [ ] **Step 2: Commit (if any changes from rebase)**

No commit needed unless the example was modified by earlier steps.
