//! Unit tests for ObserverPlugin

use vol_llm_agents::coding::{ObserverPlugin, EventObserver, ObserverError};
use vol_llm_core::{AgentStreamEvent, PluginContext};
use vol_llm_agent::react::AgentPlugin;
use std::sync::Arc;

fn create_test_context_builder() -> vol_llm_context::ContextBuilder {
    vol_llm_context::ContextBuilderBuilder::new(128_000)
        .build()
}

struct MockObserver {
    events: tokio::sync::Mutex<Vec<AgentStreamEvent>>,
}

impl MockObserver {
    fn new() -> Self {
        Self {
            events: tokio::sync::Mutex::new(Vec::new()),
        }
    }

    async fn get_events(&self) -> Vec<AgentStreamEvent> {
        self.events.lock().await.clone()
    }
}

#[async_trait::async_trait]
impl EventObserver for MockObserver {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        self.events.lock().await.push(event.clone());
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        Ok(())
    }
}

#[tokio::test]
async fn test_observer_plugin_forwards_events() {
    let mock_observer = Arc::new(MockObserver::new());
    let plugin = ObserverPlugin::new(mock_observer.clone());

    let event = AgentStreamEvent::AgentStart {
        input: "test task".to_string(),
        timestamp: chrono::Utc::now(),
    };

    // Create minimal PluginContext
    let ctx = create_test_plugin_context();

    plugin.listen(&event, &ctx).await;

    let events = mock_observer.get_events().await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AgentStreamEvent::AgentStart { .. }));
}

#[tokio::test]
async fn test_observer_plugin_id() {
    let mock_observer = Arc::new(MockObserver::new());
    let plugin = ObserverPlugin::new(mock_observer);

    assert_eq!(plugin.id(), "observer");
}

#[tokio::test]
async fn test_observer_plugin_priority() {
    let mock_observer = Arc::new(MockObserver::new());
    let plugin = ObserverPlugin::new(mock_observer);

    assert_eq!(plugin.priority(), 0);
}

#[tokio::test]
async fn test_observer_plugin_forwards_multiple_events() {
    let mock_observer = Arc::new(MockObserver::new());
    let plugin = ObserverPlugin::new(mock_observer.clone());

    let ctx = create_test_plugin_context();

    // Send multiple events
    let events = vec![
        AgentStreamEvent::AgentStart { input: "task".to_string(), timestamp: chrono::Utc::now() },
        AgentStreamEvent::ThinkingComplete { thinking: "thinking...".to_string(), timestamp: chrono::Utc::now() },
        AgentStreamEvent::ToolCallBegin {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_1".to_string(),
            tool_name: "read_file".to_string(),
            arguments: r#"{"file": "test.txt"}"#.to_string()
        },
        AgentStreamEvent::ToolCallComplete {
            timestamp: chrono::Utc::now(),
            tool_call_id: "call_1".to_string(),
            tool_name: "read_file".to_string(),
            result: "success".to_string(),
            duration_ms: Some(10),
        },
        AgentStreamEvent::IterationComplete {
            timestamp: chrono::Utc::now(),
            iteration: 1,
            tool_calls: vec![],
            final_answer: None,
        },
        AgentStreamEvent::AgentComplete { response: None, timestamp: chrono::Utc::now() },
    ];

    for event in events {
        plugin.listen(&event, &ctx).await;
    }

    let recorded_events = mock_observer.get_events().await;
    assert_eq!(recorded_events.len(), 6);
}

// Helper function to create test PluginContext
fn create_test_plugin_context() -> vol_llm_agent::react::PluginContext {
    use vol_llm_agent::react::{AgentConfig, PluginRegistry, RunContext};
    use vol_session::{InMemoryEntryStore, Session};
    use vol_llm_tool::ToolRegistry;

    let (ctx, _plugin_rx, _approval_rx) = RunContext::new(
        "test-run".to_string(),
        "test input".to_string(),
        "session-1".to_string(),
        Arc::new(Session::new(
            Arc::new(InMemoryEntryStore::new()),
        )),
        Arc::new(ToolRegistry::new()),
        AgentConfig {
            max_iterations: 10,
            max_history_messages: 20,
            context_builder: create_test_context_builder(),
            verbose: false,
            plugin_registry: PluginRegistry::new(),
            agent_id: "test-agent".to_string(),
            log_base_path: std::path::PathBuf::from("logs/test"),
            ..Default::default()
        },
    );
    vol_llm_agent::react::plugin_context_from_run_ctx(&ctx)
}
