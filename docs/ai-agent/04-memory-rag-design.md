# AI Agent - Memory vs RAG 架构决策

**创建日期**: 2026-04-06  
**状态**: 架构决策中  
**作者**: vol-monitor team

---

## 1. 核心问题

ReAct Agent 需要访问**历史信息**和**领域知识**来做出更好的推理决策。有两种主流方案：

| 方案 | 核心思想 | 典型实现 |
|------|----------|----------|
| **Memory** | 记住对话历史和关键事件 | ConversationBuffer, VectorStore |
| **RAG** | 检索外部知识和文档 | 向量检索 + 上下文注入 |

---

## 2. vol-monitor 场景分析

### 2.1 业务需求

vol-monitor 系统的 AI Agent 需要访问的信息类型：

| 信息类型 | 特点 | 访问模式 |
|----------|------|----------|
| **当前告警上下文** | 结构化数据，实时性强 | 直接从事件获取 |
| **历史告警记录** | 时间序列数据，按 symbol/tenor 查询 | 结构化查询 |
| **IV 曲线/市场数据** | 实时数据，高频更新 | API 实时拉取 |
| **告警规则定义** | 静态配置，低频变更 | 配置读取 |
| **用户历史对话** | 对话历史，短期价值 | 会话内缓存 |
| **领域知识库** | 静态文档（期权知识、交易策略） | 语义检索 |

### 2.2 ReAct 模式下的信息需求

在 ReAct 循环中，Agent 需要：

```
┌─────────────────────────────────────────────────────────────┐
│                     ReAct Loop                              │
│                                                             │
│  ┌───────────┐      ┌───────────┐      ┌───────────┐       │
│  │ Reason    │ ───► │ Act       │ ───► │ Observe   │       │
│  │ (推理)    │      │ (工具调用) │      │ (观察)    │       │
│  └───────────┘      └───────────┘      └───────────┘       │
│         ▲                                      │            │
│         │                                      │            │
│         └──────────────────────────────────────┘            │
│                                                             │
│  需要注入的上下文：                                          │
│  - 当前告警信息（必须）                                      │
│  - 相关历史数据（通过工具获取）                               │
│  - 领域知识（按需检索）                                      │
└─────────────────────────────────────────────────────────────┘
```

**关键观察**: ReAct 模式本身已经通过 **Tool** 机制解决了大部分"记忆"需求。

---

## 3. 方案对比

### 3.1 Memory 方案

#### 3.1.1 Memory 类型

```rust
// vol-llm-agent/src/memory.rs

/// 对话 Memory Trait
#[async_trait]
pub trait Memory: Send + Sync {
    /// 添加消息到记忆
    async fn add_message(&mut self, role: MessageRole, content: String);
    
    /// 获取历史消息（用于构建对话上下文）
    async fn get_messages(&self) -> Vec<Message>;
    
    /// 清除记忆
    async fn clear(&mut self);
    
    /// 获取记忆摘要（可选）
    async fn get_summary(&self) -> Option<String>;
}
```

#### 3.1.2 Memory 实现方案

| 类型 | 说明 | 适用场景 | vol-monitor 需求 |
|------|------|----------|-----------------|
| **ConversationBuffer** | 简单存储所有历史消息 | 短对话 | ❌ 告警分析通常是单次任务 |
| **ConversationSummary** | 压缩历史为摘要 | 长对话 | ⚠️ 价值有限 |
| **VectorStore Memory** | 向量检索历史片段 | 复杂知识问答 | ❌ 告警场景不需要 |
| **Entity Memory** | 按实体（symbol）分组记忆 | 多实体跟踪 | ⚠️ 可选 |

#### 3.1.3 vol-monitor 的 Memory 需求评估

| 需求 | 是否需要 | 原因 |
|------|----------|------|
| 对话历史 | ❌ 不需要 | 告警分析是单次任务，不需要多轮对话 |
| 历史告警 | ✅ 需要 | 但这是**结构化数据**，不是 Memory 范畴 |
| 用户偏好 | ⚠️ 可选 | 如偏好的分析风格，可通过配置解决 |

**结论**: vol-monitor **不需要传统 LLM Memory**。

原因：
1. 告警分析是 **task-oriented** 而非 **chat-oriented**
2. 历史数据是结构化的，应该通过 **Tool 查询** 而非 Memory 检索
3. 每次告警都是独立事件，不需要跨会话记忆

---

### 3.2 RAG 方案

#### 3.2.1 RAG 架构

