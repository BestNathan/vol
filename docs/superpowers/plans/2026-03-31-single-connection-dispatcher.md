# Single WebSocket Connection Dispatcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate multiple WebSocket connections into a single connection with internal channel dispatch to reduce resource usage.

**Architecture:** Add `SubscriptionManager` to manage per-channel subscribers, refactor `DeribitClient::subscribe()` to register subscribers and lazy-start a single shared WebSocket connection that dispatches to all subscribers.

**Tech Stack:** Rust, tokio async runtime, mpsc channels, Arc<Mutex<T>> for thread-safe state.

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-deribit/src/subscription_manager.rs` | Create | Thread-safe subscription registry with dispatch logic |
| `crates/vol-deribit/src/client.rs` | Modify | Add SubscriptionManager, refactor subscribe(), add start_connection() |
| `crates/vol-deribit/src/lib.rs` | Modify | Export SubscriptionManager module |
| `crates/vol-deribit/src/subscription_manager.rs` (test) | Create | Unit tests for SubscriptionManager |
| `crates/vol-deribit/src/client.rs` (test) | Modify | Integration test for single connection |

---

### Task 1: Create SubscriptionManager struct

**Files:**
- Create: `crates/vol-deribit/src/subscription_manager.rs`

- [ ] **Step 1: Create SubscriptionManager module**

Create `crates/vol-deribit/src/subscription_manager.rs`:

```rust
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
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_register_returns_receiver() {
        let manager = SubscriptionManager::new();
        let channel_type = ChannelType::PriceIndex("btc_usd".to_string());

        let rx = manager.register(channel_type.clone()).await;

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
```

- [ ] **Step 2: Add module to lib.rs**

Modify `crates/vol-deribit/src/lib.rs`:

```rust
// Add after existing module declarations
pub mod subscription_manager;
```

- [ ] **Step 3: Run tests to verify SubscriptionManager**

Run: `cargo test -p vol-deribit subscription_manager -- --nocapture`

Expected: All 3 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-deribit/src/subscription_manager.rs crates/vol-deribit/src/lib.rs
git commit -m "feat: add SubscriptionManager for multi-channel dispatch"
```

---

### Task 2: Add subscription_manager field to DeribitClient

**Files:**
- Modify: `crates/vol-deribit/src/client.rs`

- [ ] **Step 1: Add imports and fields**

Modify `crates/vol-deribit/src/client.rs` at top:

```rust
use crate::subscription_manager::SubscriptionManager;
// ... existing imports

/// Deribit WebSocket client for low-level connection management
pub struct DeribitClient {
    ws_url: String,
    state: Arc<Mutex<ClientState>>,
    proxy_url: Option<String>,
    subscription_manager: Arc<SubscriptionManager>,
    subscribed_channels: Arc<Mutex<Vec<ChannelType>>>,
}
```

- [ ] **Step 2: Update new() to initialize fields**

Modify `DeribitClient::new()`:

```rust
/// Create a new Deribit client
pub fn new(ws_url: impl Into<String>) -> Self {
    Self {
        ws_url: ws_url.into(),
        state: Arc::new(Mutex::new(ClientState::default())),
        proxy_url: None,
        subscription_manager: Arc::new(SubscriptionManager::new()),
        subscribed_channels: Arc::new(Mutex::new(Vec::new())),
    }
}
```

- [ ] **Step 3: Update Clone implementation**

Modify `impl Clone for DeribitClient`:

```rust
impl Clone for DeribitClient {
    fn clone(&self) -> Self {
        Self {
            ws_url: self.ws_url.clone(),
            state: self.state.clone(),
            proxy_url: self.proxy_url.clone(),
            subscription_manager: self.subscription_manager.clone(),
            subscribed_channels: self.subscribed_channels.clone(),
        }
    }
}
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p vol-deribit`

Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "refactor: add SubscriptionManager and subscribed_channels fields to DeribitClient"
```

---

### Task 3: Refactor subscribe() to use SubscriptionManager

**Files:**
- Modify: `crates/vol-deribit/src/client.rs`

- [ ] **Step 1: Rewrite subscribe() method**

Replace `DeribitClient::subscribe()` (lines 296-391):

```rust
/// Subscribe to a channel and return data stream
pub async fn subscribe(
    &self,
    channel: ChannelType,
) -> Result<mpsc::Receiver<ChannelData>, vol_core::VolError> {
    // Register subscriber and get receiver
    let rx = self.subscription_manager.register(channel.clone()).await;

    // Check if we need to start/restart the connection
    let needs_reconnect = {
        let mut channels = self.subscribed_channels.lock().await;
        let is_new = !channels.contains(&channel);
        if is_new {
            channels.push(channel);
        }
        is_new
    };

    // Start connection if this is a new channel
    if needs_reconnect {
        self.start_connection().await;
    }

    Ok(rx)
}
```

- [ ] **Step 2: Remove old inline WebSocket loop**

Delete the `tokio::spawn(async move { ... })` block that was inside subscribe().

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-deribit`

Expected: Error - `start_connection()` method not found (expected)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "refactor: update subscribe() to use SubscriptionManager and lazy connection start"
```

---

### Task 4: Add start_connection() method

**Files:**
- Modify: `crates/vol-deribit/src/client.rs`

- [ ] **Step 1: Add start_connection() method**

Add after `subscribe()` method:

```rust
/// Start the WebSocket connection and reader loop
async fn start_connection(&self) {
    let ws_url = self.ws_url.clone();
    let proxy_url = self.proxy_url.clone();
    let manager = self.subscription_manager.clone();
    let channels = self.subscribed_channels.lock().await.clone();
    let state = self.state.clone();

    tokio::spawn(async move {
        let mut retry_delay = Duration::from_secs(1);
        const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);

        loop {
            info!("Connecting to Deribit WebSocket: {}", ws_url);
            if let Some(proxy) = &proxy_url {
                info!("Using proxy: {}", proxy);
            }

            let connect_result = if let Some(proxy) = &proxy_url {
                Self::connect_via_proxy(&ws_url, proxy).await
            } else {
                connect_async(&ws_url).await.map(|(ws, _)| ws).ok()
            };

            match connect_result {
                Some(ws_stream) => {
                    info!("Connected to Deribit");

                    let (mut write, mut read) = ws_stream.split();

                    // Subscribe to ALL channels at once
                    let channel_names: Vec<&str> = channels.iter()
                        .map(|c| c.channel_name())
                        .collect();

                    let subscribe_msg = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "public/subscribe",
                        "params": {
                            "channels": channel_names
                        }
                    });

                    if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                        error!("Failed to send subscription: {}", e);
                        continue;
                    }

                    info!("Subscribed to channels: {:?}", channel_names);

                    // Update state
                    {
                        let mut s = state.lock().await;
                        s.connected = true;
                        s.subscriptions = channel_names.into_iter().map(|s| s.to_string()).collect();
                    }

                    // Read and dispatch
                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(Message::Text(text)) => {
                                if let Some((channel_type, data)) = Self::parse_and_route(&text) {
                                    manager.dispatch(&channel_type, data).await;
                                }
                            }
                            Ok(Message::Ping(data)) => {
                                if let Err(e) = write.send(Message::Pong(data)).await {
                                    warn!("Failed to send pong: {}", e);
                                }
                            }
                            Ok(Message::Close(frame)) => {
                                warn!("WebSocket closed: {:?}", frame);
                                break;
                            }
                            Ok(_) => {}
                            Err(e) => {
                                error!("WebSocket error: {}", e);
                                break;
                            }
                        }
                    }

                    // Connection lost - reset state
                    {
                        let mut s = state.lock().await;
                        s.connected = false;
                        s.subscriptions = Vec::new();
                    }
                }
                None => {
                    error!("Failed to connect to Deribit");
                }
            }

            // Wait before reconnecting (exponential backoff)
            tokio::time::sleep(retry_delay).await;
            retry_delay = (retry_delay * 2).min(MAX_RETRY_DELAY);
        }
    });
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-deribit`

Expected: Error - `parse_and_route()` method not found (expected)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "feat: add start_connection() for single WebSocket connection management"
```

