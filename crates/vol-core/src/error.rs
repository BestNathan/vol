use thiserror::Error;

/// Core error types for the volatility monitoring system
#[derive(Debug, Error)]
pub enum VolError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Alert error: {0}")]
    Alert(String),

    #[error("Notification error: {0}")]
    Notification(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, VolError>;
