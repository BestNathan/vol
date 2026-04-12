//! Session event listener for event-driven message recording.

use crate::SessionMessage;

/// Event-driven session listener for message recording.
pub struct SessionListener {
    /// Placeholder for listener state.
    #[allow(dead_code)]
    enabled: bool,
}

impl SessionListener {
    /// Create a new session listener.
    pub fn new(enabled: bool) -> Self {
        Self { enabled }
    }

    /// Record a session message.
    pub fn on_message(&self, _message: SessionMessage) {
        // TODO: Implement event-driven message recording
    }
}
