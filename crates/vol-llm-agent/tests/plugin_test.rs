//! Plugin system integration tests.

use vol_llm_agent::*;
use vol_llm_agent::react::*;
use vol_llm_agent::react::plugin::PluginId;

#[tokio::test]
async fn test_plugin_priority_ordering() {
    // Create a mock plugin with custom priority
    struct TestPlugin {
        id: String,
        priority: u32,
    }

    #[async_trait::async_trait]
    impl AgentPlugin for TestPlugin {
        fn id(&self) -> PluginId {
            self.id.clone()
        }

        fn priority(&self) -> u32 {
            self.priority
        }

        async fn intercept(
            &self,
            event: Result<AgentStreamEvent, AgentError>,
            _ctx: &RunContext,
        ) -> PluginAction<Option<Result<AgentStreamEvent, AgentError>>> {
            PluginAction::Continue(Some(event))
        }

        async fn on_complete(
            &self,
            _ctx: &RunContext,
            _response: &AgentResponse,
        ) -> PluginAction<()> {
            PluginAction::Continue(())
        }
    }

    let mut registry = PluginRegistry::new();
    registry.register(TestPlugin { id: "low".to_string(), priority: 100 });
    registry.register(TestPlugin { id: "high".to_string(), priority: 10 });
    registry.register(TestPlugin { id: "mid".to_string(), priority: 50 });

    // Should be ordered by priority: high (10), mid (50), low (100)
    let ids: Vec<String> = registry.plugins().iter().map(|p| p.id()).collect();
    assert_eq!(ids, vec!["high", "mid", "low"]);
}

#[tokio::test]
async fn test_plugin_short_circuit() {
    // Create a plugin that short-circuits on_start
    struct ShortCircuitPlugin;

    #[async_trait::async_trait]
    impl AgentPlugin for ShortCircuitPlugin {
        fn id(&self) -> PluginId {
            "short_circuit".to_string()
        }

        async fn on_start(&self, _ctx: &RunContext) -> PluginAction<()> {
            PluginAction::ShortCircuit(AgentResponse {
                content: "Cached response".to_string(),
                reasoning: "Returned from cache".to_string(),
                iterations: 0,
                tool_calls: Vec::new(),
            })
        }

        async fn intercept(
            &self,
            event: Result<AgentStreamEvent, AgentError>,
            _ctx: &RunContext,
        ) -> PluginAction<Option<Result<AgentStreamEvent, AgentError>>> {
            PluginAction::Continue(Some(event))
        }

        async fn on_complete(
            &self,
            _ctx: &RunContext,
            _response: &AgentResponse,
        ) -> PluginAction<()> {
            PluginAction::Continue(())
        }
    }

    let mut registry = PluginRegistry::new();
    registry.register(ShortCircuitPlugin);

    let config = AgentConfig {
        plugin_registry: registry,
        ..Default::default()
    };

    // Verify the plugin is registered
    assert_eq!(config.plugin_registry.plugins().len(), 1);
    assert_eq!(config.plugin_registry.plugins()[0].id(), "short_circuit");
}

#[tokio::test]
async fn test_run_context_data_storage() {
    use std::sync::Arc;
    use vol_llm_agent::session::{Session, InMemorySessionStore, InMemoryMessageStore};
    use vol_llm_tool::ToolRegistry;

    let ctx = RunContext::new(
        "test-run-123".to_string(),
        "test input".to_string(),
        "session-456".to_string(),
        Arc::new(Session::new(
            "session-456".to_string(),
            Arc::new(InMemorySessionStore::new()),
            Arc::new(InMemoryMessageStore::new()),
        )),
        Arc::new(ToolRegistry::new()),
        AgentConfig::default(),
    );

    // Test setting and getting data
    ctx.set("counter", 42i32).await.unwrap();
    let value: Option<i32> = ctx.get("counter").await;
    assert_eq!(value, Some(42));

    // Test getting with wrong type
    let wrong_type: Option<String> = ctx.get("counter").await;
    assert_eq!(wrong_type, None);

    // Test getting non-existent key
    let missing: Option<String> = ctx.get("missing").await;
    assert_eq!(missing, None);

    // Verify context fields
    assert_eq!(ctx.run_id, "test-run-123");
    assert_eq!(ctx.user_input, "test input");
    assert_eq!(ctx.session_id, "session-456");
}

#[tokio::test]
async fn test_plugin_action_variants() {
    use PluginAction::*;

    // Test Continue
    let continue_action: PluginAction<i32> = Continue(42);
    assert!(matches!(continue_action, Continue(42)));

    // Test ShortCircuit
    let response = AgentResponse {
        content: "test".to_string(),
        reasoning: String::new(),
        iterations: 0,
        tool_calls: Vec::new(),
    };
    let shortcircuit_action: PluginAction<()> = ShortCircuit(response.clone());
    assert!(matches!(shortcircuit_action, ShortCircuit(_)));

    // Test Skip
    let skip_action: PluginAction<()> = Skip;
    assert!(matches!(skip_action, Skip));

    // Test Abort
    let error = AgentError::MaxIterationsReached { max: 5 };
    let abort_action: PluginAction<()> = Abort(error);
    assert!(matches!(abort_action, Abort(_)));

    // Test map on Continue
    let mapped = Continue(21).map(|x| x * 2);
    assert!(matches!(mapped, Continue(42)));

    // Test map_err on Abort
    let mapped_err: PluginAction<()> = Abort(AgentError::MaxIterationsReached { max: 5 })
        .map_err(|e| AgentError::Context(format!("Wrapped: {}", e)));
    assert!(matches!(mapped_err, Abort(AgentError::Context(_))));
}
