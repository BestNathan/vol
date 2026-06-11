//! Channel error types.

#[derive(Debug, thiserror::Error)]
pub enum ChannelError {
    /// Target agent was not found by the routing layer.
    #[error("agent '{0}' not registered")]
    AgentNotFound(String),

    /// Request was cancelled before execution.
    #[error("request '{0}' was cancelled")]
    Cancelled(String),

    /// Request handler dropped while request was pending.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_error_agent_not_found() {
        let err = ChannelError::AgentNotFound("agent-a".into());
        assert_eq!(err.to_string(), "agent 'agent-a' not registered");
    }

    #[test]
    fn channel_error_cancelled() {
        let err = ChannelError::Cancelled("req-1".into());
        assert_eq!(err.to_string(), "request 'req-1' was cancelled");
    }

    #[test]
    fn channel_error_dispatcher_dropped() {
        let err = ChannelError::DispatcherDropped;
        assert_eq!(err.to_string(), "dispatcher dropped");
    }

    #[test]
    fn channel_error_agent_error() {
        let err = ChannelError::AgentError("timeout".into());
        assert_eq!(err.to_string(), "agent execution error: timeout");
    }

    #[test]
    fn channel_error_agent_busy() {
        let err = ChannelError::AgentBusy("agent-a".into());
        assert_eq!(err.to_string(), "agent is busy: agent-a");
    }

    #[test]
    fn connection_error_ws_send() {
        let err = ConnectionError::WsSendError("connection reset".into());
        assert_eq!(err.to_string(), "websocket send error: connection reset");
    }

    #[test]
    fn connection_error_ws_receive() {
        let err = ConnectionError::WsReceiveError("timeout".into());
        assert_eq!(err.to_string(), "websocket receive error: timeout");
    }

    #[test]
    fn connection_error_parse() {
        let err = ConnectionError::ParseError("invalid JSON".into());
        assert_eq!(err.to_string(), "parse error: invalid JSON");
    }

    #[test]
    fn connection_error_closed() {
        let err = ConnectionError::Closed;
        assert_eq!(err.to_string(), "connection closed");
    }

    #[test]
    fn connection_error_channel() {
        let err = ConnectionError::ChannelError("buffer full".into());
        assert_eq!(err.to_string(), "channel send error: buffer full");
    }
}