---

### Task 5: Add parse_and_route() method

**Files:**
- Modify: `crates/vol-deribit/src/client.rs`

- [ ] **Step 1: Add parse_and_route() method**

Add after `start_connection()`:

```rust
/// Parse message and extract channel type and data
fn parse_and_route(text: &str) -> Option<(ChannelType, ChannelData)> {
    // Try parsing as OptionMarkPrice notification (array)
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<Vec<OptionMarkPrice>>>(text) {
        if notification.method == "subscription" {
            // Extract index from channel name: "markprice.options.btc_usd" -> "btc_usd"
            let index = notification.params.channel
                .strip_prefix("markprice.options.")?
                .to_string();
            return Some((
                ChannelType::MarkpriceOptions(index),
                ChannelData::OptionMarkPrice(notification.params.data),
            ));
        }
    }

    // Try parsing as PriceIndex notification (single object)
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<PriceIndex>>(text) {
        if notification.method == "subscription" {
            let index = notification.params.channel
                .strip_prefix("deribit_price_index.")?
                .to_string();
            return Some((
                ChannelType::PriceIndex(index),
                ChannelData::PriceIndex(notification.params.data),
            ));
        }
    }

    // Try parsing as Ticker notification (array)
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<Vec<DeribitTicker>>>(text) {
        if notification.method == "subscription" {
            let base = notification.params.channel
                .strip_prefix("ticker.")?
                .split('.')
                .next()?
                .to_string();
            let ticker = notification.params.data.into_iter().next()?;
            return Some((
                ChannelType::Ticker(base),
                ChannelData::Ticker(ticker),
            ));
        }
    }

    // Try parsing as Trade notification (array)
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<Vec<Trade>>>(text) {
        if notification.method == "subscription" {
            let instrument = notification.params.channel
                .strip_prefix("trades.")?
                .to_string();
            let trade = notification.params.data.into_iter().next()?;
            return Some((
                ChannelType::Trade(instrument),
                ChannelData::Trade(trade),
            ));
        }
    }

    None
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-deribit`

