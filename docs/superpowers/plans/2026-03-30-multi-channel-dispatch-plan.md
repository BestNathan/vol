# Multi-Channel Dispatch and Index Price State Management Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现可扩展的多频道分发架构，支持从 `deribit_price_index.<INDEX>` 频道获取真实指数价格。

**Architecture:**
- `DeribitClient` (协议层): 使用 `ChannelType` 和 `ChannelData` 枚举实现类型安全的频道订阅
- `DeribitDataSource` (数据整合层): 维护 `IndexPriceState` 状态，通过 `tokio::select!` 合并多个频道数据流

**Tech Stack:** Rust, tokio, serde_json, channel-based message dispatch

---

## File Structure

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/vol-deribit/src/subscription.rs` | Modify | 添加 `deribit_price_index()` 频道构建函数 |
| `crates/vol-deribit/src/market_data.rs` | Modify | 添加 `PriceIndex` 结构体 |
| `crates/vol-deribit/src/message.rs` | Create | 添加 `ChannelType` 和 `ChannelData` 枚举 |
| `crates/vol-deribit/src/client.rs` | Modify | 改造 `subscribe` 方法支持频道类型分发 |
| `crates/vol-deribit/src/market_data.rs` | Modify | 添加 `to_volatility_data_with_index` 方法 |
| `crates/vol-datasource/src/deribit.rs` | Modify | 添加 `IndexPriceState`，改造数据合并逻辑 |

---

## Task 1: Add deribit_price_index subscription function

**Files:**
- Modify: `crates/vol-deribit/src/subscription.rs`

- [ ] **Step 1: 添加 deribit_price_index 频道构建函数**

在 `subscription.rs` 的 `book_depth` 函数之后添加：

```rust
/// Build deribit price index channel
/// e.g., "deribit_price_index.btc_usd"
pub fn deribit_price_index(index: &str) -> String {
    format!("deribit_price_index.{}", index)
}
```

- [ ] **Step 2: 添加 preset 函数**

在 `presets` 模块中添加：

```rust
/// Subscribe to deribit price index for BTC and ETH
/// Returns: ["deribit_price_index.btc_usd", "deribit_price_index.eth_usd"]
pub fn price_indices(bases: Vec<&str>) -> Vec<String> {
    bases.iter().map(|&b| deribit_price_index(&format!("{}_usd", b.to_lowercase()))).collect()
}
```

- [ ] **Step 3: 添加测试**

在 `tests` 模块中添加测试：

```rust
#[test]
fn test_deribit_price_index_builder() {
    assert_eq!(deribit_price_index("btc_usd"), "deribit_price_index.btc_usd");
    assert_eq!(deribit_price_index("eth_usd"), "deribit_price_index.eth_usd");
}

#[test]
fn test_price_indices_preset() {
    let indices = presets::price_indices(vec!["BTC", "ETH"]);
    assert_eq!(indices.len(), 2);
    assert!(indices.contains(&"deribit_price_index.btc_usd".to_string()));
    assert!(indices.contains(&"deribit_price_index.eth_usd".to_string()));
}
```

- [ ] **Step 4: 运行测试**

```bash
cargo test -p vol-deribit -- --nocapture
```

Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-deribit/src/subscription.rs
git commit -m "feat: add deribit_price_index channel builder"
```

---

## Task 2: Add PriceIndex struct

**Files:**
- Modify: `crates/vol-deribit/src/market_data.rs`

- [ ] **Step 1: 添加 PriceIndex 结构体**

在 `IndexMarkPrice` 结构体之后添加：

```rust
/// Deribit Price Index data
///
/// Received from `deribit_price_index.<INDEX>` channels
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceIndex {
    /// Index name (e.g., "btc_usd")
    pub index_name: String,
    /// Current index price
    pub price: f64,
    /// Timestamp (milliseconds since Unix epoch)
    pub timestamp: u64,
}
```

- [ ] **Step 2: 添加 PriceIndex 到 module exports**

确保 `lib.rs` 或 `mod.rs` 中导出 `PriceIndex`：

```rust
pub use market_data::{
    DeribitTicker, OptionMarkPrice, IndexMarkPrice, PriceIndex, OrderBook, Trade, PriceLevel,
};
```

