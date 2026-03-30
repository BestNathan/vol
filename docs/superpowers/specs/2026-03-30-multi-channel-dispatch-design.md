# Multi-Channel Dispatch and Index Price State Management Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 实现可扩展的多频道分发架构，支持从 `deribit_price_index.<INDEX>` 频道获取真实指数价格，替代硬编码默认值。

**Architecture:**
- `DeribitClient` (协议层): 使用 `ChannelType` 枚举和 `ChannelData` 枚举实现类型安全的频道订阅，根据频道类型自动路由到对应的解析器
- `DeribitDataSource` (数据整合层): 维护 `IndexPriceState` 状态，通过 `tokio::select!` 合并多个频道数据流

**Tech Stack:** Rust, tokio, serde_json, channel-based message dispatch

---

## File Structure

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/vol-deribit/src/message.rs` | Modify | 添加 `ChannelType` 和 `ChannelData` 枚举 |
| `crates/vol-deribit/src/client.rs` | Modify | 改造 `subscribe` 方法支持频道类型分发 |
| `crates/vol-deribit/src/market_data.rs` | Modify | 添加 `to_volatility_data_with_index` 方法 |
| `crates/vol-datasource/src/deribit.rs` | Modify | 添加 `IndexPriceState`，改造数据合并逻辑 |

---

## Design Details

### 1. Channel Type Enum (vol-deribit/src/message.rs)

添加频道类型枚举，实现编译期类型绑定：

```rust
/// 频道类型枚举 - 编译期绑定频道与数据类型
#[derive(Debug, Clone)]
pub enum ChannelType {
    /// Options mark price with IV (markprice.options.<INDEX>)
    MarkpriceOptions(String),  // index, e.g., "btc_usd"
    /// Index price (deribit_price_index.<INDEX>)
    PriceIndex(String),        // index, e.g., "btc_usd"
    /// Ticker data (ticker.<BASE>)
    Ticker(String),            // base, e.g., "BTC"
    /// Trade executions (trades.<INSTRUMENT>)
    Trade(String),             // instrument
}

/// 统一的频道数据枚举
#[derive(Debug, Clone)]
pub enum ChannelData {
    OptionMarkPrice(Vec<OptionMarkPrice>),
    PriceIndex(PriceIndex),    // Single index price data
    Ticker(DeribitTicker),
    Trade(Trade),
}

impl ChannelData {
    /// 获取频道名称用于日志
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

**扩展性：** 新增频道类型只需添加 enum variant。

---

### 2. PriceIndex Struct (vol-deribit/src/market_data.rs)

添加 `PriceIndex` 结构用于 `deribit_price_index` 频道：

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

---

### 3. DeribitClient Subscribe Method (vol-deribit/src/client.rs)

改造 `subscribe` 方法支持频道类型分发：

```rust
impl DeribitClient {
    /// 订阅频道并返回数据流
    ///
    /// 根据频道类型自动路由到对应的解析器
    pub async fn subscribe(
        &self,
        channel: ChannelType,
    ) -> Result<mpsc::Receiver<ChannelData>, VolError> {
        let (tx, rx) = mpsc::channel(1024);

        // 获取频道名称字符串用于订阅
        let channel_name = match &channel {
            ChannelType::MarkpriceOptions(idx) => subscription::markprice_options(idx),
            ChannelType::PriceIndex(idx) => subscription::deribit_price_index(idx),
            ChannelType::Ticker(base) => subscription::ticker_base(base),
            ChannelType::Trade(instrument) => subscription::trades(instrument),
        };

        // 存储频道类型用于运行时分发
        let channel_type = channel.clone();

        // Spawn WebSocket 消息处理循环
        let ws_url = self.ws_url.clone();
        let proxy_url = self.proxy_url.clone();
        tokio::spawn(async move {
            // 连接逻辑...
            // 订阅频道...
            // 读取消息并分发
            while let Some(msg) = read.next().await {
                if let Message::Text(text) = msg.ok()? {
                    if let Some(data) = Self::parse_channel_message(&text, &channel_type) {
                        let _ = tx.send(data).await;
                    }
                }
            }
        });

        Ok(rx)
    }

    /// 频道特定的解析器（内部使用）
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
}
```

---

### 3. Index Price State (vol-datasource/src/deribit.rs)

添加线程安全的指数价格状态管理：

```rust
/// Index price 状态 - 线程安全的共享状态
#[derive(Debug, Clone, Default)]
pub struct IndexPriceState {
    prices: Arc<Mutex<HashMap<String, f64>>>,  // e.g., "btc_usd" -> 95000.0
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

---

### 4. DeribitDataSource Data Merger (vol-datasource/src/deribit.rs)

改造数据源实现，合并多个频道数据流：

```rust
pub struct DeribitDataSource {
    client: DeribitClient,
    index_price_state: IndexPriceState,
    symbols: Vec<String>,
}

impl DeribitDataSource {
    pub fn new(ws_url: String, symbols: Vec<String>, _poll_interval_secs: u64) -> Self {
        let client = DeribitClient::new(ws_url);
        Self {
            client,
            index_price_state: IndexPriceState::new(),
            symbols,
        }
    }

