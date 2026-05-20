// crates/vol-llm-agent-channel/src/connection.rs

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use vol_llm_agent::react::{AgentPlugin, AgentStreamEvent, PluginId, RunContext};

use crate::agent_server_protocol::{AgentOperation, AgentPayload, AgentServerMessage, MessageKind, Operation, Payload};
use crate::error::ConnectionError;

/// Abstract connection for agent communication.
/// Implement for each transport protocol.
#[async_trait]
pub trait Connection: Send + Sync + 'static {
    /// Protocol identifier (e.g., "ws", "memory").
    fn protocol(&self) -> &str;

    /// Receive the next incoming message.
    async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>>;

    /// Send a message.
    async fn send(&self, msg: AgentServerMessage) -> Result<(), ConnectionError>;
}

/// Registered as AgentPlugin on agent creation.
/// Holds at most one active connection at a time.
/// Agent and connection have independent lifecycles.
#[derive(Clone)]
pub struct ConnectionHolder {
    connection: Arc<RwLock<Option<Arc<dyn Connection>>>>,
    sender: String,
    receiver: String,
}

impl ConnectionHolder {
    /// Create a new empty holder.
    pub fn new(sender: String, receiver: String) -> Self {
        Self {
            connection: Arc::new(RwLock::new(None)),
            sender,
            receiver,
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

#[async_trait]
impl AgentPlugin for ConnectionHolder {
    fn id(&self) -> PluginId {
        "connection_holder".to_string()
    }

    fn priority(&self) -> u32 {
        50
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        if let Some(conn) = self.connection.read().await.as_ref() {
            let event_json = serde_json::to_value(event).unwrap_or(serde_json::Value::Null);
            let msg = AgentServerMessage {
                protocol: "agent-server/1".to_string(),
                message_id: uuid::Uuid::new_v4().to_string(),
                sender: self.sender.clone(),
                receiver: self.receiver.clone(),
                kind: MessageKind::Event,
                operation: Operation::Agent(AgentOperation::Event),
                payload: Payload::Agent(AgentPayload::Event {
                    run_id: ctx.run_id.clone(),
                    event: event_json,
                }),
                meta: Default::default(),
            };
            let _ = conn.send(msg).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A no-op Connection implementation for testing.
    struct MockConnection {
        protocol: String,
    }

    #[async_trait]
    impl Connection for MockConnection {
        fn protocol(&self) -> &str { &self.protocol }
        async fn recv(&self) -> Option<Result<AgentServerMessage, ConnectionError>> { None }
        async fn send(&self, _msg: AgentServerMessage) -> Result<(), ConnectionError> { Ok(()) }
    }

    #[tokio::test]
    async fn test_holder_new_is_empty() {
        let holder = ConnectionHolder::new("sender".to_string(), "receiver".to_string());
        assert!(!holder.is_connected().await);
    }

    #[tokio::test]
    async fn test_holder_attach() {
        let holder = ConnectionHolder::new("sender".to_string(), "receiver".to_string());
        let conn = Arc::new(MockConnection { protocol: "test".to_string() });

        holder.attach(conn.clone()).await;
        assert!(holder.is_connected().await);
        assert_eq!(holder.connection().await.unwrap().protocol(), "test");
    }

    #[tokio::test]
    async fn test_holder_detach_replaces_connection() {
        let holder = ConnectionHolder::new("sender".to_string(), "receiver".to_string());
        let conn1 = Arc::new(MockConnection { protocol: "test1".to_string() });
        let conn2 = Arc::new(MockConnection { protocol: "test2".to_string() });

        holder.attach(conn1).await;
        assert_eq!(holder.connection().await.unwrap().protocol(), "test1");

        holder.attach(conn2).await;
        assert_eq!(holder.connection().await.unwrap().protocol(), "test2");
    }

    #[tokio::test]
    async fn test_holder_detach_clears() {
        let holder = ConnectionHolder::new("sender".to_string(), "receiver".to_string());
        let conn = Arc::new(MockConnection { protocol: "test".to_string() });

        holder.attach(conn).await;
        holder.detach().await;
        assert!(!holder.is_connected().await);
    }
}
