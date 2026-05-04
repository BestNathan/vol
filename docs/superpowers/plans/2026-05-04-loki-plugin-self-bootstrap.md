# LokiPlugin Self-Bootstrapping Refactoring

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Derive all agent identity fields (type, id) from `RunContext` at runtime instead of construction time, simplifying `LokiPlugin` to only hold the writer.

**Architecture:** `LokiPlugin::create_loki_entry()` now takes `(&AgentStreamEvent, &RunContext)` and internally derives `agent_type` from `ctx.config.def.r#type` and `agent_id` from `ctx.config.def.name`, with `"unknown"` fallback when no `AgentDef` is present.

**Tech Stack:** Rust, `vol-llm-agent` (RunContext, AgentDef), `vol-llm-observability` (LokiPlugin)

---

### Task 1: Refactor LokiPlugin struct, constructor, and create_loki_entry

**Files:**
- Modify: `crates/vol-llm-observability/src/loki/plugin.rs`

- [ ] **Step 1: Remove `agent_type` field from struct and simplify constructor**

Replace the struct and `new()` function. Current code (lines 28-49):

```rust
pub struct LokiPlugin {
    agent_type: String,
    writer: Arc<LokiWriter>,
}

impl LokiPlugin {
    /// Create a new LokiPlugin.
    ///
    /// Returns `None` if no Loki URL is configured (via `LokiConfig::from_env()`).
    pub fn new(config: LokiConfig, agent_type: &str) -> Self {
        let writer = LokiWriter::spawn(
            config.url,
            config.batch_size,
            config.flush_interval_ms,
        );
        Self {
            agent_type: agent_type.to_string(),
            writer: Arc::new(writer),
        }
    }
```

Replace with:

```rust
pub struct LokiPlugin {
    writer: Arc<LokiWriter>,
}

impl LokiPlugin {
    /// Create a new LokiPlugin.
    ///
    /// Agent identity (type, id) is derived from `RunContext.config.def` at runtime.
    pub fn new(config: LokiConfig) -> Self {
        let writer = LokiWriter::spawn(
            config.url,
            config.batch_size,
            config.flush_interval_ms,
        );
        Self {
            writer: Arc::new(writer),
        }
    }
```

- [ ] **Step 2: Refactor `create_loki_entry` to derive from RunContext**

Replace the function signature and add derivation at the top. Current (line 63):

```rust
    pub fn create_loki_entry(event: &AgentStreamEvent, run_id: &str, session_id: &str, agent_id: &str, agent_type: &str) -> LokiEntry {
        let labels = LokiLabels::new(agent_type, agent_id);
```

Replace with:

```rust
    pub fn create_loki_entry(event: &AgentStreamEvent, ctx: &RunContext) -> LokiEntry {
        let def = ctx.config.def.as_ref();
        let agent_type = def.map(|d| &d.r#type).unwrap_or("unknown");
        let agent_id = def.map(|d| &d.name).unwrap_or("unknown");
        let run_id = &ctx.run_id;
        let session_id = &ctx.session_id;

        let labels = LokiLabels::new(agent_type, agent_id);
```

The rest of the function body (lines 65-105) stays exactly the same — it already uses `agent_type`, `agent_id`, `run_id`, `session_id` as local variables.

- [ ] **Step 3: Simplify `listen()`**

Replace line 220. Current:

```rust
        let entry = Self::create_loki_entry(event, &ctx.run_id, &ctx.session_id, &ctx.config.def.as_ref().map(|d| &d.name).unwrap_or(&String::new()), &self.agent_type);
```

Replace with:

```rust
        let entry = Self::create_loki_entry(event, ctx);
```

- [ ] **Step 4: Update module doc comment**

Replace the doc comment lines 6-14. Current:

```rust
//! # Labels
//!
//! Each entry is sent to Loki with labels:
//! - `namespace`: `"agent"` (fixed)
//! - `agent`: Agent type (e.g., `"coding"`, `"advice"`)
//! - `agent_id`: From `AgentConfig.agent_id`
```

Replace with:

```rust
//! # Labels
//!
//! Each entry is sent to Loki with labels:
//! - `namespace`: `"agent"` (fixed)
//! - `agent`: From `AgentDef.r#type` (via `RunContext.config.def`)
//! - `agent_id`: From `AgentDef.name` (via `RunContext.config.def`)
```

### Task 2: Update tests in plugin.rs

**Files:**
- Modify: `crates/vol-llm-observability/src/loki/plugin.rs` (test module, lines 225-308)

- [ ] **Step 1: Update test_plugin_id and test_plugin_priority**

Replace the `LokiPlugin::new(config, "coding")` calls in both tests. Current:

```rust
    #[tokio::test]
    async fn test_plugin_id() {
        let config = LokiConfig::with_url("http://loki:3100".to_string());
        let plugin = LokiPlugin::new(config, "coding");
        assert_eq!(plugin.id(), "loki");
    }

    #[tokio::test]
    async fn test_plugin_priority() {
        let config = LokiConfig::with_url("http://loki:3100".to_string());
        let plugin = LokiPlugin::new(config, "coding");
        assert_eq!(plugin.priority(), 20);
    }
```

Replace with:

```rust
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
```

- [ ] **Step 2: Add test context helper function**

Add this helper at the top of the `#[cfg(test)] mod tests` block, after the existing `use super::*;` line:

```rust
    fn make_test_context(run_id: &str, session_id: &str, agent_name: &str, agent_type: &str) -> RunContext {
        use vol_llm_agent::agent_def::{AgentDef, AgentScope};
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
            tools,
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
```

- [ ] **Step 3: Update loki entry tests to use make_test_context**

Replace the three `create_loki_entry` test functions. Current (lines 271-308):

```rust
    #[test]
    fn test_loki_entry_tool_call() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let entry = LokiPlugin::create_loki_entry(&event, "run-1", "sess-1", "agent-001", "coding");
        assert!(entry.line.contains("bash"));
        assert!(entry.line.contains("run-1"));
        assert!(entry.line.contains("sess-1"));
        assert!(entry.line.contains("agent-001"));
        assert!(entry.line.contains("ToolCallBegin"));
    }

    #[test]
    fn test_loki_entry_llm_complete_includes_model_label() {
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: Utc::now(),
            model: "qwen3.5-plus".to_string(),
            usage: None,
        };
        let entry = LokiPlugin::create_loki_entry(&event, "run-1", "sess-1", "agent-001", "coding");
        assert!(entry.labels.contains_key("model"));
        assert_eq!(entry.labels["model"], "qwen3.5-plus");
    }

    #[test]
    fn test_loki_entry_plugin_event() {
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let entry = LokiPlugin::create_loki_entry(&event, "run-1", "sess-1", "agent-001", "coding");
        assert!(entry.line.contains("PluginEvent"));
        assert!(entry.line.contains("my_plugin"));
    }
```

Replace with:

```rust
    #[test]
    fn test_loki_entry_tool_call() {
        let event = AgentStreamEvent::ToolCallBegin {
            timestamp: Utc::now(),
            tool_call_id: "c1".to_string(),
            tool_name: "bash".to_string(),
            arguments: "{}".to_string(),
        };
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.line.contains("bash"));
        assert!(entry.line.contains("run-1"));
        assert!(entry.line.contains("sess-1"));
        assert!(entry.line.contains("agent-001"));
        assert!(entry.line.contains("ToolCallBegin"));
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
    }

    #[test]
    fn test_loki_entry_plugin_event() {
        let mut data = Map::new();
        data.insert("key".to_string(), json!("value"));
        let event = AgentStreamEvent::plugin_event("my_plugin".to_string(), data);
        let ctx = make_test_context("run-1", "sess-1", "agent-001", "coding");
        let entry = LokiPlugin::create_loki_entry(&event, &ctx);
        assert!(entry.line.contains("PluginEvent"));
        assert!(entry.line.contains("my_plugin"));
    }
```

- [ ] **Step 4: Add test for no-def fallback (agent_type = "unknown")**

Add this new test after the existing tests:

```rust
    #[test]
    fn test_loki_entry_fallback_no_agent_def() {
        use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
        use vol_llm_context::ContextBuilderBuilder;
        use vol_session::{InMemoryEntryStore, Session};
        use vol_llm_tool::ToolRegistry;

        // Build context with def = None
        let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
        let tools = Arc::new(ToolRegistry::new());
        let context_builder = ContextBuilderBuilder::new(128_000).build();
        let config = AgentConfig {
            def: None,
            llm: Arc::new(DummyLlm),
            tools,
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
```

### Task 3: Update CodingAgentBuilder.with_loki() call site

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`

- [ ] **Step 1: Update with_loki() to remove agent_type parameter**

Current (lines 355-365):

```rust
    /// Register LokiPlugin to send agent events to Loki.
    ///
    /// Reads the Loki URL from the `LOKI_URL` environment variable.
    /// If not set, this is a no-op (no plugin is registered).
    pub fn with_loki(mut self) -> Self {
        if let Some(config) = vol_llm_observability::loki::LokiConfig::from_env() {
            let plugin = vol_llm_observability::loki::LokiPlugin::new(config, "coding");
            self.config.plugin_registry.register(plugin);
        }
        self
    }
```

Replace with:

```rust
    /// Register LokiPlugin to send agent events to Loki.
    ///
    /// Agent identity (type, id) is derived from `RunContext.config.def` at runtime.
    /// Reads the Loki URL from the `LOKI_URL` environment variable.
    /// If not set, this is a no-op (no plugin is registered).
    pub fn with_loki(mut self) -> Self {
        if let Some(config) = vol_llm_observability::loki::LokiConfig::from_env() {
            let plugin = vol_llm_observability::loki::LokiPlugin::new(config);
            self.config.plugin_registry.register(plugin);
        }
        self
    }
```

### Task 4: Build and test

**Files:** No changes

- [ ] **Step 1: Run cargo check**

```bash
cargo check -p vol-llm-observability -p vol-llm-agents
```

Expected: no errors.

- [ ] **Step 2: Run Loki plugin tests**

```bash
cargo test -p vol-llm-observability loki -- --nocapture
```

Expected: all tests pass (test_plugin_id, test_plugin_priority, test_should_send_skips_delta_events, test_loki_entry_tool_call, test_loki_entry_llm_complete_includes_model_label, test_loki_entry_plugin_event, test_loki_entry_fallback_no_agent_def).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-observability/src/loki/plugin.rs crates/vol-llm-agents/src/coding/agent.rs
git commit -m "refactor: derive Loki labels from RunContext instead of construction time"
```
