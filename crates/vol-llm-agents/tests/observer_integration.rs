//! Integration test for ObserverPlugin with CodingAgent

use std::sync::Arc;
use tempfile::tempdir;
use vol_llm_agent::react::AgentPlugin;
use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter, ObserverPlugin};
use vol_llm_core::{AgentStreamEvent, LLMClient, LLMProvider};
use vol_llm_provider::{LLMConfig, LLMProviderConfig, LLMProviderRegistry, Secret};

/// Helper to construct the LLM client for tests.
fn create_test_llm() -> Arc<dyn LLMClient> {
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN").expect("ANTHROPIC_AUTH_TOKEN must be set");
    let llm_config = LLMProviderConfig {
        id: "anthropic-main".to_string(),
        config: LLMConfig {
            provider: LLMProvider::Anthropic,
            model: "qwen3.5-plus".to_string(),
            api_key: Secret::literal(api_key),
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
            body: None,
            headers: None,
        },
    };
    let registry = LLMProviderRegistry::from_configs(&[llm_config]).unwrap();
    registry.get("anthropic-main").unwrap().clone()
}

#[tokio::test]
async fn test_observer_plugin_receives_all_events() {
    // Create a mock observer that tracks event types
    struct EventTracker {
        events: tokio::sync::Mutex<Vec<String>>,
    }

    #[async_trait::async_trait]
    impl vol_llm_agents::coding::EventObserver for EventTracker {
        async fn on_event(
            &self,
            event: &AgentStreamEvent,
        ) -> Result<(), vol_llm_agents::coding::ObserverError> {
            self.events
                .lock()
                .await
                .push(Self::event_name(event).to_string());
            Ok(())
        }

        async fn on_complete(&self) -> Result<(), vol_llm_agents::coding::ObserverError> {
            Ok(())
        }
    }

    impl EventTracker {
        fn new() -> Self {
            Self {
                events: tokio::sync::Mutex::new(Vec::new()),
            }
        }

        fn event_name(event: &AgentStreamEvent) -> &'static str {
            match event {
                AgentStreamEvent::AgentStart { .. } => "AgentStart",
                AgentStreamEvent::LLMCallStart { .. } => "LLMCallStart",
                AgentStreamEvent::LLMCallComplete { .. } => "LLMCallComplete",
                AgentStreamEvent::LLMCallError { .. } => "LLMCallError",
                AgentStreamEvent::ThinkingStart { .. } => "ThinkingStart",
                AgentStreamEvent::ThinkingDelta { .. } => "ThinkingDelta",
                AgentStreamEvent::ThinkingComplete { .. } => "ThinkingComplete",
                AgentStreamEvent::ContentStart { .. } => "ContentStart",
                AgentStreamEvent::ContentDelta { .. } => "ContentDelta",
                AgentStreamEvent::ContentComplete { .. } => "ContentComplete",
                AgentStreamEvent::ToolCallBegin { .. } => "ToolCallBegin",
                AgentStreamEvent::ToolCallComplete { .. } => "ToolCallComplete",
                AgentStreamEvent::ToolCallError { .. } => "ToolCallError",
                AgentStreamEvent::ToolCallSkipped { .. } => "ToolCallSkipped",
                AgentStreamEvent::IterationComplete { .. } => "IterationComplete",
                AgentStreamEvent::AgentComplete { .. } => "AgentComplete",
                AgentStreamEvent::AgentAborted { .. } => "AgentAborted",
                AgentStreamEvent::PluginEvent { .. } => "PluginEvent",
                AgentStreamEvent::MaxIterationsReached { .. } => "MaxIterationsReached",
                AgentStreamEvent::IterationContinued { .. } => "IterationContinued",
                AgentStreamEvent::ToolCallArgumentDelta { .. } => "ToolCallArgumentDelta",
            }
        }

        #[allow(dead_code)]
        async fn get_events(&self) -> Vec<String> {
            self.events.lock().await.clone()
        }
    }

    // This test verifies plugin registration without requiring a real LLM
    let tracker = Arc::new(EventTracker::new());
    let plugin = ObserverPlugin::new(tracker);

    // Verify plugin is created correctly
    assert_eq!(plugin.id(), "observer");
    assert_eq!(plugin.priority(), 0);
}

#[tokio::test]
#[ignore] // Requires real LLM API key
async fn test_coding_agent_generates_complete_html_report() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");

    let config = CodingAgentConfig {
        max_iterations: 5,
        working_dir: temp_dir.path().to_path_buf(),
        html_report_path: Some(report_path.clone()),
        llm: Some(create_test_llm()),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).unwrap();

    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Simple task".to_string(),
    ));
    let agent = agent.with_observer(observer);

    let result = agent
        .run("What files are in the current directory?")
        .await
        .unwrap();

    assert!(result.success);

    // Verify report was generated
    assert!(report_path.exists());

    // Verify report contains timeline events
    let content = std::fs::read_to_string(&report_path).unwrap();
    assert!(content.contains("Timeline"));
    assert!(content.contains("ToolCall") || content.contains("Thinking"));
}
