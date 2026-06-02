//! Domain handler trait and type aliases.

use async_trait::async_trait;
use std::sync::Arc;

use crate::agent_server_protocol::{
    AgentServerMessage, Operation, ProtocolError,
};

/// Trait for domain handlers registered into AgentServerCore.
#[async_trait]
pub trait DomainHandler: Send + Sync + 'static {
    /// Unique name for debugging and logging.
    fn name(&self) -> &str;

    /// Operations this handler exclusively owns.
    /// Return an empty vec for handlers using string-based routing only.
    fn operations(&self) -> Vec<Operation>;

    /// Handle a message. The operation is embedded in `message.operation`.
    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError>;
}

/// Type alias for a registered handler.
pub type HandlerRef = Arc<dyn DomainHandler>;
