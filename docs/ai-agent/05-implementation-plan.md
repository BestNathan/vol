# AI Agent - 实现计划

**创建日期**: 2026-04-06  
**状态**: 已批准  
**作者**: vol-monitor team

---

## 1. 概述

基于已完成的设计文档，实现 vol-monitor 的 AI Agent 能力，包含 4 个新的 crates：

| Crate | 职责 | 依赖 |
|-------|------|------|
| `vol-llm-core` | 核心协议层 - LLM 交互抽象 | vol-core |
| `vol-llm-provider` | Provider 适配层 - Anthropic/OpenAI | vol-llm-core |
| `vol-llm-tool` | 工具层 - 工具定义和执行框架 | vol-llm-core |
| `vol-llm-agent` | Agent 层 - ReAct 工作流编排 | vol-llm-core, vol-llm-tool |

---

## 2. 设计文档参考

| 文档 | 说明 |
|------|------|
| [01-llm-client-architecture.md](ai-agent/01-llm-client-architecture.md) | LLM Client 架构设计 |
| [02-protocol-design.md](ai-agent/02-protocol-design.md) | 交互协议设计 |
| [03-agent-tool-design.md](ai-agent/03-agent-tool-design.md) | ReAct Agent & Tools 设计 |
| [04-memory-rag-design.md](ai-agent/04-memory-rag-design.md) | Memory vs RAG 架构决策 |

---

## 3. Phase 1: vol-llm-core（核心协议层）

### 3.1 目标

创建 `vol-llm-core` crate，定义 LLM 交互的核心抽象类型。

### 3.2 模块结构

```
crates/vol-llm-core/src/
├── lib.rs          # 模块导出
├── provider.rs     # LLMProvider enum
├── message.rs      # Message, MessageRole, MessageContent
├── tool.rs         # ToolDefinition, ToolCall, ToolChoice
├── model.rs        # ModelConfig, ModelInfo
├── conversation.rs # ConversationRequest, ConversationResponse
├── stream.rs       # 流式响应类型
├── client.rs       # LLMClient trait
└── error.rs        # LLMError
```

### 3.3 关键类型

#### provider.rs
```rust
pub enum LLMProvider {
    Anthropic,
    OpenAI,
}
```

#### message.rs
```rust
pub enum MessageRole { System, User, Assistant, Tool }

pub enum MessageContent {
    Text(String),
    MultiPart(Vec<ContentPart>),
}

pub struct Message {
    pub role: MessageRole,
    pub content: Option<MessageContent>,
    pub tool_calls: Option<Vec<ToolCall>>,
    pub tool_call_id: Option<String>,
}
```

#### tool.rs
```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
}

pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: String,
}

pub enum ToolChoice {
    Auto, Required, None, Specific { name: String },
}
```

#### model.rs
```rust
pub struct ModelConfig {
    pub max_tokens: Option<u32>,
    pub temperature: Option<f64>,
    pub top_p: Option<f64>,
    pub top_k: Option<u32>,
    pub frequency_penalty: Option<f64>,
    pub presence_penalty: Option<f64>,
    pub stop: Option<Vec<String>>,
    pub seed: Option<u64>,
    pub logprobs: Option<u32>,
}
```

#### conversation.rs
```rust
pub struct ConversationRequest {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub model_config: ModelConfig,
    pub tools: Option<Vec<ToolDefinition>>,
    pub tool_choice: Option<ToolChoice>,
    pub stream: bool,
}

pub struct ConversationResponse {
    pub message: Message,
    pub model: String,
    pub usage: TokenUsage,
    pub finish_reason: FinishReason,
    pub logprobs: Option<LogProbs>,
    pub raw: Option<serde_json::Value>,
}
```

#### client.rs
```rust
#[async_trait]
pub trait LLMClient: Send + Sync {
    fn provider(&self) -> LLMProvider;
    fn model(&self) -> &str;
    fn supported_params(&self) -> &[SupportedParam];
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse, LLMError>;
    async fn converse_stream(&self, request: ConversationRequest) -> Result<StreamReceiver, LLMError>;
}
```

#### error.rs
```rust
pub enum LLMError {
    Network(#[from] reqwest::Error),
    Api { status: u16, message: String },
    Auth(String),
    RateLimit { retry_after: Option<Duration> },
    Parse(String),
    Timeout(String),
    UnsupportedParam { param: String },
    ToolCall(String),
    ContentFiltered { reason: String },
}
```

