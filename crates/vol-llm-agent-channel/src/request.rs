//! Agent request and result types.

use std::collections::HashMap;

use vol_llm_agent::AgentResponse;

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
    /// User input to pass to ReActAgent::run().
    pub input: String,
    /// Arbitrary metadata for this request.
    pub metadata: HashMap<String, serde_json::Value>,
}

impl AgentRequest {
    /// Create a new request with an auto-generated req_id.
    pub fn new(target_id: impl Into<String>, input: impl Into<String>) -> Self {
        Self {
            req_id: uuid::Uuid::new_v4().simple().to_string(),
            target_id: target_id.into(),
            sender_id: None,
            input: input.into(),
            metadata: HashMap::new(),
        }
    }

    /// Create a new request with a specific req_id.
    pub fn with_id(
        req_id: impl Into<String>,
        target_id: impl Into<String>,
        input: impl Into<String>,
    ) -> Self {
        Self {
            req_id: req_id.into(),
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
