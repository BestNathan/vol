# Channel-Based Monitoring Config Design

**Goal:** 重构配置结构以支持 Channel-based 架构，实现通用数据源抽象、多通知渠道、规则路由和热重载能力。

**Architecture:** 分层配置（Datasources/Rules/Notifications）+ 规则主导路由（规则决定发送给哪些通知）+ 热重载（文件监听 + 原子更新）。

**Tech Stack:** Rust, serde, serde_json, toml, notify-rs, tokio (RwLock, mpsc)

---

## 配置结构总览

```toml
# =============================
# 数据源层 - 定义数据输入
# =============================
[[datasources]]
id = "deribit-markets"
type = "websocket"
provider = "deribit"
ws_url = "wss://www.deribit.com/ws/api/v2"
channels = ["markprice.btc", "markprice.eth"]
auth = { client_id = "...", client_secret = "..." }
poll_interval_secs = 60
enabled = true

[[datasources]]
id = "internal-portfolio"
type = "http-poll"
provider = "internal"
url = "https://internal-api/portfolio"
poll_interval_secs = 30
enabled = true

# =============================
# 通知层 - 定义输出通道（被规则引用）
# =============================
[[notifications]]
id = "feishu-alerts"
type = "feishu"
app_id = "cli_xxx"
app_secret = "xxx"
receive_id = "oc_xxx"
enabled = true

[[notifications]]
id = "stdout-debug"
type = "stdout"
enabled = true

# =============================
# 规则层 - 定义处理逻辑 + 路由关系
# =============================
[[rules]]
id = "absolute-iv-btc"
type = "absolute-iv"
symbol = "BTC"
short_threshold = 0.55
medium_threshold = 0.53
long_threshold = 0.51
enabled = true
# 数据源：留空表示消费所有数据源（后续可扩展）
# datasources = ["deribit-markets"]
# 通知渠道：规则决定发送给谁
notifications = ["feishu-alerts", "stdout-debug"]

[[rules]]
id = "portfolio-margin"
type = "margin-ratio"
datasources = ["internal-portfolio"]
min_threshold = 1.2
enabled = true
notifications = ["feishu-alerts"]

# =============================
# 引擎全局配置
# =============================
[engine]
hot_reload = true
hot_reload_interval_secs = 30
channel_buffer_size = 1000
alert_cooldown_secs = 300

[tenors]
short_max_dte = 7
medium_min_dte = 20
medium_max_dte = 40
long_min_dte = 80
```

---

## 设计原则

| 原则 | 说明 |
|------|------|
| **ID 引用** | 每层元素有唯一 ID，通过 ID 引用建立关系 |
| **规则主导路由** | 规则配置 `notifications` 字段决定发送给哪些通知渠道 |
| **正交分层** | Datasource/Rule/Notification 三层独立，可单独增删改 |
| **渐进扩展** | 数据源路由当前留空（全量广播），后续可扩展精细路由 |
| **热重载** | 配置文件变化自动生效，原子性更新注册表 |

---

## 架构设计

```text
┌─────────────────────────────────────────────────────────────────┐
│                     Config Hot Reload                           │
│                    (file watcher + atomic)                      │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                      Engine Core                                │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────────────┐ │
│  │ Datasources │───▶│   Rules     │───▶│    Notifications    │ │
│  │  (Producers)│    │ (Processors)│    │     (Consumers)     │ │
│  │             │    │             │    │                     │ │
│  │ - deribit   │    │ - absolute- │    │ - feishu            │ │
│  │ - internal  │    │   iv        │    │ - stdout            │ │
│  │             │    │ - margin-   │    │                     │ │
│  │             │    │   ratio     │    │                     │ │
│  └─────────────┘    └─────────────┘    └─────────────────────┘ │
│       │                    │                    │               │
│       ▼                    ▼                    ▼               │
│  mpsc::Sender         mpsc::Sender         Handler trait       │
│  MonitoringEvent      Alert                 │                   │
│                                            │                   │
│                                    ┌───────┴───────┐           │
│                                    ▼               ▼           │
│                              ┌─────────┐   ┌─────────────┐     │
│                              │ Feishu  │   │ Stdout      │     │
│                              │ Client  │   │ Handler     │     │
│                              └─────────┘   └─────────────┘     │
└─────────────────────────────────────────────────────────────────┘
```

### 数据流

1. **Datasources → Rules**：`mpsc::Sender<MonitoringEvent>`，每个数据源产生事件广播到规则通道
2. **Rules → Notifications**：规则评估后产生告警，通过 `notification_ids()` 路由到具体通知处理器
3. **Notifications**：实现 `NotificationHandler` trait，规则通过 ID 列表调用对应处理器

---

## 接口定义

### DataSource Trait

```rust
// vol-core/src/datasource.rs
#[async_trait]
pub trait DataSource: Send + Sync {
    fn id(&self) -> &str;
    fn event_type(&self) -> EventType;

    /// 启动数据源，返回事件流
    async fn run(&self, tx: mpsc::Sender<MonitoringEvent>) -> Result<()>;

    /// 健康检查
    fn health_status(&self) -> HealthStatus;
}
```

