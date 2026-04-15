//! Unit tests for ObserverPlugin

use vol_llm_agents::coding::{ObserverPlugin, EventObserver, ObserverError};
use vol_llm_core::{AgentStreamEvent, PluginContext, ToolCall};
use vol_llm_agent::react::AgentPlugin;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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
fn create_test_plugin_context() -> PluginContext {
    PluginContext {
        run_id: "test-run".to_string(),
        user_input: "test input".to_string(),
        session_id: "session-1".to_string(),
        messages: Arc::new(RwLock::new(vec![])),
        all_tool_calls: Arc::new(RwLock::new(vec![])),
        current_tool_calls: Arc::new(RwLock::new(vec![])),
        data: Arc::new(RwLock::new(HashMap::new())),
    }
}
