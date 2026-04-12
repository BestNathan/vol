//! LLM error types.

use std::time::Duration;
use thiserror::Error;

/// LLM error
#[derive(Debug, Error)]
pub enum LLMError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Rate limit exceeded. Retry after {retry_after:?}")]
    RateLimit { retry_after: Option<Duration> },

    #[error("Invalid response format: {0}")]
    Parse(String),

    #[error("Request timeout: {0}")]
    Timeout(String),

    #[error("Parameter '{param}' is not supported by this provider")]
    UnsupportedParam { param: String },

    #[error("Tool call error: {0}")]
    ToolCall(String),

    #[error("Content was filtered: {reason}")]
    ContentFiltered { reason: String },
}

pub type Result<T> = std::result::Result<T, LLMError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LLMError::Timeout("test".to_string());
        assert!(err.to_string().contains("timeout"));
    }
}