    fn spawn_data_merger(
        mut options_rx: mpsc::Receiver<ChannelData>,
        mut index_rx: mpsc::Receiver<ChannelData>,
        tx: mpsc::Sender<VolatilityData>,
        index_state: IndexPriceState,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    // 处理 index price 更新
                    Some(ChannelData::PriceIndex(price_data)) = index_rx.recv() => {
                        index_state.update(&price_data.index_name, price_data.price).await;
                    }

                    // 处理 options mark price
                    Some(ChannelData::OptionMarkPrice(options_list)) = options_rx.recv() => {
                        for option in options_list {
                            // 从状态中获取 index price
                            let underlying = extract_underlying(&option.instrument_name);
                            let index_key = format!("{}_usd", underlying.to_lowercase());
                            let index_price = index_state.get(&index_key).await;

                            // 构造 VolatilityData
                            if let Some(vol_data) = option.to_volatility_data_with_index(index_price) {
                                let _ = tx.send(vol_data).await;
                            }
                        }
                    }
                }
            }
        });
    }

    fn subscribe(&self, symbols: Vec<String>) -> Result<mpsc::Receiver<VolatilityData>> {
        let (tx, rx) = mpsc::channel(1024);

        // 为每个 symbol 订阅两个频道
        let mut options_channels = Vec::new();
        let mut index_channels = Vec::new();

        for symbol in &symbols {
            options_channels.push(ChannelType::MarkpriceOptions(format!("{}_usd", symbol.to_lowercase())));
            index_channels.push(ChannelType::PriceIndex(format!("{}_usd", symbol.to_lowercase())));
        }

        // 创建接收器
        // 注意：实际实现需要处理多个频道的合并，可以使用 futures::stream::select_all

        Ok(rx)
    }
}
```

---

### 5. OptionMarkPrice Extension (vol-deribit/src/market_data.rs)

添加支持外部 index price 的转换方法：

```rust
impl OptionMarkPrice {
    /// 使用外部提供的 index price 转换
    pub fn to_volatility_data_with_index(
        &self,
        index_price: Option<f64>,
    ) -> Option<vol_core::VolatilityData> {
        let iv = self.iv?;
        let (underlying, _year, month, day, strike, option_type) =
            crate::instrument::parse_instrument_name(&self.instrument_name)?;

        // 使用传入的 index price，如果没有则 fallback 到 strike
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

    /// 保留旧方法，标记为 deprecated
    #[deprecated(note = "Use to_volatility_data_with_index instead")]
    pub fn to_volatility_data(&self) -> Option<vol_core::VolatilityData> {
        self.to_volatility_data_with_index(None)
    }
}
```

---

## Data Flow Diagram

```
┌─────────────────────────────────────────────────────────────┐
│                     Deribit WebSocket                        │
└─────────────────────────┬───────────────────────────────────┘
                          │
                          ▼
┌─────────────────────────────────────────────────────────────┐
│                    DeribitClient                             │
│  ┌─────────────┐                                             │
│  │ subscribe() │── 订阅 markprice.options.btc_usd            │
│  └─────────────┘                                             │
│  ┌─────────────┐                                             │
│  │ subscribe() │── 订阅 deribit_price_index.btc_usd          │
│  └─────────────┘                                             │
└──────────┬──────────────────┬────────────────────────────────┘
           │                  │
           │ ChannelData      │ ChannelData
           │ (OptionMarkPrice)│ (PriceIndex)
           ▼                  ▼
┌─────────────────────────────────────────────────────────────┐
│                  DeribitDataSource                           │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                  IndexPriceState                      │   │
│  │  btc_usd: 95000.0                                     │   │
│  │  eth_usd: 3800.0                                      │   │
│  └──────────────────────────────────────────────────────┘   │
│                                                              │
│  tokio::select! 合并两个 channel:                            │
│  - PriceIndex → 更新 IndexPriceState                         │
│  - OptionMarkPrice → 从 State 获取 index_price → VolatilityData│
└─────────────────────────┬────────────────────────────────────┘
                          │
                          ▼
              VolatilityData (带真实 index_price)
```

---

## Testing Strategy

1. **单元测试** - 测试 `ChannelData` 枚举的 `channel_name()` 方法
2. **集成测试** - 测试多频道订阅和数据合并逻辑
3. **日志验证** - 运行 vol-monitor 观察日志中 index price 是否为真实值

---

## Success Criteria

1. ✅ 成功订阅 `markprice.options.*` 和 `deribit_price_index.*` 两个频道
2. ✅ Index price 从 `deribit_price_index.*` 频道获取真实数据，不再使用硬编码默认值
3. ✅ 告警日志中显示真实的 index price 值
4. ✅ 代码结构支持未来轻松添加新频道类型（如 `ticker.*`、`trades.*`）
