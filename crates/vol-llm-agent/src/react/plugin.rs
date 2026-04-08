//! Plugin system for ReAct Agent.
//!
//! Plugins can intercept and modify the agent event stream,
//! implement cross-cutting concerns like observability, caching, etc.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use super::{AgentStreamEvent, AgentResponse, AgentError};

/// Plugin unique identifier
pub type PluginId = String;

/// Stream event type alias
pub type StreamEvent = Result<AgentStreamEvent, AgentError>;

/// Plugin context - shared state passed through plugin pipeline
#[derive(Debug, Clone)]
pub struct PluginContext {
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    data: HashMap<String, serde_json::Value>,
}

impl PluginContext {
    pub fn new(run_id: String, user_input: String, session_id: String) -> Self {
        Self {
            run_id,
            user_input,
            session_id,
            data: HashMap::new(),
        }
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data.get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub fn set<T: Serialize>(&mut self, key: &str, value: T) -> Result<(), serde_json::Error> {
        self.data.insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }
}

/// Action returned by plugin hooks
#[derive(Debug)]
pub enum PluginAction<T = ()> {
    Continue(T),
    ShortCircuit(AgentResponse),
    Skip,
    Abort(AgentError),
}

impl<T> PluginAction<T> {
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> PluginAction<U> {
        match self {
            PluginAction::Continue(v) => PluginAction::Continue(f(v)),
            PluginAction::ShortCircuit(r) => PluginAction::ShortCircuit(r),
            PluginAction::Skip => PluginAction::Skip,
            PluginAction::Abort(e) => PluginAction::Abort(e),
        }
    }

    pub fn map_err<F: FnOnce(AgentError) -> AgentError>(self, f: F) -> PluginAction<T> {
        match self {
            PluginAction::Continue(v) => PluginAction::Continue(v),
            PluginAction::ShortCircuit(r) => PluginAction::ShortCircuit(r),
            PluginAction::Skip => PluginAction::Skip,
            PluginAction::Abort(e) => PluginAction::Abort(f(e)),
        }
    }
}

/// Plugin trait for extending agent functionality
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;

    fn priority(&self) -> u32 { 100 }

    /// Called before agent execution starts
    /// Return ShortCircuit to skip actual execution and return cached/synthetic response
    async fn on_start(&self, _ctx: &mut PluginContext) -> PluginAction<()> {
        PluginAction::Continue(())
    }

    /// Called for each event in the stream
    /// Return Ok(None) to drop the event
    /// Return ShortCircuit to replace remaining stream with the given response
    async fn intercept(
        &self,
        event: StreamEvent,
        ctx: &PluginContext,
    ) -> PluginAction<Option<StreamEvent>>;

    /// Called when agent completes successfully
    async fn on_complete(
        &self,
        ctx: &PluginContext,
        final_response: Option<&AgentResponse>,
    ) -> PluginAction<()>;

    /// Called when agent encounters an error
    async fn on_error(
        &self,
        _ctx: &PluginContext,
        _error: &AgentError,
    ) -> PluginAction<()> {
        PluginAction::Continue(())
    }
}

/// Plugin registry - manages plugin lifecycle and execution order
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
        async fn intercept(&self, event: StreamEvent, _ctx: &PluginContext) -> PluginAction<Option<StreamEvent>> {
            PluginAction::Continue(Some(event))
        }
        async fn on_complete(&self, _ctx: &PluginContext, _response: Option<&AgentResponse>) -> PluginAction<()> {
            PluginAction::Continue(())
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
    fn test_plugin_action_map() {
        let action: PluginAction<i32> = PluginAction::Continue(42);
        let mapped = action.map(|x| x * 2);
        assert!(matches!(mapped, PluginAction::Continue(84)));
    }
}
