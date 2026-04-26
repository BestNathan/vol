//! Plugin system for ReAct Agent.
//!
//! Defines the AgentPlugin trait, PluginDecision, and PluginRegistry.
//! RunContext (defined in run_context.rs) is the context type passed to plugin hooks.

pub use vol_llm_core::AgentStreamEvent;
pub use super::run_context::RunContext;

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
#[async_trait::async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;

    fn priority(&self) -> u32 {
        100
    }

    /// Interceptor hook - can modify or block the event.
    /// Returns PluginDecision to continue, skip, or abort.
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }

    /// Listener hook - async, parallel, fire-and-forget.
    /// Called after event execution. Used for observability, logging, etc.
    async fn listen(&self, _event: &AgentStreamEvent, _ctx: &RunContext);
}

/// Plugin registry - manages plugin lifecycle and execution order
#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Vec<std::sync::Arc<dyn AgentPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    pub fn register<P: AgentPlugin + 'static>(&mut self, plugin: P) {
        let plugin = std::sync::Arc::new(plugin);
        let pos = self
            .plugins
            .iter()
            .position(|p| p.priority() > plugin.priority())
            .unwrap_or(self.plugins.len());
        self.plugins.insert(pos, plugin);
    }

    pub fn plugins(&self) -> &[std::sync::Arc<dyn AgentPlugin>] {
        &self.plugins
    }

    pub fn get(&self, id: &str) -> Option<&std::sync::Arc<dyn AgentPlugin>> {
        self.plugins.iter().find(|p| p.id() == id)
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}
