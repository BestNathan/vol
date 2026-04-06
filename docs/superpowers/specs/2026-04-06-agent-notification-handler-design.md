# Agent Advice NotificationHandler Integration Design

> **Date:** 2026-04-06
> **Status:** Draft
> **Author:** Claude Code

## Goal

将 `AgentAdviceService` 集成到 `vol-monitor` 的通知系统中，通过实现 `NotificationHandler` trait，使其作为标准通知处理器接收 alert 并推送 AI 分析建议。

## Architecture

### Current State

- `AgentAdviceService` exists in `vol-llm-bridge` with basic structure
- Has `FrequencyLimiter` for rate limiting
- Has `LLMProviderRegistry` for LLM access
- Missing: `ToolRegistry`, `TdengineClient`, `FeishuClient`
- Not yet integrated with `vol-engine` notification flow

### Target Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      vol-monitor                             │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │              MonitoringEngine                         │   │
│  │                                                       │   │
│  │  Datasources → Rules → broadcast<Alert>              │   │
│  │                           │                           │   │
│  │        ┌──────────────────┼──────────────────┐       │   │
│  │        │                  │                  │       │   │
│  │        ▼                  ▼                  ▼       │   │
│  │  ┌──────────┐    ┌────────────────┐  ┌───────────┐  │   │
│  │  │ Stdout   │    │ FeishuNotif    │  │ Agent     │  │   │
│  │  │ Notif    │    │ (alert delivery)│  │ Advice    │  │   │
│  │  │          │    │                │  │ (analysis)│  │   │
│  │  └──────────┘    └────────────────┘  └───────────┘  │   │
│  │                                       │              │   │
│  │                                       ▼              │   │
│  │                                ┌────────────┐        │   │
│  │                                │ ReAct Agent│        │   │
│  │                                │ + Tools    │        │   │
│  │                                │ - alert_history    │   │
│  │                                │ - market_data      │   │
│  │                                └────────────┘        │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

| Decision | Approach | Rationale |
|----------|----------|-----------|
| **Integration Pattern** | Implement `NotificationHandler` on `AgentAdviceService` | Reuse existing service, minimal wrapper code |
| **Alert Distribution** | `broadcast::channel` | Support multiple subscribers (notifications + AgentAdvice) |
| **TDengine Access** | Via `ToolRegistry` + `AlertHistoryTool` | Agent decides when to query, follows ReAct pattern |
| **Feishu Client** | Dedicated lightweight client | Independent from `vol-notification`, custom message format |
| **Rate Limiting** | Existing `FrequencyLimiter` | Already implemented, prevents API abuse |

## Components

### 1. AgentAdviceService (Updated)

**Location:** `crates/vol-llm-bridge/src/service.rs`

**Current Structure:**
```rust
pub struct AgentAdviceService {
    limiter: FrequencyLimiter,
    config: AgentAdviceConfig,
    registry: LLMProviderRegistry,
}
```

**Target Structure:**
```rust
pub struct AgentAdviceService {
    limiter: FrequencyLimiter,
    config: AgentAdviceConfig,
    registry: LLMProviderRegistry,
    tools: ToolRegistry,           // NEW: for agent tool calling
    tdengine: TdengineClient,      // NEW: for historical data
    feishu: FeishuClient,          // NEW: dedicated Feishu client
}
```

**Methods:**
- `new(config, registry, tools, tdengine, feishu) -> Self`
- `process_alert(&self, alert: &Alert) -> Result<()>` - existing, adapted
- `generate_advice(&self, alert: &Alert) -> Result<String>` - uses agent
- `send_advice(&self, advice: &str, alert: &Alert) -> Result<()>` - uses Feishu

### 2. NotificationHandler Implementation

**Location:** `crates/vol-llm-bridge/src/service.rs`

```rust
#[async_trait]
impl NotificationHandler for AgentAdviceService {
    fn name(&self) -> &str {
        "agent_advice"
    }

    fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    async fn send(&self, alert: &Alert) -> Result<()> {
        // Check frequency limit
        if !self.limiter.can_analyze(alert) {
            return Ok(()); // Skip silently
        }

        // Process alert (existing logic)
        self.process_alert(alert).await?;

        // Record analysis
        self.limiter.record_analysis(alert);

        Ok(())
    }
}
```

### 3. Dedicated Feishu Client

**Location:** `crates/vol-llm-bridge/src/feishu.rs` (NEW)

```rust
pub struct FeishuClient {
    app_id: String,
    app_secret: String,
    receive_id: String,
    client: reqwest::Client,
}

impl FeishuClient {
    pub fn new(app_id: String, app_secret: String, receive_id: String) -> Self;
    pub async fn send_advice(&self, advice: &str, alert: &Alert) -> Result<()>;
}
```

