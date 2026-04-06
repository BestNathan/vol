# Agent Alert Advice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 创建一个独立的 AgentAdviceService，订阅预警 broadcast 并通过飞书推送 AI 分析建议。

**Architecture:** 新增 vol-llm-bridge crate，包含 FrequencyLimiter（频率限制）、AgentAdviceService（核心服务）、prompt 模板。服务独立订阅 alert broadcast，查询 TDengine 历史数据，调用 ReActAgent 生成建议，通过飞书发送。

**Tech Stack:** Rust, tokio, vol-llm-agent, vol-llm-tool, vol-notification, TDengine REST API, Anthropic/DashScope LLM

---

### Task 1: 创建 vol-llm-bridge crate

**Files:**
- Create: `crates/vol-llm-bridge/Cargo.toml`
- Create: `crates/vol-llm-bridge/src/lib.rs`

- [ ] **Step 1: 创建 Cargo.toml**

```toml
[package]
name = "vol-llm-bridge"
version.workspace = true
edition.workspace = true

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

- [ ] **Step 2: 创建 src/lib.rs**

```rust
//! vol-llm-bridge: AI-powered alert analysis and advice service.
//!
//! Subscribes to alert broadcast, queries historical data from TDengine,
//! generates analysis advice using ReAct Agent, and sends to Feishu.

pub mod limiter;
pub mod service;
pub mod prompt;

pub use limiter::FrequencyLimiter;
pub use service::AgentAdviceService;
pub use prompt::system_prompt;
```

- [ ] **Step 3: 验证 workspace 编译**

Run: `cargo check --workspace`
Expected: vol-llm-bridge 被识别为 workspace 成员（可能需要更新 workspace Cargo.toml）

- [ ] **Step 4: 更新 workspace Cargo.toml（如需要）**

如果 Step 3 失败，修改 `crates/Cargo.toml` 或根 `Cargo.toml` 添加 vol-llm-bridge 到 members。

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-bridge/
git commit -m "feat(vol-llm-bridge): create new crate for AI alert advice"
```

---

### Task 2: 实现 FrequencyLimiter

**Files:**
- Create: `crates/vol-llm-bridge/src/limiter.rs`
- Create: `crates/vol-llm-bridge/src/limiter_test.rs` (test)

- [ ] **Step 1: 创建 limiter.rs**

