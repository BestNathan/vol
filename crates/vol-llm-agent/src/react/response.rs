//! Agent response and error types.

use crate::react::state::ReasoningStep;
use serde::Serialize;
use thiserror::Error;
use vol_llm_core::LLMError;

/// Record of a single tool call during agent execution
#[derive(Debug, Clone, Serialize)]
pub struct ToolCallRecord {
    pub tool_name: String,
    pub arguments: String,
    pub result: String,
    pub iteration: u32,
    pub success: bool,
}

/// Agent response with full execution context
#[derive(Debug, Clone, Serialize)]
pub struct AgentResponse {
    /// Final answer content
    pub content: String,

    /// Complete reasoning chain (all thinking steps)
    pub reasoning: Vec<ReasoningStep>,

    /// Execution metadata
    pub run_id: String,
    pub session_id: String,
    pub iterations: u32,

    /// All tool calls made during execution
    pub tool_calls: Vec<ToolCallRecord>,

    /// Error information if any tool call failed
    pub error: Option<String>,
}

/// Agent error
#[derive(Debug, Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),

    #[error("Tool execution failed: {tool}: {error}")]
    ToolExecution { tool: String, error: String },

    #[error("Max iterations ({max}) reached without final response")]
    MaxIterationsReached { max: u32 },

    #[error("Invalid tool response: {0}")]
    InvalidToolResponse(String),

    #[error("Context error: {0}")]
    Context(String),

    #[error("Session error: {0}")]
    SessionError(String),
}
