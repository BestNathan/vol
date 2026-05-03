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
}
