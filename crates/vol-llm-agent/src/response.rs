//! Agent response and error types.

use thiserror::Error;
use vol_llm_core::{LLMError, ToolCall};

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

/// Agent streaming event
#[derive(Debug)]
pub enum AgentStreamEvent {
    /// Agent started execution
    AgentStart { input: String },

    /// LLM thinking completed
    ThinkingComplete { thinking: String },

    /// About to call tool
    ToolCallBegin { tool_name: String, arguments: String },

    /// Tool call completed
    ToolCallComplete { tool_name: String, result: String },

    /// One iteration completed (Reason-Act-Observation)
    IterationComplete {
        iteration: u32,
        tool_calls: Vec<vol_llm_core::ToolCall>,
        final_answer: Option<String>,
    },

    /// Agent execution completed
    AgentComplete { response: AgentResponse },

    /// Error occurred
    Error { error: AgentError },
}

/// Agent stream receiver
pub struct AgentStreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, AgentError>>,
}

impl AgentStreamReceiver {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, AgentError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<AgentStreamEvent, AgentError>> {
        self.rx.recv().await
    }
}
