---
name: Single WebSocket Connection Dispatcher
description: Consolidate multiple WebSocket connections into a single connection with internal channel dispatch
type: project
---

# Single WebSocket Connection Dispatcher Design

## Problem

Current implementation creates one WebSocket connection per channel subscription. For BTC+ETH monitoring with options mark prices and index prices, this results in 4 independent connections to `wss://www.deribit.com/ws/api/v2`.

**Current Architecture:**
```
subscribe(MarkpriceOptions(btc_usd))  ──> WebSocket #1 ──> rx1
subscribe(MarkpriceOptions(eth_usd))  ──> WebSocket #2 ──> rx2
subscribe(PriceIndex(btc_usd))        ──> WebSocket #3 ──> rx3
subscribe(PriceIndex(eth_usd))        ──> WebSocket #4 ──> rx4
```

Each connection:
- Maintains independent TCP/TLS handshake
- Runs separate auto-reconnect loop
- Consumes file descriptors and memory
- Independent subscription state

## Goal

Single WebSocket connection that internally dispatches messages to multiple `mpsc::Sender<ChannelData>` channels based on subscription type.

**Target Architecture:**
```
subscribe(MarkpriceOptions(btc_usd))  ─┐
subscribe(MarkpriceOptions(eth_usd))  ─┤
subscribe(PriceIndex(btc_usd))        ─┼──> Single WebSocket Connection
subscribe(PriceIndex(eth_usd))        ─┤        │
                                        └──> Internal Dispatcher
                                                 ├──> tx1 -> rx1 (MarkpriceOptions)
                                                 ├──> tx2 -> rx2 (PriceIndex)
                                                 └──> ...
```

## Design

### Core Components

#### 1. `SubscriptionManager` - Thread-safe subscription registry

```rust
pub struct SubscriptionManager {
    subscribers: Arc<Mutex<HashMap<ChannelType, Vec<mpsc::Sender<ChannelData>>>>>,
}

impl SubscriptionManager {
    pub fn register(&self, channel_type: ChannelType) -> mpsc::Receiver<ChannelData> {
        let (tx, rx) = mpsc::channel(1024);
        // Add tx to subscribers[channel_type]
        rx
    }

    pub async fn dispatch(&self, channel_type: &ChannelType, data: ChannelData) {
        let subscribers = self.subscribers.lock().await;
        if let Some(txs) = subscribers.get(channel_type) {
            for tx in txs {
                let _ = tx.send(data.clone()).await;
            }
        }
    }
}
```

#### 2. `DeribitClient` - Single connection state

```rust
pub struct DeribitClient {
    ws_url: String,
    proxy_url: Option<String>,
    state: Arc<Mutex<ClientState>>,
    subscription_manager: Arc<SubscriptionManager>,
    /// Channels subscribed in current connection
    subscribed_channels: Arc<Mutex<Vec<ChannelType>>>,
}
```

#### 3. Modified `subscribe()` - Lazy connection start

```rust
pub async fn subscribe(
    &self,
    channel: ChannelType,
) -> Result<mpsc::Receiver<ChannelData>, vol_core::VolError> {
    // Register subscriber and get receiver
    let rx = self.subscription_manager.register(channel.clone());

    // Check if we need to start/restart the connection
    let needs_reconnect = {
        let mut channels = self.subscribed_channels.lock().await;
        if !channels.contains(&channel) {
            channels.push(channel.clone());
            true
        } else {
            false
        }
    };

    // Start connection if not running or needs to subscribe to new channel
    if needs_reconnect || !self.state.lock().await.connected {
        self.start_connection().await;
    }

    Ok(rx)
}
```

#### 4. `start_connection()` - Single WebSocket reader loop

```rust
async fn start_connection(&self) {
    let ws_url = self.ws_url.clone();
    let proxy_url = self.proxy_url.clone();
    let manager = self.subscription_manager.clone();
    let channels = self.subscribed_channels.lock().await.clone();
    let state = self.state.clone();

    tokio::spawn(async move {
        // Connect (with proxy support)
        let ws_stream = if let Some(proxy) = &proxy_url {
            Self::connect_via_proxy(&ws_url, proxy).await
        } else {
            connect_async(&ws_url).await.ok().map(|(ws, _)| ws)
        };

        if let Some(ws_stream) = ws_stream {
            let (mut write, mut read) = ws_stream.split();

            // Subscribe to ALL channels at once
            let channel_names: Vec<&str> = channels.iter()
                .map(|c| c.channel_name())
                .collect();

            let subscribe_msg = serde_json::json!({
                "jsonrpc": "2.0",
                "id": 1,
                "method": "public/subscribe",
                "params": { "channels": channel_names }
            });

            write.send(Message::Text(subscribe_msg.to_string())).await.ok();

            state.lock().await.connected = true;

            // Read and dispatch
            while let Some(Ok(Message::Text(text))) = read.next().await {
                if let Some((channel_type, data)) = Self::parse_and_route(&text) {
                    manager.dispatch(&channel_type, data).await;
                }
            }

            state.lock().await.connected = false;
        }
    });
}
```

