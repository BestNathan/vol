# Design: Agent Alert Advice

## Why

用户收到预警后需要判断：
1. 这个预警有多严重？
2. 历史趋势如何？
3. 应该采取什么行动？

当前系统只发送原始预警数据，用户需要手动查询历史数据和分析。需要通过 AI Agent 自动完成分析并推送建议，帮助用户快速决策。

## What Changes

### 新增组件

```
┌─────────────────────────────────────────────────────────────┐
│                    vol-monitor (main)                        │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐  │
│  │  MonitoringEngine                                     │  │
│  │                                                        │  │
│  │  alert_rx → AlertManager → broadcast                  │  │
│  │                         │                             │  │
│  │         ┌───────────────┴───────────────┐            │  │
│  │         ↓                               ↓            │  │
│  │  ┌──────────────────┐         ┌───────────────────┐  │  │
│  │  │ Notification     │         │ AgentAdvice       │  │  │
│  │  │ Handler          │         │ Service           │  │  │
│  │  │ (stdout/Feishu)  │         │ (新增)            │  │  │
│  │  └──────────────────┘         │  - limiter        │  │  │
│  │                               │  - tdengine       │  │  │
│  │                               │  - agent          │  │  │
│  │                               │  - feishu         │  │  │
│  │                               └───────────────────┘  │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

### 数据流

```
1. Rule 生成 Alert
2. AlertManager cooldown check → 通过
3. broadcast.send(TracedEvent<Alert>)
4. ┌→ NotificationHandler → 飞书（原始预警）
   │
   └→ AgentAdviceService
         ├→ FrequencyLimiter 检查
         ├→ TdengineClient 查询历史数据
         ├→ ReActAgent 生成分析建议
         └→ FeishuNotification → 飞书（分析建议）
```

## How

### 1. 新增 crate: `vol-llm-bridge`

**位置**: `crates/vol-llm-bridge/Cargo.toml`

**依赖**:
```toml
[dependencies]
tokio = { workspace = true }
tracing = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
vol-core = { path = "../vol-core" }
vol-config = { path = "../vol-config" }
vol-tracing = { path = "../vol-tracing" }
vol-notification = { path = "../vol-notification" }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-provider = { path = "../vol-llm-provider" }
```

### 2. FrequencyLimiter

**文件**: `crates/vol-llm-bridge/src/limiter.rs`

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use vol_core::Alert;

pub struct FrequencyLimiter {
    cooldown_secs: u64,
    max_per_hour: u32,
    last_analysis: Arc<Mutex<HashMap<String, u64>>>,
    hourly_count: Arc<AtomicU32>,
    hour_start: Arc<Mutex<u64>>,
}

impl FrequencyLimiter {
    pub fn new(cooldown_secs: u64, max_per_hour: u32) -> Self;
    
    /// 检查是否允许分析
    pub fn can_analyze(&self, alert: &Alert) -> bool;
    
    /// 记录一次分析
    pub fn record_analysis(&self, alert: &Alert);
}

fn limiter_key(alert: &Alert) -> String {
    format!("{}:{}", alert.symbol, alert.alert_type)
}
```

### 3. AgentAdviceService

**文件**: `crates/vol-llm-bridge/src/service.rs`

```rust
use tokio::sync::broadcast;
use vol_core::Alert;
use vol_tracing::TracedEvent;
use vol_llm_agent::ReActAgent;
use vol_llm_tool::{ToolRegistry, ToolContext};
use vol_notification::FeishuNotification;
use crate::limiter::FrequencyLimiter;
use crate::TdengineClient;

pub struct AgentAdviceService {
    limiter: FrequencyLimiter,
    tdengine: TdengineClient,
    agent: ReActAgent,
    feishu: FeishuNotification,
}

impl AgentAdviceService {
    pub fn new(config: AgentAdviceConfig) -> Self;
    
    /// 运行服务，订阅 alert broadcast
    pub async fn run(
        &self,
        mut alert_rx: broadcast::Receiver<TracedEvent<Alert>>,
    ) -> Result<()>;
    
    /// 处理单个 alert
    async fn process_alert(&self, traced_alert: TracedEvent<Alert>) -> Result<()>;
    
    /// 查询历史数据
    async fn fetch_history(&self, symbol: &str) -> HistoryData;
    
    /// 生成分析建议
    async fn generate_advice(&self, alert: &Alert, history: HistoryData) -> String;
    
    /// 发送建议到飞书
    async fn send_advice(&self, advice: &str, trace_id: String) -> Result<()>;
}
```

### 4. 配置文件

**文件**: `crates/vol-config/src/lib.rs` 新增

```rust
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentAdviceConfig {
    /// 是否启用
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// 冷却时间（秒）
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: u64,
    
    /// 每小时最多分析次数
    #[serde(default = "default_max_per_hour")]
    pub max_analyses_per_hour: u32,
    
    /// LLM 配置
    pub llm: LLMConfig,
}

fn default_true() -> bool { true }
fn default_cooldown() -> u64 { 300 }  // 5 分钟
fn default_max_per_hour() -> u32 { 20 }
```

**config.toml 示例**:
```toml
[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20

[agent_advice.llm]
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_AUTH_TOKEN"
endpoint = "https://coding.dashscope.aliyuncs.com/apps/anthropic"
```

### 5. Agent Prompt

**文件**: `crates/vol-llm-bridge/src/prompt.rs`

```rust
pub fn system_prompt() -> &'static str {
    r#"你是一名衍生品市场风险分析师。

收到预警后，你需要：
1. 分析预警数据（IV、期限、symbol 等）
2. 结合历史数据了解趋势
3. 给出风险评估和操作建议

输出格式（严格遵循）：
🔔 预警分析建议

预警：{alert_type} - {symbol}
当前 IV: {iv} (阈值：{threshold})

📊 历史数据分析:
- 过去 1 小时 IV 变化 {change}%
- 过去 24 小时 IV 分位数：{percentile}%

⚠️ 风险等级：[高/中/低]

💡 建议:
{1-3 条具体操作建议}"#
}
```

### 6. 集成到 vol-monitor

**文件**: `crates/vol-monitor/src/main.rs`

```rust
// 创建 AgentAdviceService
if config.agent_advice.enabled {
    let advice_service = AgentAdviceService::new(config.agent_advice.clone());
    let advice_rx = alert_tx.subscribe();
    
    tokio::spawn(async move {
        advice_service.run(advice_rx).await
    });
}
```

## Capabilities

### New Capabilities
- `vol-llm-bridge`: AgentAdviceService，订阅 alert broadcast 并推送 AI 分析建议
- `vol-llm-bridge`: FrequencyLimiter，按 symbol:alert_type 限制分析频率
- `vol-config`: AgentAdviceConfig 配置支持

### Modified Capabilities
- `vol-monitor`: 启动时可选启用 AgentAdviceService

## Impact

- **无 Breaking Changes**: 完全新增，不影响现有代码
- **新增依赖**: vol-llm-bridge crate
- **配置变更**: 新增 [agent_advice] 配置段
- **环境影响**: 需要配置 ANTHROPIC_AUTH_TOKEN

## Testing Strategy

1. **单元测试**:
   - FrequencyLimiter 冷却逻辑
   - Prompt 格式化
   
2. **集成测试**:
   - AgentAdviceService 端到端流程
   - 与 TDengine 集成查询

3. **人工测试**:
   - 飞书消息格式验证
   - 频率限制验证
