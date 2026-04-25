//! Plugin system for ReAct Agent.
//!
//! Plugins can intercept and modify the agent event stream,
//! implement cross-cutting concerns like observability, caching, etc.

use crate::stream::AgentStreamEvent;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

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

/// PluginContext - Read-only context for plugin hooks.
///
/// This struct contains all the data plugins need for `intercept()` and `listen()`
/// hooks, but EXCLUDES the broadcast channel senders (`event_tx`, `plugin_event_tx`).
#[derive(Clone)]
pub struct PluginContext {
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    pub all_tool_calls: Arc<RwLock<Vec<crate::ToolCall>>>,
    pub current_tool_calls: Arc<RwLock<Vec<crate::ToolCall>>>,
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}

impl PluginContext {
    /// Get a clone of current tool calls
    pub async fn get_current_tool_calls(&self) -> Vec<crate::ToolCall> {
        self.current_tool_calls.read().await.clone()
    }

    /// Get a clone of all tool calls
    pub async fn get_all_tool_calls(&self) -> Vec<crate::ToolCall> {
        self.all_tool_calls.read().await.clone()
    }

    /// Get a value from the data store
    pub async fn get<T: for<'de> serde::Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data
            .read()
            .await
            .get(key)
            .and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    /// Set a value in the data store
    pub async fn set<T: serde::Serialize>(
        &self,
        key: &str,
        value: T,
    ) -> Result<(), serde_json::Error> {
        self.data
            .write()
            .await
            .insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }
}

/// Plugin trait for extending agent functionality
#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;

    fn priority(&self) -> u32 {
        100
    }

    /// Interceptor hook - can modify or block the event.
    /// Returns PluginDecision to continue, skip, or abort.
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &PluginContext) -> PluginDecision {
        PluginDecision::Continue // Default: no-op
    }

    /// Listener hook - async, parallel, fire-and-forget.
    /// Called after event execution. Used for observability, logging, etc.
    async fn listen(&self, _event: &AgentStreamEvent, _ctx: &PluginContext);
}

/// Plugin registry - manages plugin lifecycle and execution order
#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn AgentPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self {
            plugins: Vec::new(),
        }
    }

    pub fn register<P: AgentPlugin + 'static>(&mut self, plugin: P) {
        let plugin = Arc::new(plugin);
        // Insert by priority (lower number = higher priority = executed first)
        let pos = self
            .plugins
            .iter()
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
