//! Subscription manager for multi-channel dispatch over single WebSocket connection.

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::sync::Mutex;
use crate::{ChannelType, ChannelData};

/// Thread-safe subscription registry
pub struct SubscriptionManager {
    subscribers: Arc<Mutex<HashMap<ChannelType, Vec<mpsc::Sender<ChannelData>>>>>,
}

impl SubscriptionManager {
    /// Create a new subscription manager
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a subscriber for a channel type and return the receiver
    pub async fn register(&self, channel_type: ChannelType) -> mpsc::Receiver<ChannelData> {
        let (tx, rx) = mpsc::channel(1024);
        let mut subscribers = self.subscribers.lock().await;
        subscribers
            .entry(channel_type)
            .or_insert_with(Vec::new)
            .push(tx);
        rx
    }

    /// Dispatch data to all subscribers of a channel type
    pub async fn dispatch(&self, channel_type: &ChannelType, data: ChannelData) {
        let subscribers = self.subscribers.lock().await;
        if let Some(txs) = subscribers.get(channel_type) {
            for tx in txs {
                // Fire-and-forget: if subscriber is slow, skip
                let _ = tx.send(data.clone()).await;
            }
        }
    }
}

impl Default for SubscriptionManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_returns_receiver() {
        let manager = SubscriptionManager::new();
        let channel_type = ChannelType::PriceIndex("btc_usd".to_string());

        let _rx = manager.register(channel_type.clone()).await;

        // Verify we can send and receive
        let subscribers = manager.subscribers.lock().await;
        assert!(subscribers.contains_key(&channel_type));
        assert_eq!(subscribers.get(&channel_type).unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_dispatch_sends_to_subscribers() {
        let manager = SubscriptionManager::new();
        let channel_type = ChannelType::PriceIndex("btc_usd".to_string());

        let mut rx = manager.register(channel_type.clone()).await;

        let test_data = ChannelData::PriceIndex(crate::PriceIndex {
            index_name: "btc_usd".to_string(),
            price: 50000.0,
            timestamp: 1234567890,
        });

        manager.dispatch(&channel_type, test_data.clone()).await;

        let received = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            rx.recv()
        ).await.expect("timeout").expect("channel closed");

        assert_eq!(received.channel_name(), "deribit_price_index");
    }

    #[tokio::test]
    async fn test_multiple_subscribers_same_channel() {
        let manager = SubscriptionManager::new();
        let channel_type = ChannelType::PriceIndex("btc_usd".to_string());

        let mut rx1 = manager.register(channel_type.clone()).await;
        let mut rx2 = manager.register(channel_type.clone()).await;

        let test_data = ChannelData::PriceIndex(crate::PriceIndex {
            index_name: "btc_usd".to_string(),
            price: 50000.0,
            timestamp: 1234567890,
        });

        manager.dispatch(&channel_type, test_data.clone()).await;

        // Both subscribers should receive the data
        let r1 = rx1.recv().await;
        let r2 = rx2.recv().await;

        assert!(r1.is_some());
        assert!(r2.is_some());
    }
}