```rust
//! Frequency limiter for alert analysis.
//!
//! Prevents over-analysis by limiting:
//! - Per (symbol, alert_type) cooldown (default 5 min)
//! - Global hourly limit (default 20/hour)

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU32, Ordering};
use vol_core::Alert;

/// Creates a unique key for rate limiting
pub fn limiter_key(alert: &Alert) -> String {
    format!("{}:{}", alert.symbol, alert.alert_type)
}

/// Frequency limiter with per-key cooldown and global hourly limit
pub struct FrequencyLimiter {
    cooldown_secs: u64,
    max_per_hour: u32,
    last_analysis: Arc<Mutex<HashMap<String, u64>>>,
    hourly_count: Arc<AtomicU32>,
    hour_start: Arc<Mutex<u64>>,
}

impl FrequencyLimiter {
    /// Create new limiter with cooldown (seconds) and max analyses per hour
    pub fn new(cooldown_secs: u64, max_per_hour: u32) -> Self {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        Self {
            cooldown_secs,
            max_per_hour,
            last_analysis: Arc::new(Mutex::new(HashMap::new())),
            hourly_count: Arc::new(AtomicU32::new(0)),
            hour_start: Arc::new(Mutex::new(now)),
        }
    }

    /// Check if analysis is allowed for this alert
    pub fn can_analyze(&self, alert: &Alert) -> bool {
        // Check hourly limit first
        if !self.check_hourly_limit() {
            return false;
        }

        // Check per-key cooldown
        let key = limiter_key(alert);
        let now = alert.timestamp / 1000; // Convert ms to seconds
        
        let mut last_times = self.last_analysis.lock().unwrap();
        
        match last_times.get(&key) {
            Some(&last_time) => {
                if now - last_time < self.cooldown_secs {
                    return false;
                }
            }
            None => {}
        }

        true
    }

    /// Record that an analysis was performed
    pub fn record_analysis(&self, alert: &Alert) {
        let key = limiter_key(alert);
        let now = alert.timestamp / 1000;
        
        // Update per-key time
        {
            let mut last_times = self.last_analysis.lock().unwrap();
            last_times.insert(key, now);
        }

        // Increment hourly count
        self.increment_hourly_count();
    }

    fn check_hourly_limit(&self) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Check if we're in a new hour window
        {
            let mut hour_start = self.hour_start.lock().unwrap();
            if now - *hour_start >= 3600 {
                // New hour, reset counter
                *hour_start = now;
                self.hourly_count.store(0, Ordering::SeqCst);
            }
        }

        // Check current count
        self.hourly_count.load(Ordering::SeqCst) < self.max_per_hour
    }

    fn increment_hourly_count(&self) {
        self.hourly_count.fetch_add(1, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_core::{Alert, AlertType, OptionType, Tenor};

    fn create_test_alert(symbol: &str, alert_type: AlertType, timestamp: u64) -> Alert {
        Alert::new(
            alert_type,
            Tenor::Short,
            symbol.to_string(),
            0.5,
            "Test alert".to_string(),
            timestamp,
            "test".to_string(),
            50000.0,
            30,
            OptionType::Call,
            1.0,
            0.05,
            String::new(),
        )
    }

    #[test]
    fn test_first_alert_allowed() {
        let limiter = FrequencyLimiter::new(300, 20);
        let alert = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.7 }, 1000000);
        assert!(limiter.can_analyze(&alert));
    }

    #[test]
    fn test_cooldown_blocks_same_alert() {
        let limiter = FrequencyLimiter::new(300, 20);
        let alert1 = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.7 }, 1000000);
        
        // First alert - allowed
        assert!(limiter.can_analyze(&alert1));
        limiter.record_analysis(&alert1);

        // Second alert within cooldown - blocked
        let alert2 = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.7 }, 1000000 + 100);
        assert!(!limiter.can_analyze(&alert2));
    }

    #[test]
    fn test_different_symbols_independent() {
        let limiter = FrequencyLimiter::new(300, 20);
        let btc_alert = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.7 }, 1000000);
        let eth_alert = create_test_alert("ETH", AlertType::AbsoluteIv { threshold: 0.7 }, 1000000);

        // BTC alert - allowed and recorded
        assert!(limiter.can_analyze(&btc_alert));
        limiter.record_analysis(&btc_alert);

        // ETH alert - should be allowed (different key)
        assert!(limiter.can_analyze(&eth_alert));
    }

    #[test]
    fn test_different_alert_types_independent() {
        let limiter = FrequencyLimiter::new(300, 20);
        let iv_alert = create_test_alert("BTC", AlertType::AbsoluteIv { threshold: 0.7 }, 1000000);
        let rate_alert = create_test_alert("BTC", AlertType::RateChange { window_hours: 1, change_pct: 0.1 }, 1000000);

        // IV alert - allowed and recorded
        assert!(limiter.can_analyze(&iv_alert));
        limiter.record_analysis(&iv_alert);

        // RateChange alert - should be allowed (different type)
        assert!(limiter.can_analyze(&rate_alert));
    }
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --package vol-llm-bridge --lib limiter -- --nocapture`
Expected: 4 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-bridge/src/limiter.rs
git commit -m "feat(vol-llm-bridge): implement FrequencyLimiter with per-key cooldown"
```

---

### Task 3: 添加 AgentAdviceConfig 配置

**Files:**
- Modify: `crates/vol-config/src/config.rs` or `crates/vol-config/src/lib.rs`

- [ ] **Step 1: 读取现有配置结构**

先读取 `vol-config/src/lib.rs` 或 `vol-config/src/config.rs` 了解现有配置模式。

- [ ] **Step 2: 添加 AgentAdviceConfig 结构**

```rust
/// Agent advice configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentAdviceConfig {
    /// Whether agent advice is enabled
    #[serde(default = "default_true")]
    pub enabled: bool,
    
    /// Cooldown period in seconds between analyses of same (symbol, alert_type)
    #[serde(default = "default_cooldown")]
    pub cooldown_secs: u64,
    
    /// Maximum number of analyses per hour (global limit)
    #[serde(default = "default_max_per_hour")]
    pub max_analyses_per_hour: u32,
    
    /// LLM configuration for agent
    pub llm: LLMConfig,
}

fn default_true() -> bool { true }
fn default_cooldown() -> u64 { 300 }  // 5 minutes
fn default_max_per_hour() -> u32 { 20 }