- [ ] **Step 3: 编译检查**

```bash
cargo check -p vol-deribit
```

Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-deribit/src/market_data.rs
git commit -m "feat: add PriceIndex struct for deribit_price_index channel"
```

---

## Task 3: Add ChannelType and ChannelData enums

**Files:**
- Create: `crates/vol-deribit/src/message.rs`

- [ ] **Step 1: 创建 message.rs 文件**

```rust
//! Deribit message types for channel dispatch.
//!
//! This module provides type-safe channel dispatch using enums.

use serde::{Deserialize, Serialize};

use crate::{OptionMarkPrice, PriceIndex, DeribitTicker, Trade};

/// 频道类型枚举 - 编译期绑定频道与数据类型
#[derive(Debug, Clone)]
pub enum ChannelType {
    /// Options mark price with IV (markprice.options.<INDEX>)
    MarkpriceOptions(String),
    /// Index price (deribit_price_index.<INDEX>)
    PriceIndex(String),
    /// Ticker data (ticker.<BASE>)
    Ticker(String),
    /// Trade executions (trades.<INSTRUMENT>)
    Trade(String),
}

impl ChannelType {
    /// Get the channel name string for subscription
    pub fn channel_name(&self) -> String {
        match self {
            ChannelType::MarkpriceOptions(idx) => crate::subscription::markprice_options(idx),
            ChannelType::PriceIndex(idx) => crate::subscription::deribit_price_index(idx),
            ChannelType::Ticker(base) => crate::subscription::ticker_base(base),
            ChannelType::Trade(instrument) => crate::subscription::trades(instrument),
        }
    }
}

/// 统一的频道数据枚举
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ChannelData {
    OptionMarkPrice(Vec<OptionMarkPrice>),
    PriceIndex(PriceIndex),
    Ticker(DeribitTicker),
    Trade(Trade),
}

impl ChannelData {
    /// Get channel name for logging
    pub fn channel_name(&self) -> &'static str {
        match self {
            ChannelData::OptionMarkPrice(_) => "markprice.options",
            ChannelData::PriceIndex(_) => "deribit_price_index",
            ChannelData::Ticker(_) => "ticker",
            ChannelData::Trade(_) => "trades",
        }
    }
}
```

- [ ] **Step 2: 导出 message 模块**

在 `crates/vol-deribit/src/lib.rs` 中添加：

```rust
pub mod message;
pub use message::{ChannelType, ChannelData};
```

- [ ] **Step 3: 添加单元测试**

在 `message.rs` 末尾添加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_type_channel_name() {
        assert_eq!(
            ChannelType::MarkpriceOptions("btc_usd".to_string()).channel_name(),
            "markprice.options.btc_usd"
        );
        assert_eq!(
            ChannelType::PriceIndex("btc_usd".to_string()).channel_name(),
            "deribit_price_index.btc_usd"
        );
    }

    #[test]
    fn test_channel_data_channel_name() {
        // This test just verifies the enum variants compile
        let _ = ChannelData::PriceIndex(PriceIndex {
            index_name: "btc_usd".to_string(),
            price: 50000.0,
            timestamp: 1234567890,
        });
    }
}
```

- [ ] **Step 4: 运行测试**

