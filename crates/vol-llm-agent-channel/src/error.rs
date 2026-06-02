//! Channel error types.

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    /// Target agent not found in router.
    #[error("agent '{0}' not registered")]
    AgentNotFound(String),

    /// Request was cancelled before execution.
    #[error("request '{0}' was cancelled")]
    Cancelled(String),

    /// Dispatcher dropped while request was pending.
    #[error("dispatcher dropped")]
    DispatcherDropped,

    /// Internal agent error (from ReActAgent::run).
    #[error("agent execution error: {0}")]
    AgentError(String),

    /// Agent is busy (running) and cannot accept state mutation.
    #[error("agent is busy: {0}")]
    AgentBusy(String),
}

/// Error type for connection operations.
#[derive(Debug, thiserror::Error)]
pub enum ConnectionError {
    /// WebSocket send failed.
    #[error("websocket send error: {0}")]
    WsSendError(String),

    /// WebSocket receive failed.
    #[error("websocket receive error: {0}")]
    WsReceiveError(String),

    /// Failed to parse message.
    #[error("parse error: {0}")]
    ParseError(String),

    /// Connection was closed.
    #[error("connection closed")]
    Closed,

    /// Channel send failed (in-memory transport).
    #[error("channel send error: {0}")]
    ChannelError(String),
}