impl Default for AgentAdviceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_secs: default_cooldown(),
            max_analyses_per_hour: default_max_per_hour(),
            llm: LLMConfig::default(),
        }
    }
}
```

- [ ] **Step 3: 添加到主配置文件**

确保 `EngineConfigFile` 或主配置结构包含 `agent_advice: AgentAdviceConfig` 字段。

- [ ] **Step 4: 运行编译检查**

Run: `cargo check --package vol-config`
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-config/src/
git commit -m "feat(vol-config): add AgentAdviceConfig for AI advice service"
```

---

### Task 4: 实现 AgentAdviceService 核心

**Files:**
- Create: `crates/vol-llm-bridge/src/service.rs`

- [ ] **Step 1: 创建 service.rs 骨架**

```rust
//! Agent Advice Service - subscribes to alerts and sends AI analysis.

use tokio::sync::broadcast;
use tracing::{info, debug, error, warn};
use vol_core::Alert;
use vol_tracing::TracedEvent;
use vol_config::AgentAdviceConfig;
use crate::limiter::FrequencyLimiter;

/// History data fetched from TDengine
#[derive(Debug, Clone, Default)]
pub struct HistoryData {
    pub iv_1h_change_pct: Option<f64>,
    pub iv_24h_percentile: Option<f64>,
    pub current_price: Option<f64>,
}

/// Agent Advice Service
pub struct AgentAdviceService {
    config: AgentAdviceConfig,
    limiter: FrequencyLimiter,
}

impl AgentAdviceService {
    /// Create new service from config
    pub fn new(config: AgentAdviceConfig) -> Self {
        let limiter = FrequencyLimiter::new(
            config.cooldown_secs,
            config.max_analyses_per_hour,
        );
        
        Self {
            config,
            limiter,
        }
    }

    /// Run the service, subscribing to alert broadcast
    pub async fn run(
        &self,
        mut alert_rx: broadcast::Receiver<TracedEvent<Alert>>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("AgentAdviceService started");
        
        while let Ok(traced_alert) = alert_rx.recv().await {
            if let Err(e) = self.process_alert(traced_alert).await {
                error!("Failed to process alert: {}", e);
            }
        }
        
        Ok(())
    }

    /// Process a single alert
    async fn process_alert(&self, traced_alert: TracedEvent<Alert>) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let (alert, _span, trace_id) = traced_alert.split();
        
        // Check frequency limit
        if !self.limiter.can_analyze(&alert) {
            debug!("Alert analysis rate-limited: {}:{:?}", alert.symbol, alert.alert_type);
            return Ok(());
        }

        info!("Processing alert for analysis: {}:{:?}", alert.symbol, alert.alert_type);

        // Fetch historical data
        let history = self.fetch_history(&alert.symbol).await;

        // Generate advice (placeholder - implemented in Task 6)
        let advice = self.generate_advice(&alert, &history).await;

        // Send to Feishu (placeholder - implemented in Task 7)
        self.send_advice(&advice, &trace_id).await?;

        // Record that we analyzed this alert
        self.limiter.record_analysis(&alert);

        Ok(())
    }

    /// Fetch historical data from TDengine
    async fn fetch_history(&self, symbol: &str) -> HistoryData {
        // Placeholder - implemented in Task 5
        HistoryData::default()
    }

    /// Generate analysis advice using ReAct Agent
    async fn generate_advice(&self, alert: &Alert, history: &HistoryData) -> String {
        // Placeholder - implemented in Task 6
        format!("Analysis for alert: {:?}", alert.alert_type)
    }

    /// Send advice to Feishu
    async fn send_advice(&self, advice: &str, trace_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Placeholder - implemented in Task 7
        info!("Would send advice: {}", advice);
        Ok(())
    }
}
```

- [ ] **Step 2: 运行编译检查**

Run: `cargo check --package vol-llm-bridge`
Expected: Compiles with placeholder implementations

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "feat(vol-llm-bridge): implement AgentAdviceService skeleton"
```

---

### Task 5: 实现 TDengine 历史数据查询

**Files:**
- Modify: `crates/vol-llm-bridge/src/service.rs`
- Use: `crates/vol-llm-tool/src/tdengine.rs` (existing TdengineClient)

- [ ] **Step 1: 更新 fetch_history 实现**

```rust
use vol_llm_tool::TdengineClient;

