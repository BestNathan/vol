//! Coding Agent error types.

use thiserror::Error;

/// Coding Agent unified error type
#[derive(Debug, Error)]
pub enum CodingAgentError {
    #[error("Agent error: {0}")]
    Agent(#[from] vol_llm_agent::AgentError),

    #[error("Tool error: {0}")]
    Tool(#[from] vol_llm_tool::ToolError),

    #[error("Observer error: {0}")]
    Observer(#[from] ObserverError),

    #[error("HITL error: {0}")]
    HITL(#[from] HITLError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Task failed: {0}")]
    TaskFailed(String),
}

/// Observer subsystem error
#[derive(Debug, Error)]
pub enum ObserverError {
    #[error("Failed to record event: {0}")]
    RecordFailed(String),

    #[error("Failed to generate report: {0}")]
    ReportFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// HITL subsystem error
#[derive(Debug, Error)]
pub enum HITLError {
    #[error("User rejected: {0}")]
    Rejected(String),

    #[error("Timeout waiting for response")]
    Timeout,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
