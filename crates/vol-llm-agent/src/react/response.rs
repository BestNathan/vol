//! Agent response and error types.

use thiserror::Error;
use vol_llm_core::LLMError;
use vol_llm_core::ToolCall;

/// Agent response
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub reasoning: String,
    pub iterations: u32,
    pub tool_calls: Vec<ToolCall>,
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
}
