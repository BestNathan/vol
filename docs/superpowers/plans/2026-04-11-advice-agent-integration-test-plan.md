# AdviceAgent 集成测试实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 创建集成测试验证 AdviceAgent 在真实环境中的完整工作流程（Alert → ReAct Agent → TDengine → Feishu）。

**Architecture:** 测试文件使用真实的 LLM Provider、TDengine Client、Feishu Notification，通过 broadcast channel 发送测试 Alert，验证 AdviceAgent 端到端流程。

**Tech Stack:** Tokio test runtime, vol-llm-agents, vol-tdengine, vol-notification, vol-core

---

## 文件结构

| 文件 | 变更类型 | 职责 |
|------|----------|------|
| `crates/vol-llm-agents/tests/advice_agent_integration.rs` | 创建 | 集成测试文件 |
| `crates/vol-llm-agents/Cargo.toml` | 修改 | 添加测试依赖（如需要） |

---

## Task 1: 创建测试文件骨架和环境检查

**Files:**
- Create: `crates/vol-llm-agents/tests/advice_agent_integration.rs`

- [ ] **Step 1: 创建测试文件**

创建文件 `crates/vol-llm-agents/tests/advice_agent_integration.rs`：

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

use vol_llm_agents::AdviceAgent;
use vol_core::{Alert, AlertType, Tenor, OptionType};

#[tokio::test]
async fn test_advice_agent_end_to_end() {
    // Skip if not configured
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
        eprintln!("Skipping test: ANTHROPIC_AUTH_TOKEN not set");
        return;
    }

    // TODO: Implement test
    panic!("Test not implemented");
}
```

- [ ] **Step 2: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 3: 运行测试验证跳过逻辑**

Run: `cargo test -p vol-llm-agents --test advice_agent_integration -- --nocapture`
Expected: Test is skipped with message "ANTHROPIC_AUTH_TOKEN not set"

- [ ] **Step 4: 提交**

```bash
git add crates/vol-llm-agents/tests/advice_agent_integration.rs
git commit -m "test: add AdviceAgent integration test skeleton"
```

---

## Task 2: 实现测试组件设置

**Files:**
- Modify: `crates/vol-llm-agents/tests/advice_agent_integration.rs`

- [ ] **Step 1: 添加 use 语句**

在文件顶部添加：

```rust
use vol_llm_agents::{AdviceAgent, AdviceAgentConfig};
use vol_llm_provider::{LLMProviderRegistry, LLMConfig};
use vol_llm_tool::ToolRegistry;
use vol_tdengine::{TdengineClient, TdengineConfig};
use vol_notification::FeishuNotification;
use vol_tracing::TracedEvent;
use tokio::sync::broadcast;
use std::sync::Arc;
use vol_llm_tdengine::{IndexPriceTool, VolatilityIndexTool, OptionsTool, RvTool};
```

- [ ] **Step 2: 实现 LLM Provider Registry 设置**

替换测试函数内容：

```rust
#[tokio::test]
async fn test_advice_agent_end_to_end() {
    // Skip if not configured
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_err() {
        eprintln!("Skipping test: ANTHROPIC_AUTH_TOKEN not set");
        return;
    }

    // Setup LLM Provider
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set");

    let llm_config = LLMConfig::with_literal_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        api_key,
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );

    let registry = LLMProviderRegistry::from_configs(&[llm_config.clone()]);
    
    println!("✓ LLM Provider configured");
}
```

- [ ] **Step 3: 添加 TDengine 和 ToolRegistry 设置**

在 LLM Provider 设置后添加：

```rust
    // Setup TDengine and Tools
    let tdengine_config = TdengineConfig::default();
    let tool_registry = Arc::new(ToolRegistry::new());

    tool_registry.register(Arc::new(IndexPriceTool::new(Some(tdengine_config.clone()))));
    tool_registry.register(Arc::new(VolatilityIndexTool::new(Some(tdengine_config.clone()))));
    tool_registry.register(Arc::new(OptionsTool::new(Some(tdengine_config.clone()))));
    tool_registry.register(Arc::new(RvTool::new(Some(tdengine_config.clone()))));

    println!("✓ TDengine tools registered");