// Add to AgentAdviceService struct:
tdengine: TdengineClient,

// Update constructor:
pub fn new(config: AgentAdviceConfig) -> Self {
    let limiter = FrequencyLimiter::new(
        config.cooldown_secs,
        config.max_analyses_per_hour,
    );
    let tdengine = TdengineClient::new(vol_llm_tool::TdengineConfig::default());
    
    Self {
        config,
        limiter,
        tdengine,
    }
}

// Implement fetch_history:
async fn fetch_history(&self, symbol: &str) -> HistoryData {
    let mut history = HistoryData::default();

    // Convert symbol format (e.g., "BTC-29MAR24-70000-C" → "btc_usd" or "BTC")
    let index_symbol = symbol.split('-').next().unwrap_or(symbol);

    // Query 1h IV change from deribit_volatility_index
    if let Ok(response) = self.tdengine.query_alert_history(index_symbol, 60, Some(1)).await {
        if response.code == 0 {
            if let Some(data) = response.data.and_then(|d| d.as_array()) {
                if data.len() >= 2 {
                    // Calculate 1h change
                    let recent_iv = data[0].get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let older_iv = data[1].get(1).and_then(|v| v.as_f64()).unwrap_or(0.0);
                    if older_iv > 0.0 {
                        history.iv_1h_change_pct = Some(((recent_iv - older_iv) / older_iv) * 100.0);
                    }
                }
            }
        }
    }

    // Query 24h IV data for percentile calculation
    if let Ok(response) = self.tdengine.query_alert_history(index_symbol, 100, Some(24)).await {
        if response.code == 0 {
            if let Some(data) = response.data.and_then(|d| d.as_array()) {
                if !data.is_empty() {
                    // Simple percentile: where does current IV fall in 24h range?
                    let ivs: Vec<f64> = data.iter()
                        .filter_map(|row| row.get(1).and_then(|v| v.as_f64()))
                        .collect();
                    
                    if ivs.len() >= 2 {
                        let current_iv = ivs[0];
                        let count_above = ivs.iter().filter(|&&iv| iv >= current_iv).count();
                        history.iv_24h_percentile = Some((count_above as f64 / ivs.len() as f64) * 100.0);
                    }
                }
            }
        }
    }

    // Query current price from deribit_index_price
    if let Ok(response) = self.tdengine.query_market_data(index_symbol).await {
        if response.code == 0 {
            if let Some(data) = response.data.and_then(|d| d.as_array()).and_then(|arr| arr.first()) {
                history.current_price = data.get(1).and_then(|v| v.as_f64());
            }
        }
    }

    history
}
```

- [ ] **Step 2: 运行编译检查**

Run: `cargo check --package vol-llm-bridge`
Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "feat(vol-llm-bridge): implement TDengine history fetching"
```

---

### Task 6: 实现 Agent 分析建议生成

**Files:**
- Create: `crates/vol-llm-bridge/src/prompt.rs`
- Modify: `crates/vol-llm-bridge/src/service.rs`

- [ ] **Step 1: 创建 prompt.rs**

```rust
//! Agent system prompts for alert analysis.

/// System prompt for the analysis agent
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

/// Build user prompt from alert and history
pub fn build_user_prompt(alert: &vol_core::Alert, history: &crate::service::HistoryData) -> String {
    let change_str = match history.iv_1h_change_pct {
        Some(pct) if pct > 0 => format!("+{:.1}%", pct),
        Some(pct) => format!("{:.1}%", pct),
        None => "N/A".to_string(),
    };

    let percentile_str = match history.iv_24h_percentile {
        Some(p) => format!("{:.0}%", p),
        None => "N/A".to_string(),
    };

    let threshold = alert.get_threshold();

    format!(
        r#"分析以下预警：

预警类型：{:?}
Symbol: {}
当前 IV: {:.4}
阈值：{:.4}
期限：{:?}
消息：{}

历史数据:
- 过去 1 小时 IV 变化：{}
- 过去 24 小时 IV 分位数：{}
- 当前价格：{:?}

请给出风险评估和操作建议。"#,
        alert.alert_type,
        alert.symbol,
        alert.iv,
        threshold,
        alert.tenor,
        alert.message,
        change_str,
        percentile_str,
        history.current_price,
    )
}
```

