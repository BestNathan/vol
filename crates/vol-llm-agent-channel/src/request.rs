//! Agent request and result types.

use std::collections::HashMap;

use vol_llm_agent::{AgentInput, AgentResponse};

use crate::error::ChannelError;

/// External request to an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// Unique request ID (caller-provided or auto-generated).
    pub req_id: String,
    /// Target agent ID for routing.
    pub target_id: String,
    /// Sender agent ID (Some for agent-to-agent calls).
    pub sender_id: Option<String>,
    /// User input to pass to ReActAgent::run_input().
    pub input: AgentInput,
    /// Arbitrary metadata for this request.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AgentRequest {
    /// Create a new request with an auto-generated req_id.
    pub fn new(target_id: impl Into<String>, input: impl Into<String>) -> Self {
        Self::with_input(target_id, AgentInput::text(input.into()))
    }

    /// Create a new request with structured input and an auto-generated req_id.
    pub fn with_input(target_id: impl Into<String>, input: AgentInput) -> Self {
        Self {
            req_id: uuid::Uuid::new_v4().simple().to_string(),
            target_id: target_id.into(),
            sender_id: None,
            input,
            metadata: HashMap::new(),
        }
    }

    /// Create a new request with a specific req_id.
    pub fn with_id(
        req_id: impl Into<String>,
        target_id: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self::with_id_and_input(req_id, target_id, AgentInput::text(input.into()))
    }

    /// Create a new request with a specific req_id and structured input.
    pub fn with_id_and_input(
        req_id: impl Into<String>,
        target_id: impl Into<String>,
        input: AgentInput,
    ) -> Self {
        Self {
            req_id: req_id.into(),
            target_id: target_id.into(),
            sender_id: None,
            input,
            metadata: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::{AgentInput, InputPart};

    #[test]
    fn text_constructor_wraps_input_as_agent_input() {
        let request = AgentRequest::new("agent-a", "hello");
        assert_eq!(request.input, AgentInput::text("hello"));
    }

    #[test]
    fn structured_constructor_preserves_parts() {
        let input = AgentInput::new()
            .text_part("look")
            .image_url("data:image/png;base64,AAAA");
        let request = AgentRequest::with_input("agent-a", input.clone());
        assert_eq!(request.input, input);
        assert!(matches!(request.input.parts[1], InputPart::ImageUrl { .. }));
    }
}

/// Result delivered to the sender after execution.
#[derive(Debug)]
pub struct RunResult {
    /// Original request ID.
    pub req_id: String,
    /// Target agent that processed this.
    pub target_id: String,
    /// Internal run_id from ReActAgent (only present on success).
    pub run_id: Option<String>,
    /// The agent response or error.
    pub response: Result<AgentResponse, ChannelError>,
}

/// Internal wrapper for a queued request awaiting execution.
pub(crate) struct PendingRequest {
    pub request: AgentRequest,
    pub tx: tokio::sync::oneshot::Sender<RunResult>,
}