```

- [ ] **Step 4: 添加 Feishu Notification 设置**

```rust
    // Setup Feishu Notification
    let feishu = FeishuNotification::from_env()
        .expect("FEISHU_APP_ID, FEISHU_APP_SECRET, FEISHU_RECEIVE_ID must be set");

    println!("✓ Feishu notification configured");
```

- [ ] **Step 5: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully (may have unused variable warnings)

- [ ] **Step 6: 提交**

```bash
git add crates/vol-llm-agents/tests/advice_agent_integration.rs
git commit -m "test: add AdviceAgent component setup"
```

---

## Task 3: 创建 AdviceAgent 实例和 Alert Channel

**Files:**
- Modify: `crates/vol-llm-agents/tests/advice_agent_integration.rs`

- [ ] **Step 1: 添加 AdviceAgent 配置和创建**

在 Feishu 设置后添加：

```rust
    // Setup AdviceAgent
    let config = AdviceAgentConfig {
        enabled: true,
        cooldown_secs: 0,      // Disable cooldown for testing
        max_analyses_per_hour: 100, // High limit for testing
        llm_provider_id: "anthropic-main".to_string(),
    };

    let tdengine_client = Arc::new(TdengineClient::new(&tdengine_config)
        .expect("Failed to create TDengine client"));

    let advice_agent = AdviceAgent::new(
        config,
        registry,
        tool_registry,
        tdengine_client,
        feishu,
    );

    println!("✓ AdviceAgent created");
```

- [ ] **Step 2: 添加 Alert Channel 设置**

```rust
    // Setup Alert channel
    let (alert_tx, alert_rx): (broadcast::Sender<TracedEvent<Alert>>, _) = 
        broadcast::channel(100);

    println!("✓ Alert channel created");
```

- [ ] **Step 3: 创建测试 Alert**

```rust
    // Create test alert
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

    println!("✓ Test alert created: BTC AbsoluteIv (IV=0.55, threshold=0.5)");
```

- [ ] **Step 4: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/tests/advice_agent_integration.rs
git commit -m "test: add AdviceAgent instance and alert setup"
```

---

## Task 4: 实现测试执行流程

**Files:**
- Modify: `crates/vol-llm-agents/tests/advice_agent_integration.rs`

- [ ] **Step 1: 启动 AdviceAgent 后台任务**

```rust
    // Start AdviceAgent in background
    let agent_clone = advice_agent.clone();
    let handle = tokio::spawn(async move {
        if let Err(e) = agent_clone.run(alert_rx).await {
            eprintln!("AdviceAgent error: {}", e);
        }
    });

    println!("✓ AdviceAgent started in background");
```

- [ ] **Step 2: 发送测试 Alert**

```rust
    // Send test alert
    let traced_alert = TracedEvent::new(test_alert.clone());
    alert_tx.send(traced_alert).expect("Failed to send alert");

    println!("✓ Test alert sent");
```

- [ ] **Step 3: 等待处理完成**

```rust
    // Wait for processing (LLM response may take time)
    println!("⏳ Waiting for AdviceAgent to process alert...");
    tokio::time::sleep(std::time::Duration::from_secs(30)).await;

    // Abort the agent task
    handle.abort();

    println!("✓ Test completed");
```