- [ ] **Step 2: 更新 service.rs 集成 Agent**

```rust
use vol_llm_agent::ReActAgent;
use vol_llm_provider::{create_provider, LLMConfig};
use vol_llm_core::{ConversationRequest, Message};
use vol_llm_tool::{ToolRegistry, ToolContext};
use crate::prompt;

// Add to AgentAdviceService:
agent: ReActAgent,

// Update constructor:
pub fn new(config: AgentAdviceConfig) -> Self {
    let limiter = FrequencyLimiter::new(
        config.cooldown_secs,
        config.max_analyses_per_hour,
    );
    let tdengine = TdengineClient::new(vol_llm_tool::TdengineConfig::default());
    
    // Create ReActAgent with TDengine tools
    let mut tool_registry = ToolRegistry::new();
    tool_registry.register_default_tools();
    
    let llm_config = config.llm.clone();
    let llm = create_provider(&llm_config)
        .map(|provider| Box::new(provider) as Box<dyn vol_llm_core::LLMClient>)
        .unwrap_or_else(|_| {
            // Fallback to mock if API key not available
            Box::new(MockLLM::new())
        });
    
    let agent = ReActAgent::new(llm, tool_registry, vol_llm_agent::AgentConfig {
        max_iterations: 3,
        system_prompt: prompt::system_prompt().to_string(),
        verbose: false,
    });

    Self {
        config,
        limiter,
        tdengine,
        agent,
    }
}

// Implement generate_advice:
async fn generate_advice(&self, alert: &Alert, history: &HistoryData) -> String {
    let user_prompt = prompt::build_user_prompt(alert, history);
    
    let request = ConversationRequest::simple(&user_prompt);
    
    match self.agent.llm.converse(request).await {
        Ok(response) => {
            response.message.content
                .map(|c| c.as_str().to_string())
                .unwrap_or_else(|| "分析失败".to_string())
        }
        Err(e) => {
            error!("LLM call failed: {}", e);
            // Fallback to simple analysis
            self.fallback_advice(alert, history)
        }
    }
}

fn fallback_advice(&self, alert: &Alert, _history: &HistoryData) -> String {
    let risk_level = if alert.iv > alert.get_threshold() * 1.2 {
        "高"
    } else if alert.iv > alert.get_threshold() {
        "中"
    } else {
        "低"
    };

    format!(
        r#"🔔 预警分析建议

预警：{:?} - {}
当前 IV: {:.4} (阈值：{:.4})

📊 历史数据分析:
- 数据暂不可用

⚠️ 风险等级：{}

💡 建议:
1. 关注标的价格波动
2. 考虑调整仓位风险敞口"#,
        alert.alert_type,
        alert.symbol,
        alert.iv,
        alert.get_threshold(),
        risk_level,
    )
}
```

- [ ] **Step 3: 运行编译检查**

Run: `cargo check --package vol-llm-bridge`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-bridge/src/prompt.rs crates/vol-llm-bridge/src/service.rs
git commit -m "feat(vol-llm-bridge): implement Agent advice generation with fallback"
```

---

### Task 7: 实现飞书通知发送

**Files:**
- Modify: `crates/vol-llm-bridge/src/service.rs`

- [ ] **Step 1: 更新 send_advice 实现**

```rust
use vol_notification::FeishuNotification;

// Add to AgentAdviceService:
feishu: FeishuNotification,

// Update constructor (需要 FeishuConfig 或从环境变量加载):
let feishu = FeishuNotification::new_from_env();  // 假设已有此方法

// Implement send_advice:
async fn send_advice(&self, advice: &str, trace_id: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Create a simple message with the advice
    let message = format!(
        "🔔 预警分析建议\n\n{}",
        advice
    );

    // Use FeishuNotification to send
    // Note: May need to add a text-only send method to FeishuNotification
    self.feishu.send_text(&message).await?;

    info!("Advice sent to Feishu (trace_id: {})", trace_id);
    Ok(())
}
```

- [ ] **Step 2: 如需，扩展 FeishuNotification**

如果 `FeishuNotification` 没有 `send_text` 方法，需要添加：

```rust
// In crates/vol-notification/src/feishu.rs
impl FeishuNotification {
    pub async fn send_text(&self, text: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 使用现有 send 方法或实现简单的文本发送
        self.send(NotificationMessage::Text(text.to_string())).await
    }
}
```

- [ ] **Step 3: 运行编译检查**

Run: `cargo check --package vol-llm-bridge`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs crates/vol-notification/src/feishu.rs
git commit -m "feat(vol-llm-bridge): implement Feishu notification for advice"
```

