//! Plugin system for ReAct Agent.
//!
//! Plugins can intercept and modify the agent event stream,
//! implement cross-cutting concerns like observability, caching, etc.

use async_trait::async_trait;
use std::sync::Arc;
use super::run_context::RunContext;
use super::AgentStreamEvent;

/// Plugin unique identifier
pub type PluginId = String;

/// Decision returned by intercept() hook
#[derive(Debug, Clone)]
pub enum PluginDecision {
    /// Continue to next interceptor or execute event
    Continue,
    /// Skip current event (don't execute tool/loop)
    Skip,
    /// Abort entire agent execution with reason
    Abort(String),
}

/// Plugin trait for extending agent functionality
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;

    fn priority(&self) -> u32 { 100 }

    /// Interceptor hook - sync, serial, can block flow
    ///
    /// Called before event execution. Can modify or block the event.
    /// Returns PluginDecision to continue, skip, or abort.
    async fn intercept(
        &self,
        _event: &AgentStreamEvent,
        _ctx: &RunContext
    ) -> PluginDecision {
        PluginDecision::Continue  // Default: no-op
    }

    /// Listener hook - async, parallel, fire-and-forget
    ///
    /// Called after event execution. Used for observability, logging, etc.
    /// Does not affect event flow.
    async fn listen(
        &self,
        _event: &AgentStreamEvent,
        _ctx: &RunContext
    );
}