**Message Format:**
```
【AI 分析建议】

预警类型：{alert_type}
标的物：{symbol}
期限：{tenor}
当前 IV: {iv}
阈值：{threshold}

分析建议：
{advice}

---
Trace ID: {trace_id}
```

### 4. ToolRegistry Setup

**Location:** `crates/vol-monitor/src/main.rs`

Tools to register:
- `AlertHistoryTool` - query TDengine for historical alerts
- `MarketDataTool` - get current market data
- `RuleInfoTool` - get rule configuration info

```rust
let mut tools = ToolRegistry::new();
tools.register(AlertHistoryTool::new(Some(tdengine_config)));
tools.register(MarketDataTool::new(deribit_client));
tools.register(RuleInfoTool::new());
```

### 5. Engine broadcast Channel

**Location:** `crates/vol-engine/src/engine.rs`

**Current:**
```rust
let (alert_tx, alert_rx) = mpsc::channel::<TracedEvent<Alert>>(self.config.alert_buffer_size);
```

**Target:**
```rust
let (alert_tx, _) = broadcast::channel::<TracedEvent<Alert>>(self.config.alert_buffer_size);

// Create notification subscriber (mpsc adapter)
let notif_rx = BroadcastToMpscAdapter::new(alert_tx.subscribe());

// Create AgentAdvice subscriber
let agent_rx = alert_tx.subscribe();
```

**Note:** Need to handle broadcast -> mpsc adaptation for existing notification flow, OR convert notifications to also use broadcast subscriptions.

### 6. main.rs Integration

**Location:** `crates/vol-monitor/src/main.rs`

```rust
// Initialize TDengine client
let tdengine_config = TdengineConfig::from_env()?;
let tdengine_client = TdengineClient::new(tdengine_config);

// Initialize tool registry
let mut tools = ToolRegistry::new();
tools.register(AlertHistoryTool::new(Some(tdengine_config.clone())));
// ... register other tools

// Initialize Feishu client for AgentAdvice
let agent_feishu = FeishuClient::new(
    config.feishu_app_id.clone(),
    config.feishu_app_secret.clone(),
    config.feishu_receive_id.clone(),
);

// Create AgentAdviceService
let agent_service = AgentAdviceService::new(
    config.agent_advice.clone(),
    llm_registry.clone(),
    tools,
    tdengine_client,
    agent_feishu,
);

// Add as notification handler
builder = builder.with_notification(Box::new(agent_service));
```

## Data Flow

```
1. Rule triggers alert
        │
        ▼
2. Alert sent via broadcast::channel
        │
        ├─────────────────┬─────────────────┐
        │                 │                 │
        ▼                 ▼                 ▼
3. Stdout Notif    Feishu Notif    AgentAdviceService
                                      │
        ┌─────────────────────────────┤
        │                             │
        ▼                             ▼
4.  Check frequency              Generate Advice
    limit                         │
                                  ▼
                            Call Agent
                                  │
                                  ▼
                            Agent calls tools
                            (AlertHistoryTool)
                                  │
                                  ▼
                            Query TDengine
                                  │
                                  ▼
                            Generate analysis
                                  │
                                  ▼
5.                          Send via Feishu
```

## Error Handling

| Error | Handling |
|-------|----------|
| LLM provider unavailable | Log warning, skip analysis |
| TDengine query fails | Continue with advice (no historical context) |
| Feishu send fails | Log error, do not retry |
| Agent max iterations | Return partial advice with error message |
| Rate limit exceeded | Skip silently (expected behavior) |

## Testing Strategy

### Unit Tests

1. **FrequencyLimiter** - test rate limiting logic
2. **FeishuClient** - test message formatting
3. **NotificationHandler impl** - test `send()` method

### Integration Tests

1. **AgentAdviceService e2e** - mock LLM + tools, verify flow
2. **ToolRegistry integration** - verify agent can call tools
3. **TDengine integration** - verify history queries work

### Manual Testing

1. Deploy to k8s with test config
2. Trigger test alerts
3. Verify Feishu messages received

## Migration Notes

- Existing `NotificationHandler` implementations unaffected
- `broadcast::channel` change is backward compatible (subscription pattern)
- `AgentAdviceService` is opt-in via `config.agent_advice.enabled`

## Out of Scope

- Agent training or fine-tuning
- Complex multi-alert correlation
- Historical data caching (future optimization)

## Success Criteria

- [ ] `AgentAdviceService` compiles without errors
- [ ] Implements `NotificationHandler` trait
- [ ] Receives alerts via broadcast channel
- [ ] Agent can call `AlertHistoryTool`
- [ ] Feishu messages sent successfully
- [ ] Rate limiting works correctly
- [ ] All existing tests still pass