---

### Task 8: 集成到 vol-monitor 主程序

**Files:**
- Modify: `crates/vol-monitor/src/main.rs`

- [ ] **Step 1: 读取现有 main.rs 结构**

先了解当前的启动流程和配置加载方式。

- [ ] **Step 2: 添加 AgentAdviceService 启动代码**

```rust
// 在启动 MonitoringEngine 后添加：
if config.agent_advice.enabled {
    info!("Starting AgentAdviceService...");
    
    let advice_config = config.agent_advice.clone();
    let advice_service = AgentAdviceService::new(advice_config);
    
    // Subscribe to alert broadcast (需要 access to alert_tx)
    let advice_rx = alert_tx.subscribe();
    
    tokio::spawn(async move {
        if let Err(e) = advice_service.run(advice_rx).await {
            error!("AgentAdviceService failed: {}", e);
        }
    });
    
    info!("AgentAdviceService started");
}
```

- [ ] **Step 3: 更新 Cargo.toml 依赖**

```toml
# In crates/vol-monitor/Cargo.toml
[dependencies]
vol-llm-bridge = { path = "../vol-llm-bridge" }
```

- [ ] **Step 4: 运行编译检查**

Run: `cargo check --package vol-monitor`
Expected: Compiles without errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-monitor/src/main.rs crates/vol-monitor/Cargo.toml
git commit -m "feat(vol-monitor): integrate AgentAdviceService"
```

---

### Task 9: 添加配置示例和文档

**Files:**
- Modify: `config.dev.toml`
- Modify: `.env.example`
- Create: `crates/vol-llm-bridge/README.md`

- [ ] **Step 1: 更新 config.dev.toml**

```toml
# At end of file:
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

- [ ] **Step 2: 更新.env.example**

```bash
# Agent Advice (optional)
ANTHROPIC_AUTH_TOKEN=sk-xxx
```

- [ ] **Step 3: 创建 vol-llm-bridge README**

```markdown
# vol-llm-bridge

AI-powered alert analysis service for volatility monitoring.

## Features

- Subscribes to alert broadcast from MonitoringEngine
- Queries historical data from TDengine
- Generates analysis advice using ReAct Agent (LLM)
- Sends structured advice to Feishu

## Configuration

```toml
[agent_advice]
enabled = true
cooldown_secs = 300        # 5 minutes between analyses of same alert
max_analyses_per_hour = 20 # Global hourly limit

[agent_advice.llm]
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_AUTH_TOKEN"
endpoint = "https://coding.dashscope.aliyuncs.com/apps/anthropic"
```

## Environment Variables

- `ANTHROPIC_AUTH_TOKEN`: Required for LLM API access
- `TDENGINE_HOST`: TDengine host (default: 192.168.2.106)
- `TDENGINE_PORT`: TDengine port (default: 6041)

## Usage

The service is automatically started by vol-monitor when `agent_advice.enabled = true`.
```

- [ ] **Step 4: Commit**

```bash
git add config.dev.toml .env.example crates/vol-llm-bridge/README.md
git commit -m "docs: add AgentAdvice configuration examples and README"
```

---

### Task 10: 单元测试

**Files:**
- Create: `crates/vol-llm-bridge/tests/prompt_test.rs`

- [ ] **Step 1: 创建 prompt 测试**

