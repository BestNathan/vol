//! Plugin system integration tests.

use std::sync::Arc;

use vol_llm_agent::react::plugin::PluginId;
use vol_llm_agent::react::RunContext;
use vol_llm_agent::react::*;
use vol_llm_agent::*;

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
            _event: &AgentStreamEvent,
            _ctx: &RunContext,
        ) -> PluginDecision {
            PluginDecision::Continue
        }

        async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {
            // no-op
        }
    }

    let mut registry = PluginRegistry::new();
    registry.register(TestPlugin {
        id: "low".to_string(),
        priority: 100,
    });
    registry.register(TestPlugin {
        id: "high".to_string(),
        priority: 10,
    });
    registry.register(TestPlugin {
        id: "mid".to_string(),
        priority: 50,
    });

    // Should be ordered by priority: high (10), mid (50), low (100)
    let ids: Vec<String> = registry.plugins().iter().map(|p| p.id()).collect();
    assert_eq!(ids, vec!["high", "mid", "low"]);
}

#[tokio::test]
async fn test_run_context_data_storage() {
    let (ctx, _plugin_rx) = RunContext::new(
        "test-run-123".to_string(),
        "test input".to_string(),
        Arc::new(AgentConfig::default()),
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
    assert_eq!(ctx.session_id, ctx.session.id);
}

#[tokio::test]
async fn test_plugin_decision_variants() {
    use PluginDecision::*;

    // Test Continue
    let continue_decision = Continue;
    assert!(matches!(continue_decision, Continue));

    // Test Skip
    let skip_decision = Skip;
    assert!(matches!(skip_decision, Skip));

    // Test Abort
    let abort_decision = Abort("reason".to_string());
    assert!(matches!(abort_decision, Abort(_)));
}