### RuleProcessor Trait

```rust
// vol-core/src/rule.rs
#[async_trait]
pub trait RuleProcessor: Send + Sync {
    fn id(&self) -> &str;
    fn rule_type(&self) -> &str;

    /// 订阅的事件类型
    fn input_events(&self) -> Vec<EventType>;

    /// 评估事件，返回告警列表
    async fn evaluate(&self, event: &MonitoringEvent) -> Vec<Alert>;

    /// 获取配置的通知渠道 ID 列表
    fn notification_ids(&self) -> Vec<String>;
}
```

### NotificationHandler Trait

```rust
// vol-core/src/notification.rs
#[async_trait]
pub trait NotificationHandler: Send + Sync {
    fn id(&self) -> &str;
    fn handler_type(&self) -> &str;

    /// 发送告警
    async fn send(&self, alert: &Alert) -> Result<()>;

    /// 批量发送（可选优化）
    async fn send_batch(&self, alerts: &[Alert]) -> Result<()> {
        for alert in alerts {
            self.send(alert).await?;
        }
        Ok(())
    }
}
```

### EventType 枚举

```rust
// vol-core/src/event.rs
#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum EventType {
    MarketData(MarketDataType),
    Portfolio(PortfolioMetricType),
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum MarketDataType {
    Ticker,
    MarkPrice,
    Trade,
    OrderBook,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum PortfolioMetricType {
    MarginRatio,
    FreeBalance,
    DeltaExposure,
    SessionPnL,
    Greeks,
}
```

---

## 热重载设计

### 配置包装

```rust
// vol-config/src/hot_reload.rs
pub struct HotConfig<T> {
    inner: Arc<RwLock<T>>,
    watcher: Option<ConfigWatcher>,
}

impl<T: Clone> HotConfig<T> {
    pub fn get(&self) -> T {
        self.inner.read().clone()
    }

    pub fn update(&self, new: T) {
        *self.inner.write() = new;
    }
}
```

### 运行时注册表

```rust
// vol-engine/src/registry.rs
pub struct RuleRegistry {
    rules: Arc<RwLock<HashMap<String, Box<dyn RuleProcessor>>>>,
    notification_map: Arc<RwLock<HashMap<String, Box<dyn NotificationHandler>>>>,
}

impl RuleRegistry {
    pub async fn get_notifications_for_rule(
        &self,
        rule_id: &str,
        config: &RuleConfig,
    ) -> Vec<Arc<dyn NotificationHandler>> {
        let notifs = self.notification_map.read().await;
        config.notifications
            .iter()
            .filter_map(|id| notifs.get(id).map(|n| n.clone()))
            .collect()
    }

    pub async fn reload_rules(&self, new_rules: Vec<Box<dyn RuleProcessor>>) {
        let mut rules = self.rules.write().await;
        *rules = new_rules.into_iter()
            .map(|r| (r.id().to_string(), r))
            .collect();
    }

    pub async fn reload_notifications(&self, new_notifs: Vec<Box<dyn NotificationHandler>>) {
        let mut notifs = self.notification_map.write().await;
        *notifs = new_notifs.into_iter()
            .map(|n| (n.id().to_string(), Arc::new(n)))
            .collect();
    }
}
```

### 热重载流程

```text
config.toml → ConfigWatcher → HotConfig → RuleRegistry → Engine Core
    │            │               │            │              │
 [修改保存]   [notify-rs]    [原子更新]  [重建注册表]   [暂停/恢复]
```

---

## 配置结构定义

```rust
// vol-config/src/lib.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub engine: EngineConfig,

    #[serde(default)]
    pub tenors: TenorConfig,

    #[serde(default)]
    pub datasources: Vec<DataSourceConfig>,

    #[serde(default)]
    pub notifications: Vec<NotificationConfig>,

    #[serde(default)]
    pub rules: Vec<RuleConfig>,
}

// vol-config/src/datasource.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum DataSourceConfig {
    #[serde(rename = "websocket")]
    WebSocket(WebSocketDataSourceConfig),

    #[serde(rename = "http-poll")]
    HttpPoll(HttpPollDataSourceConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebSocketDataSourceConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub provider: String,
    pub ws_url: String,
    pub channels: Vec<String>,
    #[serde(default)]
    pub auth: Option<DeribitAuthConfig>,
    #[serde(default = "default_60")]
    pub poll_interval_secs: u64,
}

// vol-config/src/notification.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum NotificationConfig {
    #[serde(rename = "stdout")]
    Stdout(StdoutNotificationConfig),

    #[serde(rename = "feishu")]
    Feishu(FeishuNotificationConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FeishuNotificationConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub app_id: String,
    pub app_secret: String,
    pub receive_id: String,
}

// vol-config/src/rule.rs
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum RuleConfig {
    #[serde(rename = "absolute-iv")]
    AbsoluteIv(AbsoluteIvRuleConfig),

    #[serde(rename = "rate-change")]
    RateChange(RateChangeRuleConfig),

    #[serde(rename = "margin-ratio")]
    MarginRatio(MarginRatioRuleConfig),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AbsoluteIvRuleConfig {
    pub id: String,
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[serde(default)]
    pub datasources: Vec<String>,

    pub symbol: String,
    pub short_threshold: f64,
    pub medium_threshold: f64,
    pub long_threshold: f64,

    #[serde(default)]
    pub notifications: Vec<String>,
}
```