- [ ] **Step 4: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/tests/advice_agent_integration.rs
git commit -m "test: implement AdviceAgent test execution flow"
```

---

## Task 5: 添加验证逻辑和错误处理

**Files:**
- Modify: `crates/vol-llm-agents/tests/advice_agent_integration.rs`

- [ ] **Step 1: 添加 TDengine 连接失败处理**

在创建 TDengine client 之前添加 try-catch：

```rust
    // Setup TDengine and Tools
    let tdengine_config = TdengineConfig::default();
    
    let tdengine_client = match TdengineClient::new(&tdengine_config) {
        Ok(client) => Arc::new(client),
        Err(e) => {
            eprintln!("Skipping test: Failed to connect to TDengine: {}", e);
            return;
        }
    };

    let tool_registry = Arc::new(ToolRegistry::new());
    // ... rest of tool registration
```

- [ ] **Step 2: 添加日志目录验证**

在测试结束时添加：

```rust
    // Verify log directory was created
    let log_path = std::path::PathBuf::from("logs/agents/advice_agent");
    if log_path.exists() {
        println!("✓ Agent log directory exists: {:?}", log_path);
        
        // Check for run logs
        let runs_path = log_path.join("runs");
        if runs_path.exists() {
            if let Ok(entries) = std::fs::read_dir(&runs_path) {
                let count = entries.count();
                println!("✓ {} run log(s) created", count);
            }
        }
    }
```

- [ ] **Step 3: 添加 Feishu 发送失败处理**

测试不因为 Feishu 发送失败而失败，因为这是集成测试，重点验证流程：

```rust
    // Note: Feishu notification sending is best-effort in this test
    // The test passes if the agent processes the alert without panicking
```

- [ ] **Step 4: 验证编译**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 5: 提交**

```bash
git add crates/vol-llm-agents/tests/advice_agent_integration.rs
git commit -m "test: add error handling and log verification"
```

---

## Task 6: 运行完整测试并验证

**Files:**
- None (testing only)

- [ ] **Step 1: 设置完整环境变量**

```bash
export ANTHROPIC_AUTH_TOKEN=your_token_here
export TDENGINE_HOST=localhost
export TDENGINE_USER=root
export TDENGINE_PASS=your_password
export FEISHU_APP_ID=your_app_id
export FEISHU_APP_SECRET=your_app_secret
export FEISHU_RECEIVE_ID=your_receive_id
```

- [ ] **Step 2: 运行集成测试**

Run: `cargo test -p vol-llm-agents --test advice_agent_integration -- --nocapture`
Expected: Test runs for ~30 seconds, completes successfully

- [ ] **Step 3: 检查日志输出**

```bash
ls -la logs/agents/advice_agent/runs/
cat logs/agents/advice_agent/runs/*.jsonl
```
Expected: JSONL log files with Agent events (AgentStart, ThinkingComplete, ToolCallComplete, AgentComplete)

- [ ] **Step 4: 验证飞书通知（手动）**

检查飞书是否收到 AI 分析通知。

- [ ] **Step 5: 记录测试结果**

如果测试通过，记录在案。如果失败，分析原因并修复。

---

## 潜在问题

| 问题 | 缓解措施 |
|------|----------|
| TDengine 连接失败 | 测试跳过并打印错误信息 |
| LLM API 超时 | 30 秒等待时间，足够完成一次分析 |
| 飞书凭证无效 | 测试不依赖飞书返回，只验证流程 |
| 频率限制器阻止测试 | config 中设置 cooldown_secs=0, max_per_hour=100 |

---

## 成功标准

1. ✅ 测试文件编译通过
2. ✅ 测试在配置环境下运行完成（不 panic）
3. ✅ 日志目录和 run logs 被创建
4. ✅ 飞书收到 AI 分析通知（手动验证）
5. ✅ 测试在无配置环境下正确跳过

---

## 未来增强（不在本计划范围）

1. Mock TDengine 数据 - 不依赖真实 TDengine 实例
2. 验证飞书消息内容 - 调用飞书 API 获取并验证消息
3. 多预警类型测试 - 测试 RateChange、TermStructure 等
4. 频率限制测试 - 验证 cooldown 和 hourly limit 逻辑
5. 错误恢复测试 - 模拟 LLM API 失败