/// Plugin registry - manages plugin lifecycle and execution order
#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn AgentPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    pub fn register<P: AgentPlugin + 'static>(&mut self, plugin: P) {
        let plugin = Arc::new(plugin);
        // Insert by priority (lower number = higher priority = executed first)
        let pos = self.plugins.iter()
            .position(|p| p.priority() > plugin.priority())
            .unwrap_or(self.plugins.len());
        self.plugins.insert(pos, plugin);
    }

    pub fn plugins(&self) -> &[Arc<dyn AgentPlugin>] {
        &self.plugins
    }

    pub fn get(&self, id: &str) -> Option<&Arc<dyn AgentPlugin>> {
        self.plugins.iter().find(|p| p.id() == id)
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    struct TestPlugin { id: String, priority: u32 }

    #[async_trait]
    impl AgentPlugin for TestPlugin {
        fn id(&self) -> PluginId { self.id.clone() }
        fn priority(&self) -> u32 { self.priority }

        async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
            PluginDecision::Continue
        }

        async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {
            // no-op
        }
    }

    #[test]
    fn test_plugin_registry_orders_by_priority() {
        let mut registry = PluginRegistry::new();
        registry.register(TestPlugin { id: "low".to_string(), priority: 100 });
        registry.register(TestPlugin { id: "high".to_string(), priority: 10 });
        registry.register(TestPlugin { id: "mid".to_string(), priority: 50 });

        // Should be ordered: high (10), mid (50), low (100)
        let ids: Vec<String> = registry.plugins().iter().map(|p| p.id()).collect();
        assert_eq!(ids, vec!["high", "mid", "low"]);
    }

    #[test]
    fn test_plugin_decision_variants() {
        let continue_decision = PluginDecision::Continue;
        assert!(matches!(continue_decision, PluginDecision::Continue));

        let skip_decision = PluginDecision::Skip;
        assert!(matches!(skip_decision, PluginDecision::Skip));

        let abort_decision = PluginDecision::Abort("reason".to_string());
        assert!(matches!(abort_decision, PluginDecision::Abort(_)));
    }

    /// Test that interceptor chain executes in priority order (lower priority value = higher priority = executed first)
    #[tokio::test]
    async fn test_interceptor_chain_order() {
        struct OrderPlugin {
            id: String,
            priority: u32,
            order: Arc<Mutex<Vec<String>>>
        }

        #[async_trait]
        impl AgentPlugin for OrderPlugin {
            fn id(&self) -> PluginId { self.id.clone() }
            fn priority(&self) -> u32 { self.priority }

            async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
                let mut order = self.order.lock().await;
                order.push(self.id.clone());
                PluginDecision::Continue
            }

            async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {}
        }

        let order = Arc::new(Mutex::new(Vec::new()));
        let mut plugins = vec![
            Arc::new(OrderPlugin { id: "second".to_string(), priority: 20, order: order.clone() }),
            Arc::new(OrderPlugin { id: "first".to_string(), priority: 10, order: order.clone() }),
        ];
        plugins.sort_by_key(|p| p.priority());

        let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
        let ctx = create_test_context();

        for plugin in &plugins {
            plugin.intercept(&event, &ctx).await;
        }

        let final_order = order.lock().await;
        assert_eq!(*final_order, vec!["first", "second"]);
    }

    /// Test that Abort decision from any plugin stops the chain
    #[tokio::test]
    async fn test_interceptor_abort_stops_chain() {
        struct AbortPlugin {
            id: String,
            abort_at: usize,
            call_count: Arc<Mutex<usize>>,
        }

        #[async_trait]
        impl AgentPlugin for AbortPlugin {
            fn id(&self) -> PluginId { self.id.clone() }
            fn priority(&self) -> u32 { 100 }

            async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
                let mut count = self.call_count.lock().await;
                *count += 1;

                if *count >= self.abort_at {
                    PluginDecision::Abort(format!("Aborted by {}", self.id))
                } else {
                    PluginDecision::Continue
                }
            }

            async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {}
        }

        let call_count = Arc::new(Mutex::new(0usize));
        let plugins: Vec<Arc<dyn AgentPlugin>> = vec![
            Arc::new(AbortPlugin {
                id: "plugin1".to_string(),
                abort_at: 1,
                call_count: call_count.clone(),
            }),
            Arc::new(AbortPlugin {
                id: "plugin2".to_string(),
                abort_at: 2,
                call_count: call_count.clone(),
            }),
        ];

        let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
        let ctx = create_test_context();

        // Simulate interceptor chain - should stop at first plugin (abort_at: 1)
        for plugin in &plugins {
            let decision = plugin.intercept(&event, &ctx).await;
            if matches!(decision, PluginDecision::Abort(_)) {
                break;
            }
        }

        // Only first plugin should have been called (second should not be reached)
        let final_count = call_count.lock().await;
        assert_eq!(*final_count, 1);
    }

    /// Test that Skip decision skips current event but continues chain for next event
    #[tokio::test]
    async fn test_interceptor_skip_continues_chain() {
        struct SkipPlugin {
            id: String,
            skip: bool,
            call_count: Arc<Mutex<usize>>,
        }

        #[async_trait]
        impl AgentPlugin for SkipPlugin {
            fn id(&self) -> PluginId { self.id.clone() }
            fn priority(&self) -> u32 { 100 }

            async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
                let mut count = self.call_count.lock().await;
                *count += 1;

                if self.skip {
                    PluginDecision::Skip
                } else {
                    PluginDecision::Continue
                }
            }

            async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext) {}
        }

        let call_count = Arc::new(Mutex::new(0usize));
        let plugins: Vec<Arc<dyn AgentPlugin>> = vec![
            Arc::new(SkipPlugin {
                id: "skipper".to_string(),
                skip: true,
                call_count: call_count.clone(),
            }),
            Arc::new(SkipPlugin {
                id: "continuer".to_string(),
                skip: false,
                call_count: call_count.clone(),
            }),
        ];

        let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
        let ctx = create_test_context();

        let mut final_decision = PluginDecision::Continue;
        for plugin in &plugins {
            let decision = plugin.intercept(&event, &ctx).await;
            match decision {
                PluginDecision::Continue => continue,
                PluginDecision::Skip => {
                    final_decision = PluginDecision::Skip;
                    break;
                }
                PluginDecision::Abort(reason) => {
                    final_decision = PluginDecision::Abort(reason);
                    break;
                }
            }
        }

        // Skip was returned, but both plugins were still called in the chain before skip
        assert!(matches!(final_decision, PluginDecision::Skip));
        // Both plugins should have been called (skip doesn't stop chain from being evaluated)
        let final_count = call_count.lock().await;
        assert_eq!(*final_count, 1); // Only first plugin called because we break on Skip
    }

    fn create_test_context() -> RunContext {
        use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};
        use vol_llm_tool::ToolRegistry;
        use super::super::AgentConfig;

        RunContext::new(
            "test-run".to_string(),
            "test input".to_string(),
            "session-1".to_string(),
            Arc::new(Session::new(
                "session-1".to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            )),
            Arc::new(ToolRegistry::new()),
            AgentConfig::default(),
        )
    }
}