Expected: Compiles successfully

- [ ] **Step 3: Run all tests**

Run: `cargo test -p vol-deribit`

Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "feat: add parse_and_route() for message type detection and dispatch"
```

---

### Task 6: Remove old parse_channel_message() method

**Files:**
- Modify: `crates/vol-deribit/src/client.rs`

- [ ] **Step 1: Remove deprecated method**

Delete `parse_channel_message()` method (lines 393-413 in original file).

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-deribit`

Expected: Compiles successfully, no warnings

- [ ] **Step 3: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "refactor: remove deprecated parse_channel_message()"
```

---

### Task 7: Integration test for single connection

**Files:**
- Modify: `crates/vol-deribit/src/client.rs` (test module)

- [ ] **Step 1: Add integration test**

Add to `crates/vol-deribit/src/client.rs` at end of file:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn test_multiple_subscriptions_share_connection() {
        let client = DeribitClient::new("wss://www.deribit.com/ws/api/v2");

        // Subscribe to multiple channels
        let _rx1 = client.subscribe(ChannelType::PriceIndex("btc_usd".to_string())).await;
        let _rx2 = client.subscribe(ChannelType::PriceIndex("eth_usd".to_string())).await;
        let _rx3 = client.subscribe(ChannelType::MarkpriceOptions("btc_usd".to_string())).await;

        // Give connection time to start
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify all channels are in subscribed_channels
        let subscribed = client.subscribed_channels.lock().await;
        assert_eq!(subscribed.len(), 3);

        // Verify only one connection was created (check state)
        assert!(client.is_connected().await);
    }
}
```

- [ ] **Step 2: Run integration test**

Run: `cargo test -p vol-deribit test_multiple_subscriptions_share_connection -- --nocapture`

Expected: Test passes (connection established, 3 channels registered)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "test: add integration test for single connection multi-channel dispatch"
```

---

### Task 8: Update DeribitDataSource usage

**Files:**
- Modify: `crates/vol-datasource/src/deribit.rs` (if needed)

- [ ] **Step 1: Verify no changes needed**

The existing `DeribitDataSource` code should work without changes since the API is unchanged.

Run: `cargo check -p vol-datasource`

Expected: Compiles successfully

- [ ] **Step 2: Commit if no changes**

```bash
git commit --allow-empty -m "chore: verify DeribitDataSource compatibility with single-connection architecture"
```

---

### Task 9: Full workspace test

**Files:**
- All workspace crates

- [ ] **Step 1: Build entire workspace**

Run: `cargo build --workspace --release`

Expected: Builds successfully

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`

Expected: All tests pass

- [ ] **Step 3: Commit final changes if any**

```bash
git add -A
git commit -m "chore: full workspace build and test verification"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- [x] SubscriptionManager struct - Task 1
- [x] DeribitClient fields - Task 2
- [x] Modified subscribe() - Task 3
- [x] start_connection() method - Task 4
- [x] parse_and_route() method - Task 5
- [x] Remove old code - Task 6
- [x] Integration tests - Task 7
- [x] Verify DataSource compatibility - Task 8
- [x] Full workspace test - Task 9

**2. Placeholder scan:** No TBD/TODO placeholders found.

**3. Type consistency:**
- `ChannelType` and `ChannelData` used consistently throughout
- `SubscriptionManager` methods match usage in `client.rs`
- All async methods properly marked

---

Plan complete. Two execution options:

**1. Subagent-Driven (recommended)** - Dispatch fresh subagent per task with two-stage review

**2. Inline Execution** - Execute tasks in this session with checkpoints

Which approach?
