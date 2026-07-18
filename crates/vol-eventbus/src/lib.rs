//! vol-eventbus: Event bus implementation using tokio broadcast channels.

use std::sync::Arc;
use tokio::sync::broadcast;
use vol_core::{Alert, VolError, VolatilityData};

/// Event types that flow through the event bus
#[derive(Debug, Clone)]
pub enum Event {
    /// New volatility data received
    Data(VolatilityData),
    /// Alert triggered
    Alert(Alert),
    /// System event (startup, shutdown, error)
    System(SystemEvent),
}

/// System events
#[derive(Debug, Clone)]
pub enum SystemEvent {
    Started,
    Stopped,
    DataSourceConnected(String),
    DataSourceDisconnected(String),
    Error(String),
}

/// Event bus for publishing and subscribing to events
pub struct EventBus {
    data_tx: broadcast::Sender<Arc<VolatilityData>>,
    alert_tx: broadcast::Sender<Arc<Alert>>,
    system_tx: broadcast::Sender<Arc<SystemEvent>>,
}

impl EventBus {
    /// Create a new event bus with given channel capacity
    pub fn new(capacity: usize) -> Self {
        let (data_tx, _) = broadcast::channel(capacity);
        let (alert_tx, _) = broadcast::channel(capacity);
        let (system_tx, _) = broadcast::channel(capacity);

        Self {
            data_tx,
            alert_tx,
            system_tx,
        }
    }

    /// Publish volatility data to all subscribers
    pub fn publish_data(&self, data: VolatilityData) -> Result<(), VolError> {
        self.data_tx
            .send(Arc::new(data))
            .map_err(|e| VolError::Internal(format!("Failed to publish data: {e}")))?;
        Ok(())
    }

    /// Publish an alert to all subscribers
    pub fn publish_alert(&self, alert: Alert) -> Result<(), VolError> {
        self.alert_tx
            .send(Arc::new(alert))
            .map_err(|e| VolError::Internal(format!("Failed to publish alert: {e}")))?;
        Ok(())
    }

    /// Publish a system event
    pub fn publish_system(&self, event: SystemEvent) -> Result<(), VolError> {
        self.system_tx
            .send(Arc::new(event))
            .map_err(|e| VolError::Internal(format!("Failed to publish system event: {e}")))?;
        Ok(())
    }

    /// Subscribe to volatility data events
    pub fn subscribe_data(&self) -> broadcast::Receiver<Arc<VolatilityData>> {
        self.data_tx.subscribe()
    }

    /// Subscribe to alert events
    pub fn subscribe_alerts(&self) -> broadcast::Receiver<Arc<Alert>> {
        self.alert_tx.subscribe()
    }

    /// Subscribe to system events
    pub fn subscribe_system(&self) -> broadcast::Receiver<Arc<SystemEvent>> {
        self.system_tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(1024)
    }
}
