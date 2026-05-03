// crates/vol-llm-agent-channel/src/connection.rs

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use vol_llm_agent::react::{AgentPlugin, AgentStreamEvent, PluginId, RunContext};

use crate::error::ConnectionError;
use crate::protocol::InboundMessage;
use crate::request::RunResult;

/// Abstract connection for agent communication.
/// Implement for each transport protocol.
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    /// Protocol identifier (e.g., "ws", "memory").
    fn protocol(&self) -> &str;

    /// Receive the next incoming message from the client.
    async fn recv(&mut self) -> Option<Result<InboundMessage, ConnectionError>>;

    /// Send an agent streaming event to the client.
    async fn send_event(&self, event: &AgentStreamEvent) -> Result<(), ConnectionError>;

    /// Send the final run result to the client.
    async fn send_result(&self, result: &RunResult) -> Result<(), ConnectionError>;
}

/// Registered as AgentPlugin on agent creation.
/// Holds at most one active connection at a time.
/// Agent and connection have independent lifecycles.
pub struct ConnectionHolder {
    connection: Arc<RwLock<Option<Arc<dyn Connection>>>>,
}

impl ConnectionHolder {
    /// Create a new empty holder.
    pub fn new() -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
        }
    }

    /// Attach a connection. Detaches existing one first.
    pub async fn attach(&self, conn: Arc<dyn Connection>) {
        self.detach().await;
        *self.connection.write().await = Some(conn);
    }

    /// Detach current connection (if any).
    pub async fn detach(&self) {
        *self.connection.write().await = None;
    }

    /// Whether a connection is currently active.
    pub async fn is_connected(&self) -> bool {
        self.connection.read().await.is_some()
    }

    /// Get the current connection (for testing).
    pub async fn connection(&self) -> Option<Arc<dyn Connection>> {
        self.connection.read().await.clone()
    }
}

impl Default for ConnectionHolder {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentPlugin for ConnectionHolder {
    fn id(&self) -> PluginId {
        "connection_holder".to_string()
    }

    fn priority(&self) -> u32 {
        50
    }

    async fn listen(&self, event: &AgentStreamEvent, _ctx: &RunContext) {
        if let Some(conn) = self.connection.read().await.as_ref() {
            let _ = conn.send_event(event).await;
        }
    }
}
