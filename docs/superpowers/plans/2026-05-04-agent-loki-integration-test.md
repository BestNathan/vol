# Agent File → Loki Integration Test Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create an integration test that writes a `type=test_agent` agent definition file, loads it via `AgentLoader`, builds and runs a `ReActAgent` with `LokiPlugin` registered using a mock LLM, and verifies Loki entries carry the correct labels from the loaded `AgentDef`.

**Architecture:** The test creates a temp directory with a `.agents/agents/test_agent.md` file. An `AgentLoader` discovers it. A `ReActAgent` is built via `AgentConfig::builder()` with the loaded `AgentDef`, a mock LLM (returns `ContentComplete` immediately), and `LokiPlugin` registered. After the agent runs, we verify: (1) the agent completed successfully, (2) `create_loki_entry` produces entries with `labels["agent"] == "test_agent"` matching the loaded `AgentDef.r#type`.

**Tech Stack:** Rust, `tempfile`, `vol-llm-agent` (AgentLoader, ReActAgent, AgentDef), `vol-llm-observability` (LokiPlugin), `tokio`

---

### Task 1: Create integration test file

**Files:**
- Create: `crates/vol-llm-agents/tests/agent_loki_integration.rs`

- [ ] **Step 1: Write the integration test**

Create `crates/vol-llm-agents/tests/agent_loki_integration.rs` with this full content:

```rust
//! Integration test: agent file → AgentLoader → ReActAgent with LokiPlugin.
//!
//! Verifies that:
//! 1. An agent definition file is loaded with correct type
//! 2. The agent runs successfully with LokiPlugin registered
//! 3. Loki entries carry correct labels derived from AgentDef

use std::io::Write;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use tempfile::tempdir;
use vol_llm_agent::agent_def::AgentScope;
use vol_llm_agent::agent_loader::AgentLoader;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent, RunContext};
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_core::{
    ConversationRequest, ConversationResponse, LLMClient, LLMProvider,
    StreamEvent, StreamEventData, StreamReceiver, SupportedParam,
};
use vol_llm_observability::loki::{LokiConfig, LokiPlugin};
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

/// Mock LLM that immediately returns ContentComplete.
struct MockLlm {
    response: String,
    call_count: Arc<AtomicUsize>,
}

impl MockLlm {
    fn new(response: String) -> Self {
        Self {
            response,
            call_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn call_count(&self) -> usize {
        self.call_count.load(Ordering::SeqCst)
    }
}

#[async_trait::async_trait]
impl LLMClient for MockLlm {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock-model"
    }

    fn supported_params(&self) -> &[SupportedParam] {
        &[]
    }

    async fn converse(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<ConversationResponse> {
        unimplemented!("Use converse_stream")
    }

    async fn converse_stream(
        &self,
        _request: ConversationRequest,
    ) -> vol_llm_core::Result<StreamReceiver> {
        use tokio::sync::mpsc;
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = mpsc::channel(10);
        let text = self.response.clone();
        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamEvent {
                    id: "event_1".to_string(),
                    data: StreamEventData::ContentComplete { content: text },
                }))
                .await;
        });
        Ok(StreamReceiver::new(rx))
    }
}

#[tokio::test]
async fn test_agent_file_loaded_with_loki_plugin() {
    // 1. Create temp directory with test_agent.md
    let tmp = tempdir().unwrap();
    let agents_dir = tmp.path().join(".agents").join("agents");
    std::fs::create_dir_all(&agents_dir).unwrap();

    let mut f = std::fs::File::create(agents_dir.join("test_agent.md")).unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "name: test_agent").unwrap();
    writeln!(f, "type: test_agent").unwrap();
    writeln!(f, "description: A test agent for Loki integration").unwrap();
    writeln!(f, "---").unwrap();
    writeln!(f, "You are a test agent. Reply with 'TEST_DONE' when done.").unwrap();

    // 2. Load agent via AgentLoader
    let mut loader = AgentLoader::new_empty();
    loader.add_root(AgentScope::User, agents_dir);
    loader.discover_all().await.unwrap();

    let def = loader.get("test_agent").await.expect("test_agent should be loaded");
    assert_eq!(def.r#type, "test_agent");
    assert_eq!(def.name, "test_agent");

    // 3. Build ReActAgent with LokiPlugin
    let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools = Arc::new(ToolRegistry::new());
    let context_builder = ContextBuilderBuilder::new(128_000).build();

    let loki_config = LokiConfig::with_url("http://loki:3100".to_string());
    let loki_plugin = LokiPlugin::new(loki_config);

    let mut plugin_registry = PluginRegistry::new();
    plugin_registry.register(loki_plugin);

    let agent_config = AgentConfig::builder()
        .with_def((*def).clone())
        .with_llm(Arc::new(MockLlm::new("TEST_DONE".to_string())))
        .with_tools(tools)
        .with_session(session)
        .with_system_prompt("You are a test agent. Reply with 'TEST_DONE' when done.")
        .with_plugin_registry(plugin_registry)
        .build()
        .unwrap();

    let agent = ReActAgent::new(agent_config);

    // 4. Run the agent
    let response = agent.run("hello").await.expect("agent should complete");

    // 5. Verify agent completed successfully
    assert!(response.content.contains("TEST_DONE"));
    assert!(!response.run_id.is_empty());

    // 6. Verify LokiPlugin was registered
    let plugins = agent.config.plugin_registry.plugins();
    let loki_registered = plugins.iter().any(|p| p.id() == "loki");
    assert!(loki_registered, "LokiPlugin should be registered");

    // 7. Verify Loki labels are derived from AgentDef
    // Build a minimal RunContext to test create_loki_entry
    let session2 = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools2 = Arc::new(ToolRegistry::new());
    let context_builder2 = ContextBuilderBuilder::new(128_000).build();
    let loki_config2 = LokiConfig::with_url("http://loki:3100".to_string());
    let loki_plugin2 = LokiPlugin::new(loki_config2);
    let mut plugin_registry2 = PluginRegistry::new();
    plugin_registry2.register(loki_plugin2);

    let agent_config2 = AgentConfig::builder()
        .with_def((*def).clone())
        .with_llm(Arc::new(MockLlm::new("ok".to_string())))
        .with_tools(tools2.clone())
        .with_session(session2.clone())
        .with_system_prompt("test")
        .with_plugin_registry(plugin_registry2)
        .build()
        .unwrap();

    let (run_ctx, _rx) = RunContext::new(
        "test-run-id".to_string(),
        "hello".to_string(),
        "test-session-id".to_string(),
        session2,
        tools2,
        agent_config2,
        20,
    );

    let event = vol_llm_core::AgentStreamEvent::AgentStart {
        timestamp: chrono::Utc::now(),
        input: "hello".to_string(),
    };

    let entry = LokiPlugin::create_loki_entry(&event, &run_ctx);

    // Verify labels match AgentDef
    assert_eq!(entry.labels["agent"], "test_agent");
    assert_eq!(entry.labels["agent_id"], "test_agent");
    assert_eq!(entry.labels["namespace"], "agent");
}
```

- [ ] **Step 2: Run the test to verify it compiles and passes**

Run:
```bash
cargo test -p vol-llm-agents --test agent_loki_integration -- --nocapture
```

Expected: Test passes with `test result: ok. 1 passed`.

If `chrono` is not available as a direct dependency in the test, it's available through `vol_llm_core::AgentStreamEvent` which uses `chrono::DateTime<Utc>`. The `AgentStreamEvent` type is in `vol_llm_core::stream` — check the import path.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agents/tests/agent_loki_integration.rs
git commit -m "test: add agent file → Loki integration test"
```
