//! EventObserver trait for observing agent events.

use async_trait::async_trait;
use vol_llm_core::AgentStreamEvent;

use crate::coding::error::ObserverError;

/// Event observer trait - can be implemented for different backends
#[async_trait]
pub trait EventObserver: Send + Sync {
    /// Called when an agent event is emitted
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError>;

    /// Called when agent execution completes
    async fn on_complete(&self) -> Result<(), ObserverError>;
}
