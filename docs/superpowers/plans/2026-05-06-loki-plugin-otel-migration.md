# LokiPlugin → OTel SDK Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace LokiPlugin's direct HTTP POST to Loki with OTel SDK log export via `tracing::info!` macros and `opentelemetry-appender-tracing`.

**Architecture:** LokiPlugin becomes stateless, calling `tracing::info!` with structured fields. The `tracing-subscriber` stack, extended with `opentelemetry-appender-tracing` log layer, routes these logs through the OTel SDK to the OTel Collector. `RunContext` gains a `model` field.

**Tech Stack:** Rust, opentelemetry 0.27, opentelemetry-otlp 0.27, opentelemetry-appender-tracing 0.27, tracing, tracing-subscriber

---

### Task 1: Upgrade Workspace OTel Dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Update workspace OTel dependencies**

Replace lines 87-91 in workspace `Cargo.toml`:

```toml
# Before (remove these lines):
tracing-opentelemetry = "0.22"
rolling-file = "0.2"
opentelemetry = "0.21"
opentelemetry_sdk = { version = "0.21", features = ["tokio", "trace", "rt-tokio"] }
opentelemetry-otlp = { version = "0.14", features = ["tokio", "grpc-tonic"] }

# After (add these lines):
tracing-opentelemetry = "0.30"
rolling-file = "0.2"
opentelemetry = "0.27"
opentelemetry_sdk = { version = "0.27", features = ["tokio", "trace", "logs", "rt-tokio"] }
opentelemetry-otlp = { version = "0.27", features = ["tokio", "grpc-tonic", "logs"] }
opentelemetry-appender-tracing = "0.27"
```

- [ ] **Step 2: Verify dependency resolution**

Run: `cargo check --workspace 2>&1 | head -50`

Expected: May have compilation errors from API changes (expected, will fix in later tasks). Should NOT have dependency resolution errors. If there are resolution errors, adjust version numbers.

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml
git commit -m "chore: upgrade opentelemetry deps 0.21->0.27 for logs support"
```

---

### Task 2: Remove Loki HTTP Client Code

**Files:**
- Delete: `crates/vol-llm-observability/src/loki/client.rs`
- Delete: `crates/vol-llm-observability/src/loki/config.rs`
- Delete: `crates/vol-llm-observability/src/loki/labels.rs`
- Modify: `crates/vol-llm-observability/src/loki/mod.rs`
- Modify: `crates/vol-llm-observability/Cargo.toml`

- [ ] **Step 1: Delete Loki client files**

```bash
rm crates/vol-llm-observability/src/loki/client.rs
rm crates/vol-llm-observability/src/loki/config.rs
rm crates/vol-llm-observability/src/loki/labels.rs
```

- [ ] **Step 2: Update loki/mod.rs**

Replace the entire contents of `crates/vol-llm-observability/src/loki/mod.rs`:

```rust
//! Loki integration for agent observability.
//!
//! Provides `LokiPlugin` which implements `AgentPlugin` to send agent events
//! via `tracing::info!` structured logging. The tracing-subscriber stack,
//! extended with opentelemetry-appender-tracing, routes logs to the OTel Collector.

pub mod plugin;

