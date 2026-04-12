//! Agent streaming receiver.
//!
//! Re-exports AgentStreamEvent from vol-llm-core.

pub use vol_llm_core::AgentStreamEvent;

/// Agent stream receiver
pub struct AgentStreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, super::response::AgentError>>,
}

impl AgentStreamReceiver {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, super::response::AgentError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<AgentStreamEvent, super::response::AgentError>> {
        self.rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_stream_receiver_creation() {
        // Just verify the type can be constructed
        let (_tx, rx) = tokio::sync::mpsc::channel(1);
        let _receiver = AgentStreamReceiver::new(rx);
    }
}
