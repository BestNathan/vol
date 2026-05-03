// crates/vol-llm-agent-channel/src/router.rs

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{RwLock, oneshot};

use crate::dispatcher::AgentDispatcher;
use crate::error::ChannelError;
use crate::request::{AgentRequest, RunResult};

/// Routes requests to registered dispatchers by agent_id.
///
/// Clone to share across tasks (internally Arc-backed).
#[derive(Clone)]
pub struct AgentRouter {
    dispatchers: Arc<RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
}

impl AgentRouter {
    /// Create a new empty router.
    pub fn new() -> Self {
        Self {
            dispatchers: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a dispatcher for the given agent_id.
    pub async fn register(&self, agent_id: String, dispatcher: Arc<AgentDispatcher>) {
        self.dispatchers.write().await.insert(agent_id, dispatcher);
    }

    /// Send a request to a target agent. Returns a receiver for the result.
    ///
    /// The request's `target_id` field is updated to match the resolved dispatcher.
    pub async fn send(
        &self,
        target_id: &str,
        request: AgentRequest,
    ) -> Result<oneshot::Receiver<RunResult>, ChannelError> {
        let dispatchers = self.dispatchers.read().await;
        let dispatcher = dispatchers
            .get(target_id)
            .ok_or_else(|| ChannelError::AgentNotFound(target_id.to_string()))?;

        dispatcher.submit(request)
    }

    /// Check if an agent is registered.
    pub async fn has_agent(&self, agent_id: &str) -> bool {
        self.dispatchers.read().await.contains_key(agent_id)
    }

    /// List all registered agent IDs.
    pub async fn list_agents(&self) -> Vec<String> {
        self.dispatchers.read().await.keys().cloned().collect()
    }
}

impl Default for AgentRouter {
    fn default() -> Self {
        Self::new()
    }
}
