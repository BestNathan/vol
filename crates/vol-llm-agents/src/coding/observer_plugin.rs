//! ObserverPlugin - wraps EventObserver and integrates with PluginRegistry.

use async_trait::async_trait;
use std::sync::Arc;
use vol_llm_agent::react::{AgentPlugin, PluginContext};
use vol_llm_agent::react::plugin::PluginId;
use vol_llm_core::AgentStreamEvent;

use crate::coding::observer::EventObserver;

/// ObserverPlugin - wraps EventObserver and implements AgentPlugin
pub struct ObserverPlugin {
    observer: Arc<dyn EventObserver>,
}

impl ObserverPlugin {
    /// Create a new ObserverPlugin
    pub fn new(observer: Arc<dyn EventObserver>) -> Self {
        Self { observer }
    }

    /// Get the wrapped observer
    pub fn observer(&self) -> &Arc<dyn EventObserver> {
        &self.observer
    }
}

#[async_trait]
impl AgentPlugin for ObserverPlugin {
    fn id(&self) -> PluginId {
        "observer".to_string()
    }

    fn priority(&self) -> u32 {
        0 // Low priority value = high priority, runs first
    }

    async fn listen(&self, event: &AgentStreamEvent, _ctx: &PluginContext) {
        // Forward to observer, ignore errors to not block other plugins
        let _ = self.observer.on_event(event).await;
    }
}
