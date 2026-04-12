//! Session error types.

use thiserror::Error;
use crate::store::StoreError;

/// Session operation error
#[derive(Debug, Error)]
pub enum SessionError {
    /// Store operation failed
    #[error("Store error: {0}")]
    StoreError(#[from] StoreError),

    /// Channel closed
    #[error("Event channel closed")]
    ChannelClosed,

    /// Channel lagged (missed events)
    #[error("Channel lagged, missed {0} events")]
    Lagged(usize),
}

pub type Result<T> = std::result::Result<T, SessionError>;