```bash
cargo test -p vol-deribit -- --nocapture
```

Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-deribit/src/message.rs crates/vol-deribit/src/lib.rs
git commit -m "feat: add ChannelType and ChannelData enums for type-safe dispatch"
```

---

## Task 4: Refactor DeribitClient subscribe method

**Files:**
- Modify: `crates/vol-deribit/src/client.rs`

- [ ] **Step 1: 更新导入**

替换文件顶部的导入：

```rust
use crate::{ChannelType, ChannelData, OptionMarkPrice, PriceIndex, DeribitTicker, Trade, SubscriptionNotification};
use vol_core::VolatilityData;
```

- [ ] **Step 2: 修改 subscribe 方法签名**

替换 `spawn_ws_loop` 相关的代码，添加新的 subscribe 方法：

```rust
/// Subscribe to a channel and return data stream
pub async fn subscribe(
    &self,
    channel: ChannelType,
) -> Result<mpsc::Receiver<ChannelData>, vol_core::VolError> {
    let (tx, rx) = mpsc::channel(1024);

    // Get channel name string for subscription
    let channel_name = channel.channel_name();

    // Store channel type for runtime dispatch
    let channel_type = channel.clone();

    // Spawn WebSocket message processing loop
    let ws_url = self.ws_url.clone();
    let proxy_url = self.proxy_url.clone();

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

                    // Send subscription message
                    let subscribe_msg = serde_json::json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "public/subscribe",
                        "params": {
                            "channels": [&channel_name]
                        }
                    });

                    if let Err(e) = write.send(Message::Text(subscribe_msg.to_string())).await {
                        error!("Failed to send subscription: {}", e);
                        continue;
                    }

                    info!("Subscribed to channel: {}", channel_name);

                    // Read messages
                    while let Some(msg_result) = read.next().await {
                        match msg_result {
                            Ok(Message::Text(text)) => {
                                if let Some(data) = Self::parse_channel_message(&text, &channel_type) {
                                    if let Err(e) = tx.send(data).await {
                                        warn!("Failed to send channel data: {}", e);
                                        break;
                                    }
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

    Ok(rx)
}
```

- [ ] **Step 3: 添加 parse_channel_message 方法**

在 `parse_message` 方法之后添加：

```rust
/// Parse channel message based on channel type
fn parse_channel_message(text: &str, channel_type: &ChannelType) -> Option<ChannelData> {
    match channel_type {
        ChannelType::MarkpriceOptions(_) => {
            let notification: SubscriptionNotification<OptionMarkPrice> = serde_json::from_str(text).ok()?;
            Some(ChannelData::OptionMarkPrice(notification.params.data))
        }
        ChannelType::PriceIndex(_) => {
            let notification: SubscriptionNotification<PriceIndex> = serde_json::from_str(text).ok()?;
            Some(ChannelData::PriceIndex(notification.params.data.into_iter().next()?))
        }
        ChannelType::Ticker(_) => {
            let notification: SubscriptionNotification<DeribitTicker> = serde_json::from_str(text).ok()?;
            Some(ChannelData::Ticker(notification.params.data.into_iter().next()?))
        }
        ChannelType::Trade(_) => {
            let notification: SubscriptionNotification<Trade> = serde_json::from_str(text).ok()?;
            Some(ChannelData::Trade(notification.params.data.into_iter().next()?))
        }
    }
}
```

- [ ] **Step 4: 保留或移除旧的 run 方法**

如果 `run` 方法不再被外部使用，可以标记为 deprecated：

```rust
#[deprecated(note = "Use subscribe(ChannelType) instead")]
pub async fn run(&self, channels: Vec<String>, tx: mpsc::Sender<VolatilityData>) { ... }
```

- [ ] **Step 5: 编译检查**

```bash
cargo check -p vol-deribit
```

Expected: PASS (可能有 deprecation warning)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-deribit/src/client.rs
git commit -m "refactor: update DeribitClient to use ChannelType-based subscribe"
```

---

## Task 5: Add to_volatility_data_with_index method

**Files:**
- Modify: `crates/vol-deribit/src/market_data.rs`

- [ ] **Step 1: 添加 to_volatility_data_with_index 方法**

在 `OptionMarkPrice` impl 块中添加：

```rust
impl OptionMarkPrice {
    /// Convert to VolatilityData with externally provided index price
    pub fn to_volatility_data_with_index(
        &self,
        index_price: Option<f64>,
    ) -> Option<vol_core::VolatilityData> {
        let iv = self.iv?;

        // Parse instrument name: "BTC-29MAR24-70000-C"
        let (underlying, _year, month, day, strike, option_type) =
            crate::instrument::parse_instrument_name(&self.instrument_name)?;

        // Use provided index price, fallback to strike if not available
        let index_price = index_price.unwrap_or(strike);

        // Calculate DTE from expiry
        let expiry_str = format!("{:02}{}{:02}", day,
            match month {
                1 => "JAN", 2 => "FEB", 3 => "MAR", 4 => "APR",
                5 => "MAY", 6 => "JUN", 7 => "JUL", 8 => "AUG",
                9 => "SEP", 10 => "OCT", 11 => "NOV", 12 => "DEC",
                _ => return None,
            },
            _year % 100
        );
        let dte = crate::instrument::calculate_dte(&expiry_str)?;

        let mut extra = std::collections::HashMap::new();
        extra.insert("underlying".to_string(), serde_json::json!(underlying));
        extra.insert("mark_price".to_string(), serde_json::json!(self.mark_price));
        extra.insert("index_price".to_string(), serde_json::json!(index_price));

        Some(vol_core::VolatilityData {
            symbol: self.instrument_name.clone(),
            dte: dte as u32,
            iv,
            timestamp: self.timestamp,
            source: "deribit".to_string(),
            strike,
            option_type: match option_type {
                crate::instrument::OptionType::Call => vol_core::OptionType::Call,
                crate::instrument::OptionType::Put => vol_core::OptionType::Put,
            },
            index_price,
            delta: None,
            extra,
        })
    }

    /// Convert to VolatilityData (deprecated - use with_index version)
    #[deprecated(note = "Use to_volatility_data_with_index instead")]
    pub fn to_volatility_data(&self) -> Option<vol_core::VolatilityData> {
        self.to_volatility_data_with_index(None)
    }
}
```

- [ ] **Step 2: 添加单元测试**

在 `market_data.rs` 末尾添加测试模块：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_volatility_data_with_index() {
        let option = OptionMarkPrice {
            instrument_name: "BTC-29MAR24-70000-C".to_string(),
            mark_price: 2000.0,
            iv: Some(0.80),
            timestamp: 1234567890,
            index_price: None,
        };

        let result = option.to_volatility_data_with_index(Some(70000.0));
        assert!(result.is_some());
        let vol_data = result.unwrap();
        assert_eq!(vol_data.index_price, 70000.0);
    }

    #[test]
    fn test_to_volatility_data_with_index_fallback() {
        let option = OptionMarkPrice {
            instrument_name: "BTC-29MAR24-70000-C".to_string(),
            mark_price: 2000.0,
            iv: Some(0.80),
            timestamp: 1234567890,
            index_price: None,
        };

        // Without index price, should fall back to strike
        let result = option.to_volatility_data_with_index(None);
        assert!(result.is_some());
        let vol_data = result.unwrap();
        assert_eq!(vol_data.index_price, 70000.0); // strike price
    }
}
```

- [ ] **Step 3: 运行测试**

```bash
cargo test -p vol-deribit -- --nocapture
```

Expected: All tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-deribit/src/market_data.rs
git commit -m "feat: add to_volatility_data_with_index method"
```

---

## Task 6: Add IndexPriceState

**Files:**
- Modify: `crates/vol-datasource/src/deribit.rs`

- [ ] **Step 1: 更新导入**

在文件顶部添加：

```rust
use std::collections::HashMap;
use vol_deribit::{ChannelType, ChannelData, PriceIndex};
```

- [ ] **Step 2: 添加 IndexPriceState 结构**

在 `DeribitState` 之前添加：

```rust
/// Index price state - thread-safe shared state
#[derive(Debug, Clone, Default)]
pub struct IndexPriceState {
    prices: Arc<Mutex<HashMap<String, f64>>>,
}

impl IndexPriceState {
    pub fn new() -> Self {
        Self {
            prices: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn update(&self, index: &str, price: f64) {
        let mut prices = self.prices.lock().await;
        prices.insert(index.to_lowercase(), price);
    }

    pub async fn get(&self, index: &str) -> Option<f64> {
        let prices = self.prices.lock().await;
        prices.get(&index.to_lowercase()).copied()
    }
}
```

- [ ] **Step 3: 更新 DeribitDataSource 结构**

替换现有的 `DeribitDataSource` 定义：

```rust
/// Deribit WebSocket data source
pub struct DeribitDataSource {
    client: DeribitClient,
    index_price_state: IndexPriceState,
    symbols: Vec<String>,
}
```

- [ ] **Step 4: 更新 new 方法**

替换 `new` 方法：

```rust
impl DeribitDataSource {
    pub fn new(ws_url: String, symbols: Vec<String>, _poll_interval_secs: u64) -> Self {
        let client = DeribitClient::new(ws_url);
        Self {
            client,
            index_price_state: IndexPriceState::new(),
            symbols,
        }
    }
```

- [ ] **Step 5: 编译检查**

```bash
cargo check -p vol-datasource
```

Expected: PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vol-datasource/src/deribit.rs
git commit -m "feat: add IndexPriceState for thread-safe index price management"
```

---

## Task 7: Implement data merger in DeribitDataSource

**Files:**
- Modify: `crates/vol-datasource/src/deribit.rs`

- [ ] **Step 1: 添加 spawn_data_merger 方法**

在 `DeribitDataSource` impl 块中添加：

```rust
impl DeribitDataSource {
    // ... existing new() and with_proxy() ...

    /// Spawn data merger to combine multiple channel streams
    fn spawn_data_merger(
        mut options_rx: mpsc::Receiver<ChannelData>,
        mut index_rx: mpsc::Receiver<ChannelData>,
        tx: mpsc::Sender<vol_core::VolatilityData>,
        index_state: IndexPriceState,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // Handle index price updates
                    Some(ChannelData::PriceIndex(price_data)) = index_rx.recv() => {
                        index_state.update(&price_data.index_name, price_data.price).await;
                    }

                    // Handle options mark prices
                    Some(ChannelData::OptionMarkPrice(options_list)) = options_rx.recv() => {
                        for option in options_list {
                            // Extract underlying from instrument name
                            let underlying = option.instrument_name.split('-').next().unwrap_or("BTC");
                            let index_key = format!("{}_usd", underlying.to_lowercase());

                            // Get index price from state
                            let index_price = index_state.get(&index_key).await;

                            // Construct VolatilityData
                            if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
                                let _ = tx.send(vol_data).await;
                            }
                        }
                    }

                    // Handle channel close
                    else => {
                        break;
                    }
                }
            }
        });
    }