### 3.4 交付物

- [ ] `crates/vol-llm-core/Cargo.toml`
- [ ] `src/lib.rs` - 模块导出
- [ ] `src/provider.rs`
- [ ] `src/message.rs`
- [ ] `src/tool.rs`
- [ ] `src/model.rs`
- [ ] `src/conversation.rs`
- [ ] `src/stream.rs`
- [ ] `src/client.rs`
- [ ] `src/error.rs`
- [ ] 单元测试覆盖核心类型

---

## 4. Phase 2: vol-llm-provider（Provider 适配层）

### 4.1 目标

实现 `AnthropicProvider` 和 `OpenAIProvider`，将统一的 `ConversationRequest` 转换为目标 API 格式。

### 4.2 模块结构

```
crates/vol-llm-provider/src/
├── lib.rs          # 导出 Provider
├── config.rs       # LLMConfig 配置结构
├── anthropic.rs    # AnthropicProvider 实现
├── openai.rs       # OpenAIProvider 实现
└── factory.rs      # create_provider 工厂函数
```

### 4.3 关键实现

#### config.rs
```rust
pub struct LLMConfig {
    pub provider: LLMProvider,
    pub model: String,
    pub api_key_env: String,
    pub endpoint: Option<String>,
}
```

#### anthropic.rs
- 实现 `LLMClient` trait
- 协议转换：`ConversationRequest` → Anthropic Messages API
- 处理 system message 分离（Anthropic 要求）
- 解析响应：Anthropic → `ConversationResponse`

#### openai.rs
- 实现 `LLMClient` trait
- 协议转换：`ConversationRequest` → OpenAI Chat Completions
- 处理 system message 作为第一条消息
- 解析响应：OpenAI → `ConversationResponse`

#### factory.rs
```rust
pub fn create_provider(config: &LLMConfig) -> Result<Box<dyn LLMClient>, LLMError>
pub fn load_provider(config_path: &str) -> Result<Box<dyn LLMClient>, LLMError>
```

### 4.4 交付物

- [ ] `crates/vol-llm-provider/Cargo.toml`
- [ ] `src/lib.rs`
- [ ] `src/config.rs`
- [ ] `src/anthropic.rs`
- [ ] `src/openai.rs`
- [ ] `src/factory.rs`
- [ ] 单元测试（Mock API 响应）
- [ ] 集成测试（需要真实 API Key，默认跳过）

---

## 5. Phase 3: vol-llm-tool（工具层）

### 5.1 目标

实现工具框架和 vol-monitor 内置工具。

### 5.2 模块结构

```
crates/vol-llm-tool/src/
├── lib.rs          # 模块导出
├── tool.rs         # Tool trait, ToolResult, ToolContext
├── registry.rs     # ToolRegistry
└── tools/
    ├── mod.rs
    ├── alert_history.rs
    ├── iv_curve.rs
    ├── market_data.rs
    └── rule_info.rs
```

### 5.3 核心 Trait

#### tool.rs
```rust
pub struct ToolResult {
    pub call_id: String,
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
}

pub struct ToolContext {
    pub alert: Option<vol_core::Alert>,
    pub messages: Vec<Message>,
    pub metadata: HashMap<String, String>,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<serde_json::Value>;
    async fn execute(&self, args: &str, context: &ToolContext) 
        -> Result<ToolResult, Box<dyn std::error::Error + Send>>;
}
```

#### registry.rs
```rust
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn register<T: Tool + 'static>(&mut self, tool: T);
    pub fn definitions(&self) -> Vec<ToolDefinition>;
    pub async fn execute(&self, call: &ToolCall, context: &ToolContext) 
        -> Result<ToolResult, String>;
}
```

### 5.4 内置工具

#### alert_history.rs
- 查询指定 symbol 的历史告警
- 参数：symbol, tenor, alert_type
- 返回：告警列表和统计信息

#### iv_curve.rs
- 获取 IV 曲线数据
- 参数：symbol, tenor
- 返回：IV 曲面数据

#### market_data.rs
- 获取市场数据
- 参数：symbol, data_type
- 返回：价格、涨跌幅等

#### rule_info.rs
- 查询告警规则
- 参数：alert_type
- 返回：规则定义和阈值

### 5.5 交付物

