//! Session message types.

use serde::{Deserialize, Serialize};

/// A message within a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Unique message ID.
    pub id: String,
    /// The message content.
    pub content: String,
}

impl SessionMessage {
    /// Create a new session message.
    pub fn new(id: String, content: String) -> Self {
        Self { id, content }
    }
}
