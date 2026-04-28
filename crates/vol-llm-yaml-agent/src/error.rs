//! YAML agent error types.

use std::io;

/// Error type for YAML agent operations.
#[derive(Debug, thiserror::Error)]
pub enum YamlAgentError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Unknown plugin: {0}")]
    UnknownPlugin(String),

    #[error("LLM provider '{0}' not found")]
    LlmNotFound(String),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}