- [ ] `crates/vol-llm-tool/Cargo.toml`
- [ ] `src/lib.rs`
- [ ] `src/tool.rs`
- [ ] `src/registry.rs`
- [ ] `src/tools/mod.rs`
- [ ] `src/tools/alert_history.rs`
- [ ] `src/tools/iv_curve.rs`
- [ ] `src/tools/market_data.rs`
- [ ] `src/tools/rule_info.rs`
- [ ] 单元测试（Mock 工具执行）

---

## 6. Phase 4: vol-llm-agent（Agent 层）

### 6.1 目标

实现 ReAct Agent 工作流编排。

### 6.2 模块结构

```
crates/vol-llm-agent/src/
├── lib.rs          # 模块导出
├── agent.rs        # ReActAgent 核心
├── response.rs     # AgentResponse, AgentError
├── builder.rs      # AgentBuilder
├── prompt.rs       # 系统提示词模板
└── context.rs      # EnhancedContext (可选)
```

### 6.3 核心类型

#### agent.rs
```rust
pub struct ReActAgent {
    llm: Box<dyn LLMClient>,
    tools: ToolRegistry,
    config: AgentConfig,
}

impl ReActAgent {
    pub async fn run(&self, user_input: &str, context: ToolContext) 
        -> Result<AgentResponse, AgentError>;
}
```

#### response.rs
```rust
pub struct AgentResponse {
    pub content: String,
    pub reasoning: String,
    pub iterations: u32,
    pub tool_calls: Vec<ToolCall>,
}

pub enum AgentError {
    Llm(#[from] LLMError),
    ToolExecution { tool: String, error: String },
    MaxIterationsReached { max: u32 },
    InvalidToolResponse(String),
    Context(String),
}
```

#### builder.rs
```rust
pub struct AgentBuilder {
    llm: Option<Box<dyn LLMClient>>,
    tools: Vec<Box<dyn Tool>>,
    config: AgentConfig,
}
```

#### prompt.rs
```rust
pub fn default_system_prompt() -> &'static str;
pub struct SystemPromptBuilder { ... }
```

### 6.4 交付物

- [ ] `crates/vol-llm-agent/Cargo.toml`
- [ ] `src/lib.rs`
- [ ] `src/agent.rs`
- [ ] `src/response.rs`
- [ ] `src/builder.rs`
- [ ] `src/prompt.rs`
- [ ] `src/context.rs` (可选)
- [ ] 单元测试（Mock LLM + Mock Tools）
- [ ] 集成测试（端到端 ReAct 循环）

---

## 7. Phase 5: 集成与测试

### 7.1 集成到 workspace

- [ ] 更新根 `Cargo.toml` - 添加 4 个新 members
- [ ] 更新 workspace dependencies
- [ ] 验证编译通过

### 7.2 配置示例

- [ ] `config/llm.example.toml` - LLM 配置示例
- [ ] `.env.example` - 添加 API Key 环境变量

### 7.3 端到端测试

- [ ] 使用真实 API Key 测试 Anthropic
- [ ] 使用真实 API Key 测试 OpenAI
- [ ] 测试完整 ReAct 循环

### 7.4 文档更新

- [ ] 更新 `CLAUDE.md` - 添加 AI Agent 使用说明
- [ ] 更新 `README.md` - 添加 AI Agent 能力说明

---

## 8. 依赖关系图

```
vol-monitor (main binary)
    │
    ├── vol-llm-agent
    │   ├── vol-llm-tool
    │   │   └── vol-llm-core
    │   └── vol-llm-core
    │
    ├── vol-llm-provider
    │   └── vol-llm-core
    │
    └── vol-core (existing)
```

---

## 9. 测试策略

| Phase | 单元测试 | 集成测试 | 端到端测试 |
|-------|----------|----------|------------|
| Phase 1 (core) | ✅ 所有类型 | - | - |
| Phase 2 (provider) | ✅ Mock API | ⚠️ 真实 API (可选) | - |
| Phase 3 (tool) | ✅ Mock 执行 | ⚠️ 连接存储层 | - |
| Phase 4 (agent) | ✅ Mock LLM+Tools | ⚠️ Mock Provider | ✅ 真实 Provider |
| Phase 5 (integration) | - | ✅ workspace | ✅ 完整流程 |

---

## 10. 成功标准

- [ ] 所有 crate 编译通过 (`cargo check --workspace`)
- [ ] 单元测试通过 (`cargo test --workspace`)
- [ ] 集成测试通过（需要设置 API Key）
- [ ] 可以通过 AgentBuilder 创建并运行 ReAct Agent
- [ ] 文档完整