```

- [ ] **Step 2: 更新 subscribe 方法**

替换 `subscribe` 方法：

```rust
fn subscribe(&self, symbols: Vec<String>) -> Result<mpsc::Receiver<vol_core::VolatilityData>> {
    let (tx, rx) = mpsc::channel(1024);

    // Create channel subscriptions for each symbol
    let mut options_channels = Vec::new();
    let mut index_channels = Vec::new();

    for symbol in &symbols {
        options_channels.push(ChannelType::MarkpriceOptions(format!("{}_usd", symbol.to_lowercase())));
        index_channels.push(ChannelType::PriceIndex(format!("{}_usd", symbol.to_lowercase())));
    }

    // Clone state and client for the merger
    let index_state = self.index_price_state.clone();
    let client_options = self.client.clone();
    let client_index = self.client.clone();

    // Subscribe to channels
    let options_rx = tokio::spawn(async move {
        let mut all_rx: Vec<mpsc::Receiver<ChannelData>> = Vec::new();
        for channel in options_channels {
            if let Ok(rx) = client_options.subscribe(channel).await {
                all_rx.push(rx);
            }
        }
        // Merge all option channels into one
        merge_receivers(all_rx).await
    });

    let index_rx = tokio::spawn(async move {
        let mut all_rx: Vec<mpsc::Receiver<ChannelData>> = Vec::new();
        for channel in index_channels {
            if let Ok(rx) = client_index.subscribe(channel).await {
                all_rx.push(rx);
            }
        }
        // Merge all index channels into one
        merge_receivers(all_rx).await
    });

    // This is simplified - actual implementation needs proper channel merging
    // For now, we'll use a simpler approach with single channels per type

    Ok(rx)
}
```

**注意：** 上面的实现需要简化。让我提供一个更实用的版本：

- [ ] **Step 2 (revised): 简化版 subscribe 实现**

替换为更简单的实现：

```rust
fn subscribe(&self, symbols: Vec<String>) -> Result<mpsc::Receiver<vol_core::VolatilityData>> {
    let (tx, rx) = mpsc::channel(1024);

    // Subscribe to options mark prices for all symbols
    let options_channels: Vec<ChannelType> = symbols
        .iter()
        .map(|s| ChannelType::MarkpriceOptions(format!("{}_usd", s.to_lowercase())))
        .collect();

    // Subscribe to price indices for all symbols
    let index_channels: Vec<ChannelType> = symbols
        .iter()
        .map(|s| ChannelType::PriceIndex(format!("{}_usd", s.to_lowercase())))
        .collect();

    // For simplicity, we'll create separate subscription tasks
    // In production, you'd want to use futures::stream::select_all

    let index_state = self.index_price_state.clone();
    let client_clone = self.client.clone();
    let tx_clone = tx.clone();

    // Spawn index price subscriber
    tokio::spawn(async move {
        for channel in index_channels {
            let channel_name = channel.channel_name();
            if let Ok(mut rx) = client_clone.subscribe(channel).await {
                while let Some(data) = rx.recv().await {
                    if let ChannelData::PriceIndex(price_data) = data {
                        index_state.update(&price_data.index_name, price_data.price).await;
                        info!("Updated index price: {} = {}", price_data.index_name, price_data.price);
                    }
                }
            }
        }
    });

    // Spawn options subscriber
    tokio::spawn(async move {
        for channel in options_channels {
            let channel_name = channel.channel_name();
            if let Ok(mut rx) = client_clone.subscribe(channel).await {
                while let Some(data) = rx.recv().await {
                    if let ChannelData::OptionMarkPrice(options_list) = data {
                        for option in options_list {
                            let underlying = option.instrument_name.split('-').next().unwrap_or("BTC");
                            let index_key = format!("{}_usd", underlying.to_lowercase());
                            let index_price = index_state.get(&index_key).await;

                            if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
                                let _ = tx_clone.send(vol_data).await;
                            }
                        }
                    }
                }
            }
        }
    });

    Ok(rx)
}
```

- [ ] **Step 3: 添加 merge_receivers 辅助函数（可选）**

```rust
/// Helper to merge multiple receivers into one
async fn merge_receivers<T: Send + 'static>(
    mut receivers: Vec<mpsc::Receiver<T>>,
) -> mpsc::Receiver<T> {
    let (tx, rx) = mpsc::channel(1024);

    for mut rx_inner in receivers {
        let tx_clone = tx.clone();
        tokio::spawn(async move {
            while let Some(msg) = rx_inner.recv().await {
                if tx_clone.send(msg).await.is_err() {
                    break;
                }
            }
        });
    }

    rx
}
```

- [ ] **Step 4: 编译检查**

```bash
cargo check -p vol-datasource
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-datasource/src/deribit.rs
git commit -m "feat: implement data merger for multi-channel subscription"
```

---

## Task 8: Integration test and verification

**Files:**
- N/A (runtime verification)

- [ ] **Step 1: 完整构建**

```bash
cargo build --release
```

Expected: PASS

- [ ] **Step 2: 运行 vol-monitor 验证**

```bash
RUST_LOG=info HTTPS_PROXY=<proxy> ./target/release/vol-monitor 2>&1 | head -50
```

Expected: See "Updated index price" log messages with real prices

- [ ] **Step 3: 验证日志输出**

观察日志中是否显示真实的指数价格，例如：
```
Updated index price: btc_usd = 95234.5
Updated index price: eth_usd = 3812.3
```

- [ ] **Step 4: 验证告警中的 index_price**

当 IV 告警触发时，检查日志中的 index_price 是否为真实值：
```
[1] BTC-6JAN25-95000-C short IV 85.2% | Index: $95234.5
```

- [ ] **Step 5: Commit (如有代码修改)**

```bash
git add .
git commit -m "chore: verify multi-channel dispatch in production"
```

---

## Self-Review Checklist

- [ ] **Spec coverage:** 所有设计文档中的功能都有对应的 task
- [ ] **Placeholder scan:** 无 TBD/TODO
- [ ] **Type consistency:** `ChannelType`, `ChannelData`, `PriceIndex` 命名一致
- [ ] **Code completeness:** 所有步骤都有完整代码

---

## Execution Options

Plan saved to `docs/superpowers/plans/2026-03-30-multi-channel-dispatch-plan.md`.

**Two execution options:**

1. **Subagent-Driven (recommended)** - Dispatch fresh subagent per task, review between tasks
2. **Inline Execution** - Execute tasks in this session with checkpoints

Which approach?