#### 5. `parse_and_route()` - Message type detection

```rust
fn parse_and_route(text: &str) -> Option<(ChannelType, ChannelData)> {
    // Try parsing as OptionMarkPrice notification
    if let Ok(notification) = serde_json::from_str::<SubscriptionNotification<OptionMarkPrice>>(text) {
        return Some((
            ChannelType::MarkpriceOptions("unknown".to_string()), // Extract from params.channel
            ChannelData::OptionMarkPrice(notification.params.data),
        ));
    }

    // Try parsing as PriceIndex notification (single object)
    #[derive(Deserialize)]
    struct SinglePriceIndexNotification {
        params: SinglePriceIndexParams,
    }
    #[derive(Deserialize)]
    struct SinglePriceIndexParams {
        channel: String,
        data: PriceIndex,
    }

    if let Ok(notification) = serde_json::from_str::<SinglePriceIndexNotification>(text) {
        let index = notification.params.channel
            .strip_prefix("deribit_price_index.")?
            .to_string();
        return Some((
            ChannelType::PriceIndex(index),
            ChannelData::PriceIndex(notification.params.data),
        ));
    }

    None
}
```

## Data Flow

```
1. User calls subscribe(MarkpriceOptions(btc_usd))
       │
2. SubscriptionManager creates mpsc channel, stores tx
       │
3. Check if WebSocket running → No
       │
4. start_connection() spawns WebSocket reader
       │
5. Subscribe to ALL channels: ["markprice.options.btc_usd", ...]
       │
6. WebSocket receives notification
       │
7. parse_and_route() extracts (ChannelType, ChannelData)
       │
8. SubscriptionManager.dispatch() sends to all matching subscribers
       │
9. User receives data on mpsc::Receiver<ChannelData>
```

## Error Handling

### Connection Loss
- Single reconnection loop for all channels
- On reconnect, re-subscribe to all channels atomically
- Subscribers see gap in data (same as current behavior)

### Parse Failure
- Log warning, continue processing
- Other channels unaffected

### Subscriber Backpressure
- `mpsc::channel(1024)` provides buffering
- If full, `send()` returns error, log and continue
- Slow subscriber doesn't block others (fire-and-forget send)

## API Compatibility

Existing API unchanged:

```rust
// Before and After - same usage
let client = DeribitClient::new(ws_url);
let rx1 = client.subscribe(ChannelType::MarkpriceOptions("btc_usd".into())).await?;
let rx2 = client.subscribe(ChannelType::PriceIndex("btc_usd".into())).await?;
// Single WebSocket connection used for both
```

## Migration Plan

1. Add `SubscriptionManager` struct
2. Add `subscribed_channels` field to `DeribitClient`
3. Refactor `subscribe()` to register and lazy-start
4. Extract `start_connection()` from current inline loop
5. Replace per-channel subscription with batch subscription
6. Add `parse_and_route()` dispatcher
7. Remove old `subscribe()` implementation
8. Test with multi-channel subscription

## Testing

1. **Unit tests:** `SubscriptionManager` register/dispatch logic
2. **Integration tests:** Subscribe to 4 channels, verify single connection
3. **Load tests:** Monitor memory/CPU vs current implementation
4. **Reconnection tests:** Verify all channels re-subscribed after disconnect

## Trade-offs

| Aspect | Current (Per-Channel) | New (Single Connection) |
|--------|----------------------|------------------------|
| Connections | N (4 for BTC+ETH) | 1 |
| Reconnection | Per-channel | All channels together |
| Dynamic subscription | ✅ Yes | ✅ Yes |
| Code complexity | Simple | Moderate |
| Resource usage | High | Low |
| Failure isolation | Per-channel | All-or-nothing |

**Failure isolation note:** Current design allows one channel to reconnect independently. New design reconnects all together. This is acceptable because:
- Deribit connection is usually all-or-nothing (network/server issue)
- Simplifies state management significantly
- Reconnection is fast (<1s typical)
