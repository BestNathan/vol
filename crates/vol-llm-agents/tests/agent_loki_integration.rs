//! Integration test: agent file → AgentLoader → ReActAgent with LokiPlugin.
//!
//! Verifies that:
//! 1. An agent definition file is loaded with correct type
//! 2. The agent runs successfully with LokiPlugin registered
//! 3. Loki entries carry correct labels derived from AgentDef

use std::io::Write;
use std::sync::Arc;
use tempfile::tempdir;
use vol_llm_agent::agent_def::AgentScope;
use vol_llm_agent::agent_loader::AgentLoader;
use vol_llm_agent::react::{AgentConfig, PluginRegistry, ReActAgent, RunContext};
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_core::{
    LLMClient, LLMProvider,
    StreamEvent, StreamEventData, StreamReceiver, SupportedParam,
};
use vol_llm_observability::loki::{LokiConfig, LokiPlugin};
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

/// Mock LLM that immediately returns ContentComplete.
struct MockLlm {
    response: String,
}

impl MockLlm {
    fn new(response: String) -> Self {
        Self { response }
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
        _request: vol_llm_core::ConversationRequest,
    ) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
        unimplemented!("Use converse_stream")
    }

    async fn converse_stream(
        &self,
        _request: vol_llm_core::ConversationRequest,
    ) -> vol_llm_core::Result<StreamReceiver> {
        use tokio::sync::mpsc;
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

    let loki_config = LokiConfig::with_url("http://loki:3100".to_string());
    let loki_plugin = LokiPlugin::new(loki_config);

    let mut plugin_registry = PluginRegistry::new();
    plugin_registry.register(loki_plugin);

    // 4. Verify LokiPlugin was registered
    let loki_registered = plugin_registry.plugins().iter().any(|p| p.id() == "loki");
    assert!(loki_registered, "LokiPlugin should be registered");

    let agent_config = AgentConfig::builder()
        .with_def((*def).clone())
        .with_llm(Arc::new(MockLlm::new("TEST_DONE".to_string())))
        .with_tools(tools)
        .with_session(session)
        .with_system_prompt("You are a test agent. Reply with 'TEST_DONE' when done.".to_string())
        .with_plugin_registry(plugin_registry)
        .build()
        .unwrap();

    let agent = ReActAgent::new(agent_config);

    // 5. Run the agent
    let response = agent.run("hello").await.expect("agent should complete");

    // 6. Verify agent completed successfully
    assert!(response.content.contains("TEST_DONE"));
    assert!(!response.run_id.is_empty());

    // 7. Verify Loki labels are derived from AgentDef
    // Build a minimal RunContext to test create_loki_entry
    let session2 = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
    let tools2 = Arc::new(ToolRegistry::new());
    let context_builder = ContextBuilderBuilder::new(128_000).build();

    let agent_config2 = AgentConfig::builder()
        .with_def((*def).clone())
        .with_llm(Arc::new(MockLlm::new("ok".to_string())))
        .with_tools(tools2.clone())
        .with_session(session2.clone())
        .with_system_prompt("test".to_string())
        .with_plugin_registry(PluginRegistry::new())
        .with_context_builder(context_builder)
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