```
┌─────────────────────────────────────────────────────────────┐
│                    RAG Pipeline                             │
│                                                             │
│   User Query ──►  Embedding  ──►  Vector Search  ──► Docs  │
│                                                             │
│   Retrieved Docs ──► Context Injection ──► LLM Prompt      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

#### 3.2.2 vol-monitor 的 RAG 需求评估

| 知识类型 | 是否需要 RAG | 替代方案 |
|----------|-------------|----------|
| 期权交易知识 | ⚠️ 可选 | LLM 已有知识足够 |
| 历史告警模式 | ❌ 不需要 | Tool 查询结构化数据 |
| 监控规则文档 | ❌ 不需要 | 配置读取 |
| 市场分析框架 | ⚠️ 可选 | System Prompt 注入 |

**结论**: vol-monitor **不需要完整的 RAG 管道**。

原因：
1. 领域知识（期权交易）是通用知识，LLM 已经具备
2. 业务数据（告警、IV、市场）是结构化的，应该通过 Tool 访问
3. 配置/规则文档是确定性的，直接读取更高效

---

## 4. 推荐架构：Context + Tool 模式

### 4.1 设计原则

```
┌─────────────────────────────────────────────────────────────┐
│            vol-monitor Agent Architecture                   │
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  ToolContext                         │   │
│  │  - alert: Alert (当前告警)                           │   │
│  │  - portfolio: PortfolioState (持仓状态)             │   │
│  │  - metadata: HashMap (自定义元数据)                 │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │              ReAct Agent Loop                        │   │
│  │                                                      │   │
│  │  Reason ──► Act (Tool Call) ──► Observe             │   │
│  │    ▲                                      │         │   │
│  │    │◄─────────────────────────────────────┘         │   │
│  │                                                      │   │
│  │  Tools:                                              │   │
│  │  - alert_history: 查询历史告警                       │   │
│  │  - iv_curve: 获取 IV 曲线                            │   │
│  │  - market_data: 获取市场数据                         │   │
│  │  - rule_info: 查询规则定义                           │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
│  ✗ Memory: 不需要（对话是 task-oriented）                   │
│  ✗ RAG: 不需要（领域知识 LLM 已有，业务数据通过 Tool）        │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### 4.2 为什么不需要 Memory

1. **告警分析是单次任务** - 不是多轮对话场景
2. **历史数据是结构化的** - 应该通过 Tool 查询数据库，不是向量检索
3. **Trace 上下文已足够** - `TracedEvent` 携带了告警的完整上下文

### 4.3 为什么不需要 RAG

1. **领域知识 LLM 已有** - 期权交易知识是通用金融知识
2. **业务数据通过 Tool** - 告警/IV/市场数据是结构化查询
3. **配置文档直接读** - 规则定义是确定性配置

### 4.4 可选扩展：轻量级 Context 增强

虽然不需要完整的 Memory/RAG，但可以提供一些**轻量级的上下文增强**：

```rust
// vol-llm-agent/src/context.rs

/// 增强上下文 - 可选的领域特定信息
#[derive(Debug, Clone, Default)]
pub struct EnhancedContext {
    /// 最近 N 条相关告警（预加载，减少工具调用）
    pub recent_alerts: Vec<AlertSummary>,
    /// 标的当前状态快照
    pub symbol_snapshot: SymbolState,
    /// 市场状态（牛市/熊市/震荡）
    pub market_regime: Option<String>,
    /// 用户偏好（分析风格、详细程度）
    pub user_preferences: UserPreferences,
}

/// 告警摘要
#[derive(Debug, Clone)]
pub struct AlertSummary {
    pub alert_type: String,
    pub symbol: String,
    pub tenor: Tenor,
    pub iv: f64,
    pub timestamp: DateTime<Utc>,
    pub outcome: Option<String>, // 后续走势（如果有）
}

/// 标的状态
#[derive(Debug, Clone)]
pub struct SymbolState {
    pub symbol: String,
    pub current_price: f64,
    pub iv_rank: f64, // IV 历史百分位
    pub iv_percentile: f64,
    pub skew: f64, // 25Delta Skew
}

/// 用户偏好
#[derive(Debug, Clone, Default)]
pub struct UserPreferences {
    /// 分析详细程度 (1-5)
    pub detail_level: u8,
    /// 偏好输出语言
    pub language: String,
    /// 是否包含操作建议
    pub include_actions: bool,
    /// 风险偏好 (conservative/moderate/aggressive)
    pub risk_tolerance: String,
}

impl EnhancedContext {
    /// 从存储和配置构建增强上下文
    pub async fn build(
        alert: &Alert,
        storage: &dyn AlertStorage,
        config: &AgentConfig,
    ) -> Result<Self, ContextError> {
        // 预加载最近告警（减少第一次工具调用）
        let recent_alerts = storage
            .query_recent(&alert.symbol, 24) // 过去 24 小时
            .await?
            .into_iter()
            .map(|a| a.into())
            .collect();
        
        // 获取标的状态快照
        let symbol_snapshot = storage
            .get_symbol_state(&alert.symbol)
            .await?;
        
        Ok(Self {
            recent_alerts,
            symbol_snapshot,
            market_regime: None, // 可选
            user_preferences: config.preferences.clone(),
        })
    }
}
```

