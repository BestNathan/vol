use tokio::sync::broadcast;

#[derive(Debug, Clone)]
pub struct ControlPlaneEvent {
    pub event_type: String,
    pub node_id: Option<String>,
}

#[derive(Clone)]
pub struct EventBus {
    tx: broadcast::Sender<ControlPlaneEvent>,
}

impl EventBus {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self { tx }
    }

    pub fn publish(&self, event: ControlPlaneEvent) {
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ControlPlaneEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn event_bus_publish_and_subscribe() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        bus.publish(ControlPlaneEvent {
            event_type: "node_connected".to_string(),
            node_id: Some("node-a".to_string()),
        });

        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "node_connected");
        assert_eq!(event.node_id, Some("node-a".to_string()));
    }

    #[tokio::test]
    async fn event_bus_multiple_subscribers() {
        let bus = EventBus::new();
        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(ControlPlaneEvent {
            event_type: "heartbeat".to_string(),
            node_id: None,
        });

        let event1 = rx1.recv().await.unwrap();
        let event2 = rx2.recv().await.unwrap();
        assert_eq!(event1.event_type, "heartbeat");
        assert_eq!(event2.event_type, "heartbeat");
    }

    #[tokio::test]
    async fn event_bus_lagging_receiver_misses_events() {
        let bus = EventBus::new();
        let mut rx = bus.subscribe();

        // Fill buffer beyond capacity to test lag behavior
        for i in 0..300 {
            bus.publish(ControlPlaneEvent {
                event_type: format!("event-{i}"),
                node_id: None,
            });
        }

        // The receiver will have lagged; recv returns Err(Lagged)
        let result = rx.recv().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn event_bus_default_creates_new_bus() {
        let bus = EventBus::default();
        let mut rx = bus.subscribe();
        bus.publish(ControlPlaneEvent {
            event_type: "default_test".to_string(),
            node_id: None,
        });
        let event = rx.recv().await.unwrap();
        assert_eq!(event.event_type, "default_test");
    }
}
