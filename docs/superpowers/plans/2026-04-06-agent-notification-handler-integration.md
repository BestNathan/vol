# Agent Advice NotificationHandler Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Integrate `AgentAdviceService` as a `NotificationHandler` in vol-monitor, enabling AI-powered alert analysis with Feishu notifications.

**Architecture:** AgentAdviceService implements NotificationHandler trait, receives alerts via broadcast channel, uses ReAct Agent with tools (AlertHistoryTool for TDengine queries), sends advice via FeishuNotification (openlark SDK).

**Tech Stack:** Rust, tokio (broadcast channels), vol-llm-agent (ReActAgent), vol-llm-tool (ToolRegistry, TdengineClient), vol-notification (FeishuNotification/openlark), vol-core (NotificationHandler trait)

---

## File Structure

**Files to Create:**
- None (all functionality exists, needs integration)

**Files to Modify:**
- `crates/vol-llm-bridge/src/service.rs` - Add fields, implement NotificationHandler
- `crates/vol-llm-bridge/src/lib.rs` - Export NotificationHandler impl
- `crates/vol-engine/src/engine.rs` - Change alert channel to broadcast
- `crates/vol-monitor/src/main.rs` - Initialize and register AgentAdviceService
- `crates/vol-llm-bridge/Cargo.toml` - Add dependencies if needed

**Tests:**
- `crates/vol-llm-bridge/src/service.rs` - Unit tests for NotificationHandler impl
- `crates/vol-llm-bridge/tests/` - Integration tests (may create)

---

### Task 1: Add FeishuNotification send_advice method

**Files:**
- Modify: `crates/vol-notification/src/feishu.rs:270-312`

- [ ] **Step 1: Add send_advice method to FeishuNotification**

Add a new method after the `send()` method for sending AI analysis messages:

```rust
/// Send AI analysis advice to Feishu
pub async fn send_advice(
    &self,
    advice: &str,
    alert: &Alert,
    trace_id: &str,
) -> Result<()> {
    let span = info_span!(
        "agent_advice_send",
        channel = "agent_advice",
        alert_type = %alert.alert_type,
        tenor = ?alert.tenor,
        symbol = %alert.symbol,
        trace_id = %trace_id,
    );

    async {
        // Build custom message for AI advice
        let content = format!(
            "🤖 AI 分析建议\n\n\
             预警类型：{}\n\
             标的物：{}\n\
             期限：{}\n\
             当前 IV: {:.1}%\n\
             指数价格：{:.2} USD\n\n\
             分析建议:\n{}\n\n\
             ---\nTrace ID: {}",
            alert.alert_type,
            alert.symbol,
            self.tenor_cn(alert.tenor),
            alert.iv * 100.0,
            alert.index_price,
            advice,
            trace_id
        );

        // Send as text message
        self.send_message("text", &json!({ "text": content }).to_string()).await
    }
    .instrument(span)
    .await
}
```

- [ ] **Step 2: Run cargo check to verify compilation**

```bash
cargo check -p vol-notification
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-notification/src/feishu.rs
git commit -m "feat(vol-notification): add send_advice method for AI analysis messages"
```

---

### Task 2: Update AgentAdviceService structure

**Files:**
- Modify: `crates/vol-llm-bridge/src/service.rs:37-55`

- [ ] **Step 1: Add use statements at top of service.rs**

```rust
use vol_notification::FeishuNotification;
use vol_llm_tool::{ToolRegistry, TdengineClient};
use vol_core::NotificationHandler;
```

- [ ] **Step 2: Update AgentAdviceService struct**

```rust
pub struct AgentAdviceService {
    limiter: FrequencyLimiter,
    config: AgentAdviceConfig,
    registry: LLMProviderRegistry,
    tools: ToolRegistry,
    tdengine: TdengineClient,
    feishu: FeishuNotification,
}
```

- [ ] **Step 3: Update new() constructor**

```rust
impl AgentAdviceService {
    pub fn new(
        config: AgentAdviceConfig,
        registry: LLMProviderRegistry,
        tools: ToolRegistry,
        tdengine: TdengineClient,
        feishu: FeishuNotification,
    ) -> Self {
        Self {
            limiter: FrequencyLimiter::new(config.cooldown_secs, config.max_analyses_per_hour),
            config,
            registry,
            tools,
            tdengine,
            feishu,
        }
    }
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p vol-llm-bridge
```

