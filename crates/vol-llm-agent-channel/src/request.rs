//! Agent request and result types.

use vol_llm_agent::{AgentInput, AgentResponse};

use crate::error::ChannelError;

/// External request to an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// Target agent ID for routing.
    pub target_id: String,
    /// Sender agent ID (Some for agent-to-agent calls).
    pub sender_id: Option<String>,
    /// Input to pass to ReActAgent::run_input().
    pub input: AgentInput,
}

impl AgentRequest {
    /// Create a new request.
    pub fn new(target_id: impl Into<String>, input: AgentInput) -> Self {
        Self {
            target_id: target_id.into(),
            sender_id: None,
            input,
        }
    }
}

/// Result delivered to the sender after execution.
#[derive(Debug)]
pub struct RunResult {
    /// Run ID for one inference run.
    pub run_id: String,
    /// Target agent that processed this.
    pub target_id: String,
    /// The agent response or error.
    pub response: Result<AgentResponse, ChannelError>,
}

/// Internal wrapper for a queued request awaiting execution.
pub(crate) struct PendingRequest {
    pub request: AgentRequest,
    pub tx: tokio::sync::oneshot::Sender<RunResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_request_new_stores_input() {
        let input = AgentInput::text("hello");
        let request = AgentRequest::new("agent_a", input);

        assert_eq!(request.target_id, "agent_a");
        assert_eq!(request.input.display_text(), "hello");
    }

    #[test]
    fn agent_request_with_run_id_on_input() {
        let input = AgentInput::text("hello").with_run_id("run_123");
        let request = AgentRequest::new("agent_a", input);

        assert_eq!(request.input.run_id.as_deref(), Some("run_123"));
        assert_eq!(request.input.display_text(), "hello");
    }
}