### 4.5 使用方式

```rust
// 方式 1：通过 ToolContext 注入
let context = ToolContext {
    alert: Some(alert.clone()),
    messages: vec![],
    metadata: HashMap::new(),
};

// 方式 2：通过 System Prompt 注入
let enhanced_context = EnhancedContext::build(&alert, storage, &config).await?;

let system_prompt = SystemPromptBuilder::new()
    .with_tools(&tools)
    .with_context(&enhanced_context) // 注入增强上下文
    .build();

let agent = AgentBuilder::new()
    .with_llm(llm)
    .with_tools(tools)
    .with_system_prompt(system_prompt)
    .build()?;
```

---

## 5. 架构决策

### 5.1 决策

| 方案 | 决策 | 原因 |
|------|------|------|
| **Memory (对话记忆)** | ❌ 不采用 | 告警分析是 task-oriented，不是 chat-oriented |
| **RAG (知识检索)** | ❌ 不采用 | 领域知识 LLM 已有，业务数据通过 Tool 访问 |
| **Enhanced Context** | ✅ 采用 | 轻量级上下文预加载，减少工具调用次数 |

### 5.2 架构原则

1. **Tool-first** - 所有数据访问通过 Tool 抽象
2. **Context-injection** - 关键上下文通过 System Prompt 注入
3. **No Memory** - 不维护对话历史，每次告警独立处理
4. **No RAG** - 不需要向量检索，结构化数据走 Tool

### 5.3 架构演进路径

```
Phase 1 (MVP): Context + Tool
  └── 通过 ToolContext 传递告警上下文
  └── 通过 Tool 查询历史数据

Phase 2 (可选): Enhanced Context
  └── 预加载最近告警，减少工具调用
  └── 注入用户偏好

Phase 3 (可选): Learning from History
  └── 记录 Agent 分析结果和后续走势
  └── 用于优化 System Prompt
```

---

## 6. 最终设计

### 6.1 更新后的 ReAct Agent

```rust
// crates/vol-llm-agent/src/agent.rs

/// ReAct Agent - vol-monitor 定制版
pub struct ReActAgent {
    llm: Box<dyn LLMClient>,
    tools: ToolRegistry,
    config: AgentConfig,
    // ✗ 不需要 Memory 字段
    // ✗ 不需要 VectorStore 字段
}

impl ReActAgent {
    pub async fn run(
        &self,
        user_input: &str,
        context: ToolContext,  // 唯一的信息来源
    ) -> Result<AgentResponse, AgentError> {
        // ToolContext 包含：
        // - alert: 当前告警
        // - messages: 对话历史（如果需要多轮）
        // - metadata: 自定义元数据
        
        // 不需要 Memory 检索
        // 不需要 RAG 检索
        // 所有数据通过 Tool 获取
    }
}
```

### 6.2 ToolContext 扩展

```rust
// crates/vol-llm-tool/src/context.rs

#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    /// 当前告警（核心上下文）
    pub alert: Option<vol_core::Alert>,
    
    /// 对话历史（可选，用于多轮对话）
    pub messages: Vec<vol_llm_core::Message>,
    
    /// 增强上下文（可选，预加载的数据）
    pub enhanced: Option<EnhancedContext>,
    
    /// 自定义元数据
    pub metadata: std::collections::HashMap<String, String>,
}

/// 增强上下文 - 可选的预加载数据
#[derive(Debug, Clone, Default)]
pub struct EnhancedContext {
    /// 最近告警摘要（减少首次工具调用）
    pub recent_alerts: Vec<AlertSummary>,
    /// 标的状态快照
    pub symbol_snapshot: SymbolState,
    /// 用户偏好
    pub user_preferences: UserPreferences,
}
```

---

## 7. 对比总结

| 维度 | Memory 方案 | RAG 方案 | Context + Tool (推荐) |
|------|-------------|----------|----------------------|
| **复杂度** | 中 | 高 | 低 |
| **维护成本** | 中（需要存储/清理） | 高（需要向量索引） | 低 |
| **响应速度** | 慢（检索开销） | 慢（检索 + 注入） | 快（直接 Tool） |
| **准确性** | 中（可能检索无关内容） | 中（依赖 embedding 质量） | 高（结构化查询） |
| **适用场景** | 多轮对话 | 知识问答 | Task-oriented 分析 |
| **vol-monitor 匹配度** | ❌ 低 | ❌ 低 | ✅ 高 |

---

## 8. 参考

- [LangChain Memory Types](https://python.langchain.com/docs/modules/memory/) - 各种 Memory 类型对比
- [RAG Best Practices](https://arxiv.org/abs/2312.10997) - RAG 实践指南
- [ReAct Paper](https://arxiv.org/abs/2210.03629) - ReAct 原始论文
