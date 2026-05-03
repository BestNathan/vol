//! Channel error types.

use thiserror::Error;

// Placeholder — to be implemented in subsequent tasks.

/// Placeholder error type.
#[derive(Debug, Error)]
#[error("channel error: {0}")]
pub struct ChannelError(String);
