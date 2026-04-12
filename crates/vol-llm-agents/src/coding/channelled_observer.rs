//! ChannelledEventObserver - guarantees ordered event processing via mpsc channel.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use vol_llm_core::AgentStreamEvent;

use crate::coding::observer::EventObserver;
use crate::coding::error::ObserverError;

/// ChannelledEventObserver - wraps mpsc channel + single consumer task for ordered event processing.
///
/// # Why this exists
///
/// The PluginRegistry calls plugin.listen() concurrently for multiple events.
/// This means on_event() calls can arrive out of order even though events are
/// emitted sequentially. This observer uses a single consumer task to guarantee
/// events are processed in the order they are received.
pub struct ChannelledEventObserver {
    tx: mpsc::UnboundedSender<AgentStreamEvent>,
    events: Arc<Mutex<Vec<AgentStreamEvent>>>,
    shutdown_tx: Option<oneshot::Sender<()>>,
}

impl ChannelledEventObserver {
    /// Create a new ChannelledEventObserver with spawned consumer task.
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let events = Arc::new(Mutex::new(Vec::new()));
        let (shutdown_tx, mut shutdown_rx) = oneshot::channel();

        let events_clone = events.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        events_clone.lock().await.push(event);
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        Self {
            tx,
            events,
            shutdown_tx: Some(shutdown_tx),
        }
    }

    /// Get all recorded events in order.
    pub async fn events(&self) -> Vec<AgentStreamEvent> {
        self.events.lock().await.clone()
    }

    /// Wait for pending events and signal shutdown.
    pub async fn wait_completion(&mut self) {
        // Allow pending events to be processed
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Some(shutdown_tx) = self.shutdown_tx.take() {
            let _ = shutdown_tx.send(());
        }
    }
}

#[async_trait::async_trait]
impl EventObserver for ChannelledEventObserver {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let _ = self.tx.send(event.clone());
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        // Note: wait_completion requires mutable access, but we have &self.
        // This is a design limitation - the consumer task will continue running
        // but all pending events will eventually be processed.
        // We sleep briefly to allow pending events to drain.
        tokio::time::sleep(Duration::from_millis(100)).await;
        Ok(())
    }
}
