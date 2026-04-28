//! Error types stub

/// Error type for YAML agent operations.
#[derive(Debug, thiserror::Error)]
pub enum YamlAgentError {
    #[error("YAML parse error: {0}")]
    YamlError(#[from] serde_yaml::Error),
}