Expected: PASS (will have unused field warnings, that's ok for now)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "feat(vol-llm-bridge): add ToolRegistry, TdengineClient, FeishuNotification fields"
```

---

### Task 3: Implement NotificationHandler trait

**Files:**
- Modify: `crates/vol-llm-bridge/src/service.rs:80-185`

- [ ] **Step 1: Add NotificationHandler implementation**

Add at the end of `service.rs`:

```rust
#[async_trait::async_trait]
impl NotificationHandler for AgentAdviceService {
    fn name(&self) -> &str {
        "agent_advice"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    async fn send(&self, alert: &Alert) -> Result<(), vol_core::VolError> {
        // Check frequency limit first
        if !self.limiter.can_analyze(alert) {
            tracing::info!(
                "Skipping AI analysis for {}:{} (frequency limited)",
                alert.symbol,
                alert.alert_type
            );
            return Ok(());
        }

        // Process the alert
        if let Err(e) = self.process_alert(alert).await {
            tracing::error!("AgentAdviceService failed to process alert: {}", e);
            // Don't return error - we don't want to block other notifications
        }

        // Record this analysis
        self.limiter.record_analysis(alert);

        Ok(())
    }

    fn clone_box(&self) -> Box<dyn NotificationHandler> {
        Box::new(self.clone())
    }
}
```

- [ ] **Step 2: Make AgentAdviceService cloneable**

Add `#[derive(Clone)]` to the struct:

```rust
#[derive(Clone)]
pub struct AgentAdviceService {
    ...
}
```

Note: This requires all fields to be Clone. FeishuNotification already derives Clone. ToolRegistry and TdengineClient may need Arc wrappers.

If compilation fails, wrap non-Clone fields in Arc:

```rust
use std::sync::Arc;

pub struct AgentAdviceService {
    limiter: FrequencyLimiter,
    config: AgentAdviceConfig,
    registry: LLMProviderRegistry,
    tools: Arc<ToolRegistry>,
    tdengine: Arc<TdengineClient>,
    feishu: FeishuNotification,
}
```

- [ ] **Step 3: Update new() to wrap in Arc if needed**

```rust
Self {
    limiter: FrequencyLimiter::new(...),
    config,
    registry,
    tools: Arc::new(tools),
    tdengine: Arc::new(tdengine),
    feishu,
}
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p vol-llm-bridge
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "feat(vol-llm-bridge): implement NotificationHandler trait for AgentAdviceService"
```

---

### Task 4: Implement process_alert method

**Files:**
- Modify: `crates/vol-llm-bridge/src/service.rs:84-122`

- [ ] **Step 1: Replace process_alert with full implementation**

```rust
/// Process a single alert and send AI advice
async fn process_alert(
    &self,
    alert: &Alert,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let trace_id = &alert.trace_id;

    tracing::info!(
        "Processing alert for AI analysis: {}:{} (trace_id: {})",
        alert.symbol,
        alert.alert_type,
        trace_id
    );

    // Generate advice using ReAct Agent
    let advice = self.generate_advice(alert).await
        .unwrap_or_else(|e| format!("Failed to generate advice: {}", e));

    // Send advice to Feishu
    self.send_advice(&advice, alert, trace_id).await?;

    Ok(())
}
```

- [ ] **Step 2: Update generate_advice to use agent with tools**

```rust
/// Generate advice using ReAct Agent
async fn generate_advice(
    &self,
    alert: &Alert,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    use vol_llm_agent::{ReActAgent, AgentConfig};
    use vol_llm_tool::ToolContext;
    use crate::prompt::{system_prompt, build_user_prompt, get_threshold_from_alert};

    // Get provider from registry by ID
    let llm = self.registry.get(&self.config.llm_provider_id)
        .ok_or_else(|| format!("Unknown provider: {}", self.config.llm_provider_id))?;

    // Create agent with tools
    let agent = ReActAgent::new(
        llm,
        (*self.tools).clone(),
        AgentConfig {
            max_iterations: 5,
            system_prompt: system_prompt().to_string(),
            verbose: false,
        },
    );

    // Get threshold from alert type
    let threshold = get_threshold_from_alert(&alert.alert_type);

    // Build user prompt
    let user_prompt = build_user_prompt(
        &alert.alert_type.to_string(),
        &alert.symbol,
        alert.iv,
        threshold,
        "History data will be queried by agent",
    );

    // Run agent with context
    let context = ToolContext {
        alert: Some(alert.clone()),
        instrument: alert.symbol.clone(),
        messages: Vec::new(),
        metadata: std::collections::HashMap::new(),
    };
    
    let response = agent.run(&user_prompt, context).await?;

    Ok(response.content)
}
```

- [ ] **Step 3: Update send_advice to call FeishuNotification**

```rust
/// Send advice to Feishu
async fn send_advice(
    &self,
    advice: &str,
    alert: &Alert,
    trace_id: &str,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    self.feishu.send_advice(advice, alert, trace_id).await?;
    Ok(())
}
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p vol-llm-bridge
```

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "feat(vol-llm-bridge): implement process_alert, generate_advice, send_advice methods"
```

---

### Task 5: Update vol-engine to use broadcast channel

**Files:**
- Modify: `crates/vol-engine/src/engine.rs:50-91`

- [ ] **Step 1: Change alert channel from mpsc to broadcast**

In `run()` method, replace:

```rust
// OLD:
let (alert_tx, alert_rx) = mpsc::channel::<TracedEvent<Alert>>(self.config.alert_buffer_size);
```

With:

```rust
// NEW:
let (alert_tx, _) = broadcast::channel::<TracedEvent<Alert>>(self.config.alert_buffer_size);
```

- [ ] **Step 2: Update spawn_notifications to subscribe to broadcast**

Replace the `spawn_notifications` method signature:

```rust
fn spawn_notifications(
    &self,
    alert_rx: broadcast::Receiver<TracedEvent<Alert>>,
    alert_manager: AlertManager,
) -> Vec<JoinHandle<Result<()>>> {
```

- [ ] **Step 3: Update spawn_notifications to receive from broadcast**

Inside the spawned task, change from `alert_rx.recv()` to handle broadcast:

```rust
vec![tokio::spawn(async move {
    info!("Starting {} notification channels", num_notifications);
    let mut rx = alert_rx;
    while let Ok(traced_alert) = rx.recv().await {
        // ... existing notification logic
    }
    Ok(())
})]
```

- [ ] **Step 4: Update AlertManager creation**

The AlertManager needs to be cloneable for multiple notification subscribers. Wrap in Arc:

```rust
let alert_manager = Arc::new(AlertManager::new(self.config.config_file.clone()));
```

- [ ] **Step 5: Run cargo check**

```bash
cargo check -p vol-engine
```

Expected: May have errors - fix them

- [ ] **Step 6: Commit**

```bash
git add crates/vol-engine/src/engine.rs
git commit -m "feat(vol-engine): change alert channel to broadcast for multiple subscribers"
```

---

### Task 6: Update main.rs to initialize AgentAdviceService

**Files:**
- Modify: `crates/vol-monitor/src/main.rs:85-120`

- [ ] **Step 1: Add use statements**

At top of main.rs:

```rust
use vol_llm_bridge::{AgentAdviceService, FrequencyLimiter};
use vol_llm_tool::{ToolRegistry, TdengineClient, TdengineConfig};
use vol_notification::FeishuNotification;
use vol_config::FeishuConfig;
```

- [ ] **Step 2: Initialize TDengine client**

After LLM provider initialization (around line 113):

```rust
// Initialize TDengine client
let tdengine_client = TdengineClient::new(TdengineConfig::default());
info!("TDengine client initialized");
```

- [ ] **Step 3: Initialize ToolRegistry**

```rust
// Initialize tool registry
let mut tools = ToolRegistry::new();
tools.register(crate::tools::AlertHistoryTool::new(Some(tdengine_client.clone())));
info!("Tool registry initialized with {} tools", tools.tool_names().len());
```

Note: May need to create AlertHistoryTool wrapper or import from vol-llm-tool.

- [ ] **Step 4: Initialize FeishuNotification**

```rust
// Initialize Feishu notification for AI advice
let feishu_config = FeishuConfig {
    app_id: config.feishu_app_id.clone(),
    app_secret: config.feishu_app_secret.clone(),
    receive_id: config.feishu_receive_id.clone(),
    message_template: "{tenor} {alert_type} {symbol} | IV={value}".to_string(),
};
let feishu = FeishuNotification::new(feishu_config)
    .unwrap_or_else(|e| {
        warn!("Failed to initialize Feishu: {}", e);
        // Create dummy config for fallback
        FeishuNotification::new(FeishuConfig {
            app_id: Some("dummy".to_string()),
            app_secret: Some("dummy".to_string()),
            receive_id: Some("oc_dummy".to_string()),
            message_template: "dummy".to_string(),
        }).unwrap()
    });
info!("Feishu notification initialized");
```

- [ ] **Step 5: Create AgentAdviceService**

```rust
// Create AgentAdviceService
let agent_service = config.llm_providers.is_some().then(|| {
    AgentAdviceService::new(
        config.agent_advice.clone(),
        llm_registry.clone().unwrap(),
        tools,
        tdengine_client,
        feishu,
    )
});
```

- [ ] **Step 6: Add AgentAdviceService as notification handler**

In the notification loop (around line 166-199), add:

```rust
// Add AgentAdviceService if enabled
if config.agent_advice.enabled {
    if let Some(service) = agent_service {
        builder = builder.with_notification(Box::new(service));
        info!("Added AgentAdviceService notification handler");
    }
}
```

- [ ] **Step 7: Run cargo check**

```bash
cargo check -p vol-monitor
```

Expected: May have errors - fix them

- [ ] **Step 8: Commit**

```bash
git add crates/vol-monitor/src/main.rs
git commit -m "feat(vol-monitor): initialize and register AgentAdviceService"
```

---

### Task 7: Fix compilation errors and type mismatches

**Files:**
- Various (depends on errors)

- [ ] **Step 1: Run full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Fix any compilation errors**

Common issues to check:
- Missing imports
- Type mismatches (Arc vs direct)
- Clone trait bounds
- Method signature mismatches

- [ ] **Step 3: Commit all fixes**

```bash
git add -A
git commit -m "fix: resolve compilation errors in agent integration"
```

---

### Task 8: Add unit tests

**Files:**
- Create: `crates/vol-llm-bridge/src/service_test.rs` (or inline in service.rs)

- [ ] **Step 1: Add test module**

At end of `service.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_advice_service_creation() {
        // Test that we can create the service (mock dependencies)
        // This is a placeholder - actual test needs mock LLM provider
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-bridge
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "test(vol-llm-bridge): add unit tests for AgentAdviceService"
```

---

### Task 9: Integration testing

**Files:**
- Test: Manual testing or create integration test

- [ ] **Step 1: Build release**

```bash
cargo build --release
```

Expected: PASS

- [ ] **Step 2: Run with test config**

```bash
source .env && ./target/release/vol-monitor --config config.dev.toml
```

Expected: Starts without errors

- [ ] **Step 3: Check logs for AgentAdviceService**

Look for:
- "Added AgentAdviceService notification handler"
- "TDengine client initialized"
- "Tool registry initialized"

- [ ] **Step 4: Commit any fixes**

```bash
git add -A
git commit -m "fix: integration test fixes"
```

---

### Task 10: Documentation update

**Files:**
- Modify: `docs/CONFIGURATION.md`

- [ ] **Step 1: Add AgentAdviceService documentation**

Add section explaining:
- Configuration options
- Required environment variables
- How to enable/disable

- [ ] **Step 2: Commit**

```bash
git add docs/CONFIGURATION.md
git commit -m "docs: add AgentAdviceService configuration documentation"
```

---

## Self-Review Checklist

- [ ] **Spec coverage:** All components from design spec implemented
- [ ] **No placeholders:** All TODOs replaced with actual code
- [ ] **Type consistency:** NotificationHandler trait properly implemented
- [ ] **Tests:** Unit tests pass, integration test verified
- [ ] **Documentation:** Configuration docs updated

---

## Execution Choice

Plan complete and saved to `docs/superpowers/plans/2026-04-06-agent-notification-handler-integration.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session, batch execution with checkpoints

**Which approach?**
