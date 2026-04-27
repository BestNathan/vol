//! WikiAgent error types.

use thiserror::Error;

/// WikiAgent unified error type.
#[derive(Debug, Error)]
pub enum WikiAgentError {
    #[error("Agent error: {0}")]
    Agent(#[from] vol_llm_agent::AgentError),

    #[error("Tool error: {0}")]
    Tool(#[from] vol_llm_tool::ToolError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Compression failed: {0}")]
    CompressionFailed(String),
}