pub use plugin::LokiPlugin;
```

- [ ] **Step 3: Update vol-llm-observability Cargo.toml**

Replace `crates/vol-llm-observability/Cargo.toml` dependencies section:

```toml
[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
chrono = { workspace = true }
opentelemetry = { workspace = true }
opentelemetry_sdk = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-agent = { path = "../vol-llm-agent" }

[dev-dependencies]
tracing-subscriber = "0.3"
vol-session = { path = "../vol-session" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-context = { path = "../vol-llm-context" }
```

Note: Removed `reqwest` and `tempfile` dependencies.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-observability/src/loki/mod.rs crates/vol-llm-observability/Cargo.toml
git add -u crates/vol-llm-observability/src/loki/
git commit -m "refactor: remove Loki HTTP client, config, labels — replaced by OTel SDK"
```

---

### Task 3: Rewrite LokiPlugin to Use tracing::info!

**Files:**
- Modify: `crates/vol-llm-observability/src/loki/plugin.rs`

- [ ] **Step 1: Rewrite plugin.rs**

Replace the entire contents of `crates/vol-llm-observability/src/loki/plugin.rs`:

```rust
//! LokiPlugin - Sends agent events to OTel Collector via tracing macros.
//!
//! Implements `AgentPlugin` to intercept agent run events and forward them
//! as structured logs via `tracing::info!`. The tracing-subscriber stack,
//! extended with opentelemetry-appender-tracing, routes logs to the OTel Collector.
//!
//! This plugin is stateless — it holds no endpoint, writer, or config.
//! It relies on the tracing layer being properly initialized (see vol-monitor tracing_setup).
//!
//! # Structured Fields
//!
//! Each log entry carries:
//! - `namespace`: `"agent"` (fixed)
//! - `session_id`: Session ID from RunContext
//! - `agent_id`: Agent instance name
//! - `agent_type`: Agent type (e.g., "coding", "qa")
//! - `run_id`: Run ID
//! - `model`: Model used for this run
//! - `event`: Full serialized AgentStreamEvent variant content

use async_trait::async_trait;
use serde_json::{json, Value};
use vol_llm_agent::react::{AgentPlugin, PluginDecision, RunContext};
use vol_llm_core::stream::AgentStreamEvent;

/// Plugin that sends agent events to OTel via tracing macros.
///
/// Stateless — no fields, no config. Clone is trivial.
pub struct LokiPlugin;

impl LokiPlugin {
    /// Create a new LokiPlugin.
    pub fn new() -> Self {
        Self
    }

    /// Whether an event should be sent to OTel.
    /// Skips high-frequency streaming delta events.
    pub fn should_send(event: &AgentStreamEvent) -> bool {
        !matches!(
            event,
            AgentStreamEvent::ThinkingDelta { .. }
                | AgentStreamEvent::ContentDelta { .. }
                | AgentStreamEvent::ToolCallArgumentDelta { .. }
        )
    }

    /// Convert an event to a flat JSON object with metadata.
    /// Same flattening logic as the previous Loki-based implementation.
    fn create_event_json(event: &AgentStreamEvent, ctx: &RunContext) -> String {
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());
        let run_id = &ctx.run_id;
        let session_id = &ctx.session_id;

        let line_map = match serde_json::to_value(event) {
            Ok(Value::Object(mut map)) => {
                if map.len() == 1 {
                    let (variant, fields) = map.iter().next().unwrap();
                    let mut flat = serde_json::Map::new();
                    flat.insert("event".to_string(), json!(variant));
                    if let Value::Object(fields) = fields {
                        for (k, v) in fields {
                            flat.insert(k.clone(), v.clone());
                        }
                    }
                    flat.insert("run_id".to_string(), json!(run_id));
                    flat.insert("session_id".to_string(), json!(session_id));
                    flat.insert("agent_id".to_string(), json!(agent_id));
                    flat
                } else {
                    map.insert("run_id".to_string(), json!(run_id));
                    map.insert("session_id".to_string(), json!(session_id));
                    map.insert("agent_id".to_string(), json!(agent_id));
                    map
                }
            }
            _ => {
                let mut map = serde_json::Map::new();
                map.insert("event".to_string(), json!(event.event_name()));
                map.insert("run_id".to_string(), json!(run_id));
                map.insert("session_id".to_string(), json!(session_id));
                map.insert("agent_id".to_string(), json!(agent_id));
                map
            }
        };

        json!(line_map).to_string()
    }
}

#[async_trait]
impl AgentPlugin for LokiPlugin {
    fn id(&self) -> String {
        "loki".to_string()
    }

    fn priority(&self) -> u32 {
        20
    }

    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if !Self::should_send(event) {
            return;
        }
        let event_json = Self::create_event_json(event, ctx);
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).map_or("unknown", |v| v.as_str());
        let agent_id = def.map(|d| &d.name).map_or("unknown", |v| v.as_str());

        tracing::info!(
            namespace = "agent",
            session_id = ctx.session_id,
            agent_id = agent_id,
            agent_type = agent_type,
            run_id = ctx.run_id,
            model = ctx.model,
            event = %event_json,
            "agent_event"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::Map;
    use vol_llm_agent::agent_def::AgentDef;
    use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
    use vol_llm_context::ContextBuilderBuilder;
    use vol_llm_core::LLMClient;
    use vol_llm_core::{ConversationRequest, ConversationResponse, LLMProvider, Result as LlmResult, StreamReceiver};
    use vol_session::{InMemoryEntryStore, Session};
    use vol_llm_tool::ToolRegistry;

    struct DummyLlm;
    #[async_trait::async_trait]
    impl LLMClient for DummyLlm {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "test" }
        fn supported_params(&self) -> &[] { &[] }
        async fn converse(&self, _: ConversationRequest) -> LlmResult<ConversationResponse> { unimplemented!() }
        async fn converse_stream(&self, _: ConversationRequest) -> LlmResult<StreamReceiver> { unimplemented!() }
    }

    fn make_test_context(run_id: &str, session_id: &str, agent_name: &str, agent_type: &str) -> RunContext {
        let def = AgentDef::new(agent_name, "prompt").with_type(agent_type);
        let session = std::sync::Arc::new(Session::new(std::sync::Arc::new(InMemoryEntryStore::new())));
        let tools = std::sync::Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: Some(def),
            llm: std::sync::Arc::new(DummyLlm),
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
            "test-model".to_string(),
        );
        ctx
    }

    #[tokio::test]
    async fn test_plugin_id() {
        let plugin = LokiPlugin::new();
        assert_eq!(plugin.id(), "loki");
    }

    #[tokio::test]
    async fn test_plugin_priority() {
        let plugin = LokiPlugin::new();
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
    fn test_event_json_tool_call_has_all_fields() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let json = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["event"], "ToolCallBegin");
        assert_eq!(parsed["tool_call_id"], "c1");
        assert_eq!(parsed["tool_name"], "bash");
        assert_eq!(parsed["arguments"], "{}");
        assert_eq!(parsed["run_id"], "run-1");
        assert_eq!(parsed["session_id"], "sess-1");
        assert_eq!(parsed["agent_id"], "agent-001");
    }

    #[test]
    fn test_event_json_llm_complete_includes_model() {
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: Utc::now(),
            model: "qwen3.5-plus".to_string(),
            usage: None,
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let _json = LokiPlugin::create_event_json(&event, &ctx);
        // Model is now from RunContext, not the event.
        // The event still carries its own model field; both are fine.
    }

    #[test]
    fn test_event_json_plugin_event() {
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let json = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["event"], "PluginEvent");
        assert_eq!(parsed["name"], "my_plugin");
    }

    #[test]
    fn test_event_json_fallback_no_agent_def() {
        let session = std::sync::Arc::new(Session::new(std::sync::Arc::new(InMemoryEntryStore::new())));
        let tools = std::sync::Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: None,
            llm: std::sync::Arc::new(DummyLlm),
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
            "test-model".to_string(),
        );

        let event = AgentStreamEvent::AgentStart {
            timestamp: Utc::now(),
            input: "hello".to_string(),
        };
        let json = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["agent_id"], "unknown");
    }

    #[test]
    fn test_event_json_llm_call_start_includes_messages() {
        use vol_llm_core::{Message, message::{MessageContent, MessageRole}};
        let event = AgentStreamEvent::LLMCallStart {
            timestamp: Utc::now(),
            iteration: 1,
            messages: vec![Message {
                role: MessageRole::User,
                content: Some(MessageContent::Text("hello".to_string())),
                tool_calls: None,
                tool_call_id: None,
                name: None,
                thinking: None,
            }],
        };
        let ctx = make_test_context("r1", "s1", "a1", "coding");
        let json = LokiPlugin::create_event_json(&event, &ctx);
        let parsed: Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.get("messages").is_some());
        assert!(parsed["messages"].as_array().unwrap().len() > 0);
    }
}
```

- [ ] **Step 2: Verify compilation for this crate**

Run: `cargo check -p vol-llm-observability 2>&1 | tail -20`

Expected: Will fail because `RunContext::new` doesn't yet have the `model` parameter (will fix in Task 4).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-observability/src/loki/plugin.rs
git commit -m "refactor: rewrite LokiPlugin to use tracing::info! instead of HTTP POST"
```

---

### Task 4: Add model Field to RunContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`

- [ ] **Step 1: Add model field to RunContext struct**

In `crates/vol-llm-agent/src/react/run_context.rs`, add `model` field to the struct (after `session_id`):

```rust
pub struct RunContext {
    // Immutable fields
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    /// Model used for this run, from LLM config.
    pub model: String,
    // ... rest unchanged ...
```

- [ ] **Step 2: Update RunContext::new() signature**

Add `model: String` parameter and normalize empty to "unknown":

```rust
    pub fn new(
        run_id: String,
        user_input: String,
        session_id: String,
        session: Arc<Session>,
        tools: Arc<ToolRegistry>,
        config: AgentConfig,
        max_history_messages: usize,
        model: String,
    ) -> (Self, mpsc::Receiver<PluginRequest>) {
```

In the struct initialization inside `new()`, add:

```rust
        let ctx = Self {
            run_id,
            user_input,
            session_id,
            model: if model.is_empty() { "unknown".to_string() } else { model },
            // ... rest unchanged ...
```

- [ ] **Step 3: Update Clone impl**

In the `impl Clone for RunContext`, add `model` field:

```rust
    fn clone(&self) -> Self {
        Self {
            run_id: self.run_id.clone(),
            user_input: self.user_input.clone(),
            session_id: self.session_id.clone(),
            model: self.model.clone(),
            // ... rest unchanged ...
```

- [ ] **Step 4: Update all test helpers in run_context.rs**

Every `RunContext::new(...)` call in the `#[cfg(test)] mod tests` section needs `"test-model".to_string()` appended. There are approximately 12 calls. Update each one:

```rust
// Example pattern for each call:
let (ctx, _rx) = RunContext::new(
    "test-run".to_string(),
    "test input".to_string(),
    "session-1".to_string(),
    Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
    Arc::new(vol_llm_tool::ToolRegistry::new()),
    AgentConfig::default(),
    20,
    "test-model".to_string(),  // <-- ADD THIS
);
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs
git commit -m "feat: add model field to RunContext"
```

---

### Task 5: Update agent.rs to Pass model to RunContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Pass model into RunContext::new()**

In `crates/vol-llm-agent/src/react/agent.rs`, around line 212, update the `RunContext::new()` call:

```rust
        let (run_ctx, plugin_rx) = RunContext::new(
            run_id.clone(),
            user_input.to_string(),
            self.config.session.id.clone(),
            session.clone(),
            effective_tools,
            config.clone(),
            max_history_messages,
            self.config.llm.model().to_string(),  // <-- ADD THIS
        );
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: pass model from LLM config into RunContext"
```

---

### Task 6: Update All External Callers of RunContext::new()

**Files:**
- Modify: `crates/vol-llm-agents/tests/observer_plugin_unit.rs`
- Modify: `crates/vol-llm-agents/tests/agent_loki_integration.rs`
- Modify: `crates/vol-llm-agent/src/plugins/caching.rs`
- Modify: `crates/vol-llm-agent/src/plugins/rate_limiter.rs`
- Modify: `crates/vol-llm-agent/src/plugins/retry.rs`
- Modify: `crates/vol-llm-agent/src/react/tests.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_flow_test.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_test.rs`
- Modify: `crates/vol-llm-observability/src/plugin.rs`

- [ ] **Step 1: Add model parameter to all RunContext::new() calls**

For each file, append `"test-model".to_string()` (or appropriate model) to every `RunContext::new(...)` call. Each call currently has 7 arguments; add the 8th `model` argument.

Example pattern for each file:

```rust
// Before:
let (ctx, _rx) = RunContext::new(
    run_id, user_input, session_id, session, tools, config, max_history,
);

// After:
let (ctx, _rx) = RunContext::new(
    run_id, user_input, session_id, session, tools, config, max_history,
    "test-model".to_string(),
);
```

Files and approximate line numbers:
- `vol-llm-agents/tests/observer_plugin_unit.rs:124`
- `vol-llm-agents/tests/agent_loki_integration.rs:148`
- `vol-llm-agent/src/plugins/caching.rs:154`
- `vol-llm-agent/src/plugins/rate_limiter.rs:59`
- `vol-llm-agent/src/plugins/retry.rs:76`
- `vol-llm-agent/src/react/tests.rs:260, 306, 339`
- `vol-llm-agent/tests/plugin_flow_test.rs:318`
- `vol-llm-agent/tests/plugin_test.rs:64`
- `vol-llm-observability/src/plugin.rs:227`

- [ ] **Step 2: Verify workspace compiles**

Run: `cargo check --workspace 2>&1 | tail -30`

Expected: No compilation errors. If there are errors, fix them (likely more RunContext::new() callers we missed).

- [ ] **Step 3: Run tests**

Run: `cargo test --workspace 2>&1 | tail -40`

Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/tests/observer_plugin_unit.rs
git add crates/vol-llm-agents/tests/agent_loki_integration.rs
git add crates/vol-llm-agent/src/plugins/caching.rs
git add crates/vol-llm-agent/src/plugins/rate_limiter.rs
git add crates/vol-llm-agent/src/plugins/retry.rs
git add crates/vol-llm-agent/src/react/tests.rs
git add crates/vol-llm-agent/tests/plugin_flow_test.rs
git add crates/vol-llm-agent/tests/plugin_test.rs
git add crates/vol-llm-observability/src/plugin.rs
git commit -m "refactor: update all RunContext::new() callers with model parameter"
```

---

### Task 7: Update vol-llm-agents CodingAgent Builder and Examples

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-agents/examples/agent_loki_example.rs`
- Modify: `crates/vol-llm-agents/tests/agent_loki_integration.rs`

- [ ] **Step 1: Update CodingAgent.with_loki() builder method**

In `crates/vol-llm-agents/src/coding/agent.rs`, replace the `with_loki()` method (around line 355-366):

```rust
    /// Register LokiPlugin to send agent events to OTel via tracing.
    ///
    /// LokiPlugin is stateless — no configuration needed. The OTel
    /// collector endpoint is set via OTEL_EXPORTER_OTLP_ENDPOINT env var.
    pub fn with_loki(mut self) -> Self {
        let plugin = vol_llm_observability::loki::LokiPlugin::new();
        self.config.plugin_registry.register(plugin);
        self
    }
```

- [ ] **Step 2: Update agent_loki_example.rs**

Replace `crates/vol-llm-agents/examples/agent_loki_example.rs` Loki-related sections. Change:

```rust
// Before (remove):
use vol_llm_observability::loki::{LokiConfig, LokiPlugin};
// ...
let loki_config = LokiConfig::with_url(loki_url.clone());
let loki_plugin = LokiPlugin::new(loki_config);

// After (replace with):
use vol_llm_observability::loki::LokiPlugin;
// ...
let loki_plugin = LokiPlugin::new();
```

Update the printed messages to reflect OTel instead of direct Loki:

```rust
// Change "LokiPlugin registered (Loki URL: ...)" to:
println!("  ✓ LokiPlugin registered (OTel logs via tracing)");

// Change "Loki entries were sent to: {}" section to:
println!("Logs are sent to OTel Collector via tracing layer.");
println!("Log labels:");
println!("  - namespace: agent");
println!("  - agent: k8s_ops_agent");
println!("  - agent_id: k8s_ops_agent");
println!("  - model: qwen3.5-plus");
```

Remove the LOKI_URL env var check and default:

```rust
// Remove this block:
let loki_url = std::env::var("LOKI_URL")
    .unwrap_or_else(|_| "http://localhost:3100".to_string());
// ...
println!("  ✓ LOKI_URL = {}", loki_url);
```

- [ ] **Step 3: Update agent_loki_integration.rs**

In `crates/vol-llm-agents/tests/agent_loki_integration.rs`, update imports and usage:

```rust
// Before (remove):
use vol_llm_observability::loki::{LokiConfig, LokiPlugin};

// After (replace with):
use vol_llm_observability::loki::LokiPlugin;
```

Update the test that creates LokiPlugin:

```rust
// Before:
let loki_config = LokiConfig::with_url("http://loki:3100".to_string());
let loki_plugin = LokiPlugin::new(loki_config);

// After:
let loki_plugin = LokiPlugin::new();
```

Remove the `LokiPlugin::create_loki_entry()` call (this method no longer exists). Replace with verifying that the event JSON is correctly constructed:

```rust
// The integration test should verify the agent runs successfully
// with LokiPlugin registered. The actual log output verification
// is done in vol-llm-observability unit tests.
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git add crates/vol-llm-agents/examples/agent_loki_example.rs
git add crates/vol-llm-agents/tests/agent_loki_integration.rs
git commit -m "refactor: update CodingAgent builder and examples for stateless LokiPlugin"
```

---

### Task 8: Add OTel Log Initialization in vol-monitor

**Files:**
- Modify: `crates/vol-monitor/src/tracing_setup.rs`
- Modify: `crates/vol-monitor/Cargo.toml`

- [ ] **Step 1: Add opentelemetry-appender-tracing to vol-monitor Cargo.toml**

In `crates/vol-monitor/Cargo.toml`, add to `[dependencies]`:

```toml
opentelemetry-appender-tracing = { workspace = true }
```

- [ ] **Step 2: Add init_otel_logs() function to tracing_setup.rs**

Add this new function to `crates/vol-monitor/src/tracing_setup.rs`:

```rust
use opentelemetry_appender_tracing::layer;
use opentelemetry_sdk::{logs::LoggerProvider, Resource};

/// Initialize OTel log exporter.
///
/// Reads OTEL_EXPORTER_OTLP_ENDPOINT from env (fallback to config endpoint).
/// Creates a LoggerProvider with BatchLogProcessor and sets up the
/// opentelemetry-appender-tracing layer.
pub fn init_otel_logs(
    config: &OpenTelemetryConfig,
) -> Result<layer::OpenTelemetryTracingBridge<opentelemetry_sdk::logs::LoggerProvider>, Box<dyn std::error::Error + Send + Sync>> {
    use opentelemetry_otlp::WithExportConfig;

    let endpoint = std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT")
        .unwrap_or_else(|_| config.endpoint.clone());

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| config.service_name.clone());

    let resource = Resource::new(vec![
        opentelemetry::KeyValue::new("service.name", service_name.clone()),
        opentelemetry::KeyValue::new("service.namespace", config.service_namespace.clone()),
        opentelemetry::KeyValue::new("deployment.environment", config.deployment_environment.clone()),
    ]);

    let exporter = opentelemetry_otlp::new_exporter()
        .tonic()
        .with_endpoint(&endpoint)
        .with_timeout(std::time::Duration::from_millis(config.batch.max_export_timeout_millis))
        .build_log_exporter()?;

    let logger_provider = LoggerProvider::builder()
        .with_resource(resource)
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build();

    // Set as global provider for shutdown
    // global::set_logger_provider(logger_provider.clone());

    let otel_layer = layer::OpenTelemetryTracingBridge::new(logger_provider);

    tracing::info!(
        "OpenTelemetry logs enabled: endpoint={} service={}",
        endpoint,
        service_name
    );

    Ok(otel_layer)
}
```

- [ ] **Step 3: Integrate OTel log layer into init()**

In the existing `init()` function, call `init_otel_logs()` and add the layer to the subscriber. Modify both the OTel-enabled and non-OTel branches:

```rust
// In the OTel-enabled branch (around line 127):
let otel_log_layer = if config.opentelemetry.enabled {
    Some(init_otel_logs(&config.opentelemetry).ok())
} else {
    None
};

// Then add to subscriber:
let subscriber = Registry::default()
    .with(env_filter)
    .with(console_layer)
    .with(file_layer)
    .with(otel_layer)  // existing trace layer
    .with(otel_log_layer);  // <-- NEW: log layer
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-monitor/src/tracing_setup.rs crates/vol-monitor/Cargo.toml
git commit -m "feat: add init_otel_logs() for OTel log export via tracing layer"
```

---

### Task 9: Full Workspace Build and Test

**Files:**
- All of the above

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace 2>&1 | tail -30`

Expected: No errors. If errors, fix them.

- [ ] **Step 2: Full workspace test**

Run: `cargo test --workspace 2>&1 | tail -50`

Expected: All tests pass.

- [ ] **Step 3: Fix any remaining issues**

If `cargo test` fails, diagnose and fix. Common issues:
- Missed `RunContext::new()` callers
- OTel API changes in vol-monitor tracing_setup (the existing OTel trace setup may need API updates for 0.27)
- Feature flag changes in dependent crates

- [ ] **Step 4: Commit fixes**

```bash
git add -A
git commit -m "fix: resolve remaining compilation issues from OTel upgrade"
```

---

### Task 10: Verify Success Criteria

- [ ] **Step 1: Verify LokiPlugin has no HTTP/URL code**

Run: `grep -inE "http|url|reqwest|endpoint|loki.*://" crates/vol-llm-observability/src/loki/plugin.rs`

Expected: No matches.

- [ ] **Step 2: Verify structured fields in tracing call**

Run: `grep "tracing::info!" crates/vol-llm-observability/src/loki/plugin.rs`

Expected: Contains `namespace`, `session_id`, `agent_id`, `agent_type`, `run_id`, `model`, `event`.

- [ ] **Step 3: Verify init_otel_logs exists**

Run: `grep "pub fn init_otel_logs" crates/vol-monitor/src/tracing_setup.rs`

Expected: Function definition found.

- [ ] **Step 4: Verify RunContext.model field**

Run: `grep "pub model: String" crates/vol-llm-agent/src/react/run_context.rs`

Expected: Field definition found.

- [ ] **Step 5: Final test run**

Run: `cargo test --workspace 2>&1 | grep -E "(PASSED|FAILED|test result)"`

Expected: `test result: ok. X passed; 0 failed`
