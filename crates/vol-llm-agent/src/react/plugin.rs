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
}