---

## 实施计划

### 阶段 1：核心抽象（vol-core）

| 文件 | 变更 |
|------|------|
| `vol-core/src/event.rs` | 新建 `MonitoringEvent` 枚举 |
| `vol-core/src/datasource.rs` | 更新 `DataSource` Trait |
| `vol-core/src/rule.rs` | 新建 `RuleProcessor` Trait |
| `vol-core/src/notification.rs` | 重命名 `NotificationHandler` |

### 阶段 2：配置结构（vol-config）

| 文件 | 变更 |
|------|------|
| `vol-config/src/lib.rs` | 新 `Config` 结构 |
| `vol-config/src/datasource.rs` | 数据源配置枚举 |
| `vol-config/src/notification.rs` | 通知配置枚举 |
| `vol-config/src/rule.rs` | 规则配置枚举 |
| `vol-config/src/hot_reload.rs` | 热重载支持 |

### 阶段 3：引擎实现（vol-engine）

| 文件 | 变更 |
|------|------|
| `vol-engine/src/lib.rs` | crate 根 |
| `vol-engine/src/engine.rs` | `MonitoringEngine` |
| `vol-engine/src/builder.rs` | `MonitoringEngineBuilder` |
| `vol-engine/src/registry.rs` | `RuleRegistry` |

### 阶段 4：适配现有实现

| Crate | 变更 |
|-------|------|
| `vol-datasource` | 实现新 `DataSource` Trait |
| `vol-rules` | 实现 `RuleProcessor` Trait |
| `vol-notification` | 实现 `NotificationHandler` Trait |

### 阶段 5：入口整合

| 文件 | 变更 |
|------|------|
| `vol-monitor/src/main.rs` | 使用新引擎 |
| `config.toml` | 新配置格式 |

---

## 配置示例

### 完整配置示例

```toml
# 引擎配置
[engine]
hot_reload = true
hot_reload_interval_secs = 30
channel_buffer_size = 1000
alert_cooldown_secs = 300

# 期限分类
[tenors]
short_max_dte = 7
medium_min_dte = 20
medium_max_dte = 40
long_min_dte = 80

# 数据源
[[datasources]]
id = "deribit-markets"
type = "websocket"
provider = "deribit"
ws_url = "wss://www.deribit.com/ws/api/v2"
channels = ["markprice.btc", "markprice.eth"]
auth = { client_id = "nhXng7Bj", client_secret = "OxCGY10HlzgKfRoXPBRQqg5IBQcZguGPhE1tewP5U3Y" }
poll_interval_secs = 60
enabled = true

# 通知
[[notifications]]
id = "feishu-alerts"
type = "feishu"
app_id = "cli_a936b13197385bde"
app_secret = "JnWnFrrOvzHi4deDmFY9kd1NMGbiWuNz"
receive_id = "oc_c29208d94757e2aefd97bfa5f57e0b26"
enabled = true

[[notifications]]
id = "stdout"
type = "stdout"
enabled = true

# 规则
[[rules]]
id = "absolute-iv-btc"
type = "absolute-iv"
symbol = "BTC"
short_threshold = 0.55
medium_threshold = 0.53
long_threshold = 0.51
enabled = true
notifications = ["feishu-alerts", "stdout"]

[[rules]]
id = "absolute-iv-eth"
type = "absolute-iv"
symbol = "ETH"
short_threshold = 0.75
medium_threshold = 0.73
long_threshold = 0.71
enabled = true
notifications = ["feishu-alerts", "stdout"]

[[rules]]
id = "rate-change"
type = "rate-change"
symbol = "BTC"
window_1h_threshold = 0.05
window_4h_threshold = 0.10
window_24h_threshold = 0.20
enabled = true
notifications = ["feishu-alerts"]

[[rules]]
id = "portfolio-margin"
type = "margin-ratio"
datasources = ["internal-portfolio"]
min_threshold = 1.2
enabled = true
notifications = ["feishu-alerts"]
```

---

## 自审记录

- [x] **Placeholder 扫描** - 无 TBD/TODO
- [x] **内部一致性** - 配置方向：规则→通知，Trait 命名统一（DataSource/RuleProcessor/NotificationHandler）
- [x] **Scope 检查** - 聚焦配置重构，不包含 Sink 抽象（后续设计）
- [x] **Ambiguity 检查** - 数据源路由留空后续扩展，已明确说明"当前全量广播"
