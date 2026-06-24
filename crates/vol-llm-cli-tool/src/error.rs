//! Error types for the cli-tool crate.

#[derive(Debug, thiserror::Error)]
pub enum CliToolError {
    #[error("config error: {0}")]
    Config(String),

    #[error("binary not allowed: first token `{token}` is not in {allowed:?}")]
    BinaryNotAllowed {
        token: String,
        allowed: Vec<String>,
    },

    #[error("invalid arguments: {0}")]
    InvalidArguments(String),

    #[error("sandbox execution failed: {0}")]
    SandboxFailed(String),

    #[error("command timed out after {0} seconds")]
    Timeout(u64),
}
