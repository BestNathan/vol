# AdviceAgent 集成测试设计

**Goal:** 创建集成测试验证 AdviceAgent 在真实环境中的完整工作流程。

**Architecture:** 测试脚本启动 AdviceAgent，通过 broadcast channel 发送模拟预警，验证 ReAct Agent 分析、TDengine 工具查询、飞书通知发送的完整流程。

**Tech Stack:** Tokio test runtime, vol-llm-agents, vol-tdengine, vol-notification

---

## 1. 测试文件结构

### 文件：`crates/vol-llm-agents/tests/advice_agent_integration.rs` (新建)

```rust
//! AdviceAgent Integration Test
//!
//! This test verifies the complete workflow of AdviceAgent:
//! 1. Alert is sent via broadcast channel
//! 2. AdviceAgent receives and processes the alert
//! 3. ReAct Agent analyzes with real LLM API and TDengine tools
//! 4. Feishu notification is sent
//!
//! Requirements:
//! - ANTHROPIC_AUTH_TOKEN environment variable
//! - TDengine connection (env: TDENGINE_HOST, TDENGINE_USER, TDENGINE_PASS)
//! - Feishu credentials (env: FEISHU_APP_ID, FEISHU_APP_SECRET, FEISHU_RECEIVE_ID)
//!
//! Run with:
//! ```bash
//! cargo test -p vol-llm-agents --test advice_agent_integration -- --nocapture
//! ```

use vol_llm_agents::{AdviceAgent, AdviceAgentConfig};
use vol_llm_provider::{LLMProviderRegistry, LLMConfig};
use vol_llm_tool::ToolRegistry;
use vol_tdengine::{TdengineClient, TdengineConfig};
use vol_notification::FeishuNotification;
use vol_core::{Alert, AlertType, Tenor, OptionType};
use tokio::sync::broadcast;

#[tokio::test]
async fn test_advice_agent_end_to_end() {
    // Skip if not configured
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
        eprintln!("Skipping test: ANTHROPIC_AUTH_TOKEN not set");
        return;
    }

    // Setup components...
    // Create alert and send...
    // Verify processing completed...
}
```

---

## 2. 测试组件设置

### Step 1: LLM Provider Registry

```rust
let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
    .expect("ANTHROPIC_AUTH_TOKEN must be set");

let llm_config = LLMConfig::with_literal_key(
    vol_llm_core::LLMProvider::Anthropic,
    "qwen3.5-plus",
    api_key,
    "https://coding.dashscope.aliyuncs.com/apps/anthropic",
);

let registry = LLMProviderRegistry::from_configs(&[llm_config.clone()]);
```

### Step 2: ToolRegistry with TDengine Tools

```rust
let tdengine_config = TdengineConfig::default();
let tool_registry = Arc::new(ToolRegistry::new());

tool_registry.register(Arc::new(IndexPriceTool::new(Some(tdengine_config.clone()))));
tool_registry.register(Arc::new(VolatilityIndexTool::new(Some(tdengine_config.clone()))));
tool_registry.register(Arc::new(OptionsTool::new(Some(tdengine_config.clone()))));
tool_registry.register(Arc::new(RvTool::new(Some(tdengine_config.clone()))));
```

### Step 3: Tdengine Client

```rust
let tdengine_client = Arc::new(TdengineClient::new(&tdengine_config)
    .expect("Failed to create TDengine client"));
```

### Step 4: Feishu Notification

```rust
let feishu = FeishuNotification::from_env()
    .expect("FEISHU_APP_ID, FEISHU_APP_SECRET, FEISHU_RECEIVE_ID must be set");
```

### Step 5: AdviceAgent Config and Creation

```rust
let config = AdviceAgentConfig {
    enabled: true,
    cooldown_secs: 0,      // Disable cooldown for testing
    max_analyses_per_hour: 100, // High limit for testing
    llm_provider_id: "anthropic-main".to_string(),
};

let advice_agent = AdviceAgent::new(
    config,
    registry,
    tool_registry,
    tdengine_client,
    feishu,
);
```

---

## 3. 测试 Alert 创建

### Alert Channel Setup

```rust
let (alert_tx, alert_rx): (broadcast::Sender<TracedEvent<Alert>>, _) = 
    broadcast::channel(100);
```

### Test Alert

```rust
let test_alert = Alert {
    alert_type: AlertType::AbsoluteIv { threshold: 0.5 },
    tenor: Tenor::Short,
    symbol: "BTC".to_string(),
    iv: 0.55,  // Above threshold
    message: "IV exceeded threshold".to_string(),
    timestamp: 0,
    source: "test".to_string(),
    index_price: 50000.0,
    dte: 30,
    option_type: OptionType::Call,
    moneyness: 1.0,
    mark_price_coin: 0.05,
    trace_id: "test-integration-001".to_string(),
};
```

---

## 4. 测试执行流程

### Start AdviceAgent in Background

```rust
let agent_clone = advice_agent.clone();
let handle = tokio::spawn(async move {
    agent_clone.run(alert_rx).await.expect("AdviceAgent failed");
});
```

### Send Alert

```rust
let traced_alert = TracedEvent::new(test_alert.clone());
alert_tx.send(traced_alert).expect("Failed to send alert");
```

### Wait for Processing

```rust
// Give the agent time to process (max 30 seconds for LLM response)
tokio::time::sleep(Duration::from_secs(30)).await;

// Optionally abort the agent task
handle.abort();
```

---

## 5. 验证标准

### 日志验证

```rust
// Check that log file was created with agent analysis
let log_path = PathBuf::from("logs/agents/advice_agent");
assert!(log_path.exists(), "Agent log directory should exist");
```

### 飞书通知验证（可选）

如果飞书 webhook 返回成功，说明通知已发送。由于验证实际消息内容需要额外的 API 调用，第一阶段测试只验证发送成功。

---

## 6. 错误处理

### 环境变量缺失

```rust
#[tokio::test]
async fn test_advice_agent_end_to_end() {
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
        eprintln!("Skipping test: ANTHROPIC_AUTH_TOKEN not set");
        return; // Test passes by skipping
    }
    // ...
}
```

### TDengine 连接失败

```rust
let tdengine_client = match TdengineClient::new(&tdengine_config) {
    Ok(client) => Arc::new(client),
    Err(e) => {
        eprintln!("Skipping test: Failed to connect to TDengine: {}", e);
        return;
    }
};
```

---

## 7. 未来增强

1. **Mock TDengine 数据** - 不依赖真实 TDengine 实例，使用预设数据
2. **验证飞书消息内容** - 调用飞书 API 获取发送的消息并验证包含 AI 分析
3. **多预警类型测试** - 测试 RateChange、TermStructure 等不同预警类型
4. **频率限制测试** - 验证 cooldown 和 hourly limit 逻辑
5. **错误恢复测试** - 模拟 LLM API 失败，验证错误处理

---

## 8. 运行说明

### 环境准备

```bash
# 设置环境变量
export ANTHROPIC_AUTH_TOKEN=your_token_here
export TDENGINE_HOST=localhost
export TDENGINE_USER=root
export TDENGINE_PASS=your_password
export FEISHU_APP_ID=your_app_id
export FEISHU_APP_SECRET=your_app_secret
export FEISHU_RECEIVE_ID=your_receive_id
```

### 运行测试

```bash
cargo test -p vol-llm-agents --test advice_agent_integration -- --nocapture
```

### 预期输出

```
running 1 test
test_advice_agent_end_to_end ... ok
```

日志文件将输出到 `logs/agents/advice_agent/` 目录。