```rust
use vol_llm_bridge::prompt::{system_prompt, build_user_prompt};
use vol_core::{Alert, AlertType, OptionType, Tenor};
use vol_llm_bridge::service::HistoryData;

#[test]
fn test_system_prompt_contains_format() {
    let prompt = system_prompt();
    assert!(prompt.contains("预警分析建议"));
    assert!(prompt.contains("风险等级"));
    assert!(prompt.contains("建议"));
}

#[test]
fn test_user_prompt_format() {
    let alert = Alert::new(
        AlertType::AbsoluteIv { threshold: 0.7 },
        Tenor::Short,
        "BTC-29MAR24-70000-C".to_string(),
        0.85,
        "IV exceeds threshold".to_string(),
        1000000,
        "deribit".to_string(),
        50000.0,
        30,
        OptionType::Call,
        1.0,
        0.05,
        String::new(),
    );

    let history = HistoryData {
        iv_1h_change_pct: Some(15.5),
        iv_24h_percentile: Some(85.0),
        current_price: Some(50000.0),
    };

    let prompt = build_user_prompt(&alert, &history);
    
    assert!(prompt.contains("BTC-29MAR24-70000-C"));
    assert!(prompt.contains("+15.5%"));
    assert!(prompt.contains("85%"));
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --package vol-llm-bridge --test prompt_test -- --nocapture`
Expected: 2 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-bridge/tests/prompt_test.rs
git commit -m "test(vol-llm-bridge): add prompt unit tests"
```

---

### Task 11: 集成测试

**Files:**
- Create: `crates/vol-llm-bridge/tests/integration_test.rs`

- [ ] **Step 1: 创建集成测试（Mock LLM）**

```rust
use vol_llm_bridge::{AgentAdviceService, FrequencyLimiter};
use vol_config::AgentAdviceConfig;
use tokio::sync::broadcast;

#[tokio::test]
async fn test_service_accepts_first_alert() {
    let config = AgentAdviceConfig::default();
    let service = AgentAdviceService::new(config);
    
    // Create a mock alert and send through broadcast
    let (tx, rx) = broadcast::channel(10);
    
    // Spawn service and send test alert...
    // (Full implementation depends on mock setup)
}
```

- [ ] **Step 2: 运行测试**

Run: `cargo test --package vol-llm-bridge --test integration_test -- --nocapture`
Expected: Tests compile and run (may skip without full mock setup)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-bridge/tests/integration_test.rs
git commit -m "test(vol-llm-bridge): add integration test skeleton"
```

---

### Task 12: 文档和清理

**Files:**
- Create: `docs/AGENT_ADVICE.md`

- [ ] **Step 1: 创建使用指南**

```markdown
# Agent Advice 使用指南

## 概述

Agent Advice 功能会自动分析预警数据并结合历史趋势，通过 AI 生成风险评估和操作建议，推送到飞书。

## 启用功能

1. 在配置文件中启用：

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

2. 设置环境变量：

```bash
export ANTHROPIC_AUTH_TOKEN=sk-xxx
```

3. 启动服务：

```bash
./target/release/vol-monitor --config config.dev.toml
```

## 输出示例

```
🔔 预警分析建议

预警：AbsoluteIv - BTC-29MAR24-70000-C
当前 IV: 0.85 (阈值：0.70)

📊 历史数据分析:
- 过去 1 小时 IV 变化 +35%
- 过去 24 小时 IV 分位数：92%

⚠️ 风险等级：高

💡 建议:
1. 建议减仓或平仓该合约
2. 关注标的价格支撑位 $68,000
3. 考虑买入反向期权对冲
```

## 配置说明

| 配置项 | 说明 | 默认值 |
|--------|------|--------|
| `enabled` | 是否启用 | `true` |
| `cooldown_secs` | 同一预警组合的分析间隔（秒） | `300` |
| `max_analyses_per_hour` | 每小时最多分析次数 | `20` |

## 故障排查

### LLM API 调用失败

检查 `ANTHROPIC_AUTH_TOKEN` 是否正确设置。

服务会自动降级到简单分析模式。

### TDengine 查询失败

检查 TDengine 服务是否可访问（默认 192.168.2.106:6041）。

历史数据暂不可用时，使用预警数据本身进行简单分析。
```

- [ ] **Step 2: 运行最终编译检查**

Run: `cargo build --workspace`
Expected: All packages compile successfully

- [ ] **Step 3: 运行所有测试**

Run: `cargo test --workspace`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add docs/AGENT_ADVICE.md
git commit -m "docs: add AgentAdvice usage guide"
```

---

## 完成标准

- [ ] vol-llm-bridge crate 编译通过
- [ ] FrequencyLimiter 单元测试全部通过
- [ ] AgentAdviceService 集成测试通过
- [ ] vol-monitor 集成后编译通过
- [ ] 配置示例正确
- [ ] 文档完整

---

## 执行选项

**1. Subagent-Driven（推荐）** - 每个任务 dispatch 一个 subagent，任务间 review，快速迭代

**2. Inline Execution** - 在当前 session 使用 executing-plans 批量执行任务

选择哪种方式？
