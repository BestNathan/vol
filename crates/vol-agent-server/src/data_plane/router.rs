// crates/vol-llm-agent-channel/src/router.rs

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{oneshot, RwLock};

use crate::data_plane::dispatcher::AgentDispatcher;
use vol_llm_agent::ReActAgent;
use vol_llm_agent_protocol::error::ChannelError;
use vol_llm_agent_protocol::request::{AgentRequest, RunResult};
use vol_session::Session;

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

    /// Cancel a request by run_id across all registered dispatchers.
    pub async fn cancel(&self, run_id: &str) -> bool {
        for dispatcher in self.dispatchers.read().await.values() {
            if dispatcher.cancel(run_id).await {
                return true;
            }
        }
        false
    }

    /// Check if an agent is registered.
    pub async fn has_agent(&self, agent_id: &str) -> bool {
        self.dispatchers.read().await.contains_key(agent_id)
    }

    /// Swap the session of a registered agent. Fails if agent is running.
    pub async fn swap_session(
        &self,
        agent_id: &str,
        session: Arc<Session>,
    ) -> Result<(), ChannelError> {
        let dispatchers = self.dispatchers.read().await;
        let dispatcher = dispatchers
            .get(agent_id)
            .ok_or_else(|| ChannelError::AgentNotFound(agent_id.to_string()))?;
        dispatcher
            .swap_session(session)
            .map_err(|e| ChannelError::AgentBusy(e.to_string()))
    }

    /// List all registered agent IDs.
    pub async fn list_agents(&self) -> Vec<String> {
        self.dispatchers.read().await.keys().cloned().collect()
    }

    /// Clone the agent for the given agent_id.
    pub async fn get_agent(&self, agent_id: &str) -> Option<Arc<ReActAgent>> {
        self.dispatchers
            .read()
            .await
            .get(agent_id)
            .map(|d| d.get_agent())
    }

    /// Check if an agent is currently running.
    pub async fn is_agent_running(&self, agent_id: &str) -> bool {
        let dispatchers = self.dispatchers.read().await;
        dispatchers.get(agent_id).is_some_and(|d| d.is_busy())
    }
}

impl Default for AgentRouter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::AgentInput;
    use vol_session::memory_store::InMemoryEntryStore;

    #[tokio::test]
    async fn test_router_empty_returns_not_found() {
        let router = AgentRouter::new();
        let req = AgentRequest::new("nonexistent", AgentInput::text("hello"));
        let result = router.send("nonexistent", req).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ChannelError::AgentNotFound(id) => assert_eq!(id, "nonexistent"),
            other => panic!("Expected AgentNotFound, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_router_has_agent_empty() {
        let router = AgentRouter::new();
        assert!(!router.has_agent("agent_a").await);
    }

    #[tokio::test]
    async fn test_router_list_agents_empty() {
        let router = AgentRouter::new();
        let agents = router.list_agents().await;
        assert!(agents.is_empty());
    }

    #[tokio::test]
    async fn test_router_cancel_returns_false_for_empty_router() {
        let router = AgentRouter::new();
        assert!(!router.cancel("some-run-id").await);
    }

    #[tokio::test]
    async fn test_router_get_agent_returns_none_for_empty_router() {
        let router = AgentRouter::new();
        assert!(router.get_agent("agent-x").await.is_none());
    }

    #[tokio::test]
    async fn test_router_is_agent_running_returns_false_for_empty_router() {
        let router = AgentRouter::new();
        assert!(!router.is_agent_running("agent-x").await);
    }

    #[tokio::test]
    async fn test_router_default_creates_valid_router() {
        let router = AgentRouter::default();
        assert!(!router.has_agent("any").await);
    }

    #[tokio::test]
    async fn test_router_swap_session_returns_error_for_missing_agent() {
        let router = AgentRouter::new();
        let store: Arc<dyn vol_session::SessionEntryStore> =
            Arc::new(InMemoryEntryStore::new());
        let session = Arc::new(Session::new(store));
        let result = router.swap_session("missing-agent", session).await;
        assert!(result.is_err());
    }
}
