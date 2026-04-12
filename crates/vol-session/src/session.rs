//! Session management.

/// Represents an active session.
#[derive(Debug, Clone)]
pub struct Session {
    /// Unique session ID.
    pub id: String,
}

impl Session {
    /// Create a new session.
    pub fn new(id: String) -> Self {
        Self { id }
    }
}
