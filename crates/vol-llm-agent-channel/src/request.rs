//! Agent request and result types.

use std::collections::HashMap;

use vol_llm_agent::AgentResponse;

use crate::error::ChannelError;

/// External request to an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// Unique run ID for one inference run.
    pub run_id: String,
    /// Target agent ID for routing.
    pub target_id: String,
    /// Sender agent ID (Some for agent-to-agent calls).
    pub sender_id: Option<String>,
    /// User input to pass to ReActAgent::run_with_id().
    pub input: String,
    /// Arbitrary metadata for this request.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AgentRequest {
    /// Create a new request with an auto-generated run_id.
    pub fn new(target_id: impl Into<String>, input: impl Into<String>) -> Self {
        Self::with_run_id(uuid::Uuid::new_v4().simple().to_string(), target_id, input)
    }

    /// Create a new request with a specific run_id.
    pub fn with_run_id(
        run_id: impl Into<String>,
        target_id: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            run_id: run_id.into(),
            target_id: target_id.into(),
            sender_id: None,
            input: input.into(),
            metadata: HashMap::new(),
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
    fn agent_request_with_run_id_sets_run_id() {
        let request = AgentRequest::with_run_id("run_123", "agent_a", "hello");

        assert_eq!(request.run_id, "run_123");
        assert_eq!(request.target_id, "agent_a");
        assert_eq!(request.input, "hello");
    }

    #[test]
    fn agent_request_new_generates_run_id() {
        let request = AgentRequest::new("agent_a", "hello");

        assert!(!request.run_id.is_empty());
    }
}
