# AI Agent - ReAct Agent & Tools 设计

**创建日期**: 2026-04-06  
**状态**: 设计中  
**作者**: vol-monitor team

---

## 1. 概述

### 1.1 设计目标

基于 vol-monitor 系统的告警数据，设计一个具备 **ReAct（Reason + Act）** 能力的 AI Agent，能够：

1. **自主推理** - 分析告警上下文，生成推理链
2. **工具调用** - 根据需要调用工具获取额外信息
3. **多轮迭代** - 支持多轮 Reason-Act-Observation 循环
4. **最终响应** - 综合所有信息生成最终分析结论

### 1.2 包结构

```
crates/
├── vol-llm-core/        # 核心协议层 - LLM 交互抽象
├── vol-llm-provider/    # Provider 适配层 - Anthropic/OpenAI 实现
├── vol-llm-tool/        # 工具层 - 工具定义、执行框架
└── vol-llm-agent/       # Agent 层 - ReAct 工作流编排
```

### 1.3 包依赖关系

```
vol-llm-agent
    ├── vol-llm-tool
    ├── vol-llm-core
    └── vol-core         # 使用 Alert 等业务类型

vol-llm-tool
    └── vol-llm-core     # 使用 ToolDefinition/ToolCall

vol-llm-provider
    └── vol-llm-core     # 实现 LLMClient trait

vol-llm-core
    └── vol-core         # 基础依赖
```

---

## 2. vol-llm-core - 核心协议层

### 2.1 职责

定义 LLM 交互的核心抽象，不包含具体 Provider 实现。

### 2.2 核心类型

```rust
// crates/vol-llm-core/src/lib.rs

pub mod message;      // Message, MessageRole, MessageContent
pub mod conversation; // ConversationRequest, ConversationResponse
pub mod model;        // ModelConfig, ModelInfo
pub mod tool;         // ToolDefinition, ToolCall, ToolChoice
pub mod stream;       // 流式响应相关类型
pub mod client;       // LLMClient trait
pub mod error;        // LLMError
pub mod provider;     // LLMProvider enum

pub use message::*;
pub use conversation::*;
pub use model::*;
pub use tool::*;
pub use stream::*;
pub use client::*;
pub use error::*;
pub use provider::*;
```

### 2.3 导出类型总览

| 类型 | 用途 |
|------|------|
| `Message`, `MessageRole` | 消息角色和内容 |
| `ConversationRequest` | 对话请求（带 builder 模式） |
| `ConversationResponse` | 对话响应 |
| `ToolDefinition`, `ToolCall` | 工具定义和调用 |
| `LLMClient` | 统一 Client trait |
| `LLMProvider` | Provider 枚举 |
| `LLMError` | 错误类型 |

---

## 3. vol-llm-provider - Provider 适配层

### 3.1 职责

实现具体 LLM Provider 的协议转换，将统一的 `ConversationRequest` 转换为目标 API 格式。

### 3.2 模块结构

```rust
// crates/vol-llm-provider/src/lib.rs

pub mod anthropic;    // AnthropicProvider
pub mod openai;       // OpenAIProvider
pub mod config;       // LLMConfig 配置

pub use anthropic::AnthropicProvider;
pub use openai::OpenAIProvider;
pub use config::LLMConfig;
```

### 3.3 Provider 工厂模式

```rust
// crates/vol-llm-provider/src/factory.rs

use vol_llm_core::{LLMClient, LLMProvider, LLMError};
use crate::{AnthropicProvider, OpenAIProvider, LLMConfig};

/// 创建 Provider 实例
pub fn create_provider(config: &LLMConfig) -> Result<Box<dyn LLMClient>, LLMError> {
    match config.provider {
        LLMProvider::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        LLMProvider::OpenAI => Ok(Box::new(OpenAIProvider::new(config)?)),
    }
}

/// 从配置加载 Provider（支持环境变量覆盖）
pub fn load_provider(config_path: &str) -> Result<Box<dyn LLMClient>, LLMError> {
    let config = LLMConfig::load(config_path)?;
    create_provider(&config)
}
```

---

## 4. vol-llm-tool - 工具层

### 4.1 职责

提供工具定义、执行框架和内置工具实现。

### 4.2 核心 Trait

```rust
// crates/vol-llm-tool/src/tool.rs

use vol_llm_core::{ToolDefinition, ToolCall, ConversationResponse};

/// 工具执行结果
#[derive(Debug, Clone)]
pub struct ToolResult {
    /// 关联的工具调用 ID
    pub call_id: String,
    /// 工具执行是否成功
    pub success: bool,
    /// 工具输出内容（文本或结构化数据）
    pub content: String,
    /// 错误信息（如果失败）
    pub error: Option<String>,
    /// 结构化数据（可选，用于复杂工具）
    pub data: Option<serde_json::Value>,
}

/// 工具执行上下文
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// 当前告警信息
    pub alert: Option<vol_core::Alert>,
    /// 历史对话消息
    pub messages: Vec<vol_llm_core::Message>,
    /// 自定义元数据
    pub metadata: std::collections::HashMap<String, String>,
}

/// 工具 Trait - 所有工具必须实现
#[async_trait]
pub trait Tool: Send + Sync {
    /// 工具名称（唯一标识）
    fn name(&self) -> &str;
    
    /// 工具描述（用于 LLM 理解工具用途）
    fn description(&self) -> &str;
    
    /// 工具参数 schema（JSON Schema）
    fn parameters(&self) -> Option<serde_json::Value>;
    
    /// 转换为 ToolDefinition
    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: Some(self.description().to_string()),
            parameters: self.parameters(),
        }
    }
    
    /// 执行工具
    async fn execute(&self, args: &str, context: &ToolContext) -> Result<ToolResult, Box<dyn std::error::Error + Send>>;
}
```

### 4.3 工具注册表

```rust
// crates/vol-llm-tool/src/registry.rs

use std::collections::HashMap;
use crate::tool::{Tool, ToolResult, ToolContext};

/// 工具注册表
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }
    
    /// 注册工具
    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }
    
    /// 获取工具定义列表（用于传给 LLM）
    pub fn definitions(&self) -> Vec<vol_llm_core::ToolDefinition> {
        self.tools.values().map(|t| t.to_definition()).collect()
    }
    
    /// 执行工具
    pub async fn execute(
        &self,
        call: &vol_llm_core::ToolCall,
        context: &ToolContext,
    ) -> Result<ToolResult, String> {
        let tool = self.tools.get(&call.name)
            .ok_or_else(|| format!("Unknown tool: {}", call.name))?;
        
        tool.execute(&call.arguments, context)
            .await
            .map_err(|e| e.to_string())
    }
    
    /// 获取工具名称列表
    pub fn tool_names(&self) -> Vec<&str> {
        self.tools.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

### 4.4 内置工具

#### 4.4.1 告警历史查询工具

```rust
// crates/vol-llm-tool/src/tools/alert_history.rs

use vol_core::{Alert, Tenor};

/// 告警历史查询工具
pub struct AlertHistoryTool {
    /// 查询窗口（小时）
    window_hours: u32,
}

impl AlertHistoryTool {
    pub fn new(window_hours: u32) -> Self {
        Self { window_hours }
    }
}

#[async_trait]
impl Tool for AlertHistoryTool {
    fn name(&self) -> &str {
        "alert_history"
    }
    
    fn description(&self) -> &str {
        "查询指定 symbol 的历史告警记录，用于分析告警频率和模式"
    }
    
    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "标的符号，如 'BTC', 'ETH'"
                },
                "tenor": {
                    "type": "string",
                    "enum": ["short", "medium", "long"],
                    "description": "期限类型"
                },
                "alert_type": {
                    "type": "string",
                    "description": "告警类型（可选）"
                }
            },
            "required": ["symbol"]
        }))
    }
    
    async fn execute(&self, args: &str, _context: &ToolContext) -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        #[derive(serde::Deserialize)]
        struct Args {
            symbol: String,
            tenor: Option<String>,
            alert_type: Option<String>,
        }
        
        let args: Args = serde_json::from_str(args)
            .map_err(|e| format!("Invalid arguments: {}", e))?;
        
        // TODO: 从存储层查询历史告警
        // let alerts = storage.query_alerts(&args.symbol, self.window_hours).await?;
        
        let result = ToolResult {
            call_id: String::new(), // 由调用方填充
            success: true,
            content: format!("查询到 {} 条历史告警", 0),
            error: None,
            data: None,
        };
        
        Ok(result)
    }
}
```

#### 4.4.2 IV 曲线查询工具

```rust
// crates/vol-llm-tool/src/tools/iv_curve.rs

/// IV 曲线查询工具
pub struct IvCurveTool;

#[async_trait]
impl Tool for IvCurveTool {
    fn name(&self) -> &str {
        "iv_curve"
    }
    
    fn description(&self) -> &str {
        "获取标的的隐含波动率曲面数据，包括不同行权价和期限的 IV"
    }
    
    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "标的符号"
                },
                "tenor": {
                    "type": "string",
                    "enum": ["short", "medium", "long"],
                    "description": "期限"
                }
            },
            "required": ["symbol"]
        }))
    }
    
    async fn execute(&self, args: &str, context: &ToolContext) -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        // 实现 IV 曲线查询逻辑
        todo!()
    }
}
```

#### 4.4.3 市场数据查询工具

```rust
// crates/vol-llm-tool/src/tools/market_data.rs

/// 市场数据查询工具
pub struct MarketDataTool;

#[async_trait]
impl Tool for MarketDataTool {
    fn name(&self) -> &str {
        "market_data"
    }
    
    fn description(&self) -> &str {
        "获取实时市场数据，包括价格、涨跌幅、成交量等"
    }
    
    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "标的符号"
                },
                "data_type": {
                    "type": "string",
                    "enum": ["price", "volume", "funding_rate", "open_interest"],
                    "description": "数据类型"
                }
            },
            "required": ["symbol"]
        }))
    }
    
    async fn execute(&self, args: &str, _context: &ToolContext) -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        todo!()
    }
}
```

#### 4.4.4 告警规则查询工具

```rust
// crates/vol-llm-tool/src/tools/rule_info.rs

/// 告警规则查询工具
pub struct RuleInfoTool;

#[async_trait]
impl Tool for RuleInfoTool {
    fn name(&self) -> &str {
        "rule_info"
    }
    
    fn description(&self) -> &str {
        "查询告警规则的详细信息，包括触发条件和阈值"
    }
    
    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "alert_type": {
                    "type": "string",
                    "description": "告警类型，如 'absolute_iv', 'rate_change'"
                }
            },
            "required": ["alert_type"]
        }))
    }
    
    async fn execute(&self, args: &str, _context: &ToolContext) -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        todo!()
    }
}
```

### 4.5 工具宏（可选扩展）

```rust
// crates/vol-llm-tool/src/macros.rs

/// 快速定义工具的宏
#[macro_export]
macro_rules! define_tool {
    (
        name: $name:expr,
        desc: $desc:expr,
        params: $params:tt,
        exec: |$args:ident, $ctx:ident| $body:expr
    ) => {
        struct $name;
        
        #[async_trait::async_trait]
        impl vol_llm_tool::Tool for $name {
            fn name(&self) -> &str { $name }
            fn description(&self) -> &str { $desc }
            fn parameters(&self) -> Option<serde_json::Value> {
                Some(serde_json::json! $params)
            }
            async fn execute(
                &self,
                $args: &str,
                $ctx: &vol_llm_tool::ToolContext,
            ) -> Result<vol_llm_tool::ToolResult, Box<dyn std::error::Error + Send>> {
                $body
            }
        }
    };
}
```

---

## 5. vol-llm-agent - ReAct Agent 层

### 5.1 ReAct 模式概述

ReAct（**Re**ason + **Act**）是一种让 LLM 交替进行**推理**和**行动**的范式：

```
┌─────────────────────────────────────────────────────────────┐
│                    ReAct 循环                                │
│                                                             │
│   Input → Reason → Act → Observation → Reason → Act → ...  │
│                              │                              │
│                              ▼                              │
│                       最终响应                               │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

**每个循环包含**:

1. **Reason（推理）** - LLM 分析当前状态，决定下一步行动
2. **Act（行动）** - 调用工具获取信息
3. **Observation（观察）** - 将工具结果反馈给 LLM

### 5.2 Agent 核心类型

```rust
// crates/vol-llm-agent/src/agent.rs

use vol_llm_core::{
    LLMClient, Message, MessageRole, ConversationRequest,
    ToolDefinition, ToolCall, ToolChoice,
};
use vol_llm_tool::{ToolRegistry, ToolResult, ToolContext};

/// ReAct Agent 状态
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentState {
    /// 初始状态
    Init,
    /// 正在推理
    Reasoning,
    /// 正在执行工具
    ExecutingTool,
    /// 等待观察结果
    AwaitingObservation,
    /// 完成
    Completed,
    /// 错误
    Error(String),
}

/// 单步 ReAct 循环的结果
#[derive(Debug, Clone)]
pub enum ReActOutcome {
    /// LLM 决定调用工具
    ToolCall {
        calls: Vec<ToolCall>,
        reasoning: String,
    },
    /// LLM 生成最终响应
    FinalResponse {
        content: String,
        reasoning: String,
    },
}

/// Agent 配置
#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// 最大 ReAct 迭代次数
    pub max_iterations: u32,
    /// 系统提示词模板
    pub system_prompt: String,
    /// 是否启用详细日志
    pub verbose: bool,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            system_prompt: default_system_prompt().to_string(),
            verbose: false,
        }
    }
}

/// ReAct Agent
pub struct ReActAgent {
    /// LLM 客户端
    llm: Box<dyn LLMClient>,
    /// 工具注册表
    tools: ToolRegistry,
    /// Agent 配置
    config: AgentConfig,
}

impl ReActAgent {
    /// 创建新 Agent
    pub fn new(llm: Box<dyn LLMClient>, tools: ToolRegistry, config: AgentConfig) -> Self {
        Self { llm, tools, config }
    }
    
    /// 运行 ReAct 循环
    pub async fn run(
        &self,
        user_input: &str,
        context: ToolContext,
    ) -> Result<AgentResponse, AgentError> {
        let mut messages = Vec::new();
        let mut iteration = 0;
        
        // 初始化对话
        messages.push(Message::system(self.config.system_prompt.clone()));
        messages.push(Message::user(user_input));
        
        loop {
            iteration += 1;
            
            if iteration > self.config.max_iterations {
                return Err(AgentError::MaxIterationsReached {
                    max: self.config.max_iterations,
                });
            }
            
            // Step 1: Reason - 调用 LLM 获取下一步行动
            let outcome = self.reason(&messages, &context).await?;
            
            match outcome {
                ReActOutcome::ToolCall { calls, reasoning } => {
                    if self.config.verbose {
                        tracing::info!("Iteration {}: {}", iteration, reasoning);
                        tracing::info!("Tool calls: {:?}", calls);
                    }
                    
                    // Step 2: Act - 执行工具
                    let observations = self.execute_tools(&calls, &context).await?;
                    
                    if self.config.verbose {
                        tracing::info!("Observations: {:?}", observations);
                    }
                    
                    // Step 3: Observation - 将结果加入对话
                    messages.push(Message::assistant_with_tools(
                        reasoning,
                        calls.clone(),
                    ));
                    
                    for obs in observations {
                        messages.push(Message::tool(obs.content, obs.call_id));
                    }
                    
                    // 继续下一轮循环
                }
                ReActOutcome::FinalResponse { content, reasoning } => {
                    if self.config.verbose {
                        tracing::info!("Final response: {}", reasoning);
                    }
                    
                    return Ok(AgentResponse {
                        content,
                        reasoning,
                        iterations: iteration,
                        tool_calls: vec![],
                    });
                }
            }
        }
    }
    
    /// Reason 阶段 - 调用 LLM 决定下一步
    async fn reason(
        &self,
        messages: &[Message],
        _context: &ToolContext,
    ) -> Result<ReActOutcome, AgentError> {
        let tools = self.tools.definitions();
        
        let request = ConversationRequest::with_history(
            None,
            messages.to_vec(),
        )
        .with_tools(tools)
        .with_tool_choice(ToolChoice::Auto);
        
        let response = self.llm.converse(request).await?;
        
        // 判断 LLM 是否调用了工具
        if let Some(tool_calls) = response.message.tool_calls {
            if !tool_calls.is_empty() {
                return Ok(ReActOutcome::ToolCall {
                    calls: tool_calls,
                    reasoning: response.message.content
                        .unwrap_or_default()
                        .as_str()
                        .to_string(),
                });
            }
        }
        
        // 没有工具调用，说明是最终响应
        Ok(ReActOutcome::FinalResponse {
            content: response.message.content
                .unwrap_or_default()
                .as_str()
                .to_string(),
            reasoning: String::new(),
        })
    }
    
    /// Act 阶段 - 执行工具调用
    async fn execute_tools(
        &self,
        calls: &[ToolCall],
        context: &ToolContext,
    ) -> Result<Vec<ToolResult>, AgentError> {
        let mut results = Vec::new();
        
        for call in calls {
            let result = self.tools.execute(call, context).await
                .map_err(|e| AgentError::ToolExecution {
                    tool: call.name.clone(),
                    error: e,
                })?;
            
            results.push(ToolResult {
                call_id: call.id.clone(),
                ..result
            });
        }
        
        Ok(results)
    }
}
```

### 5.3 Agent 响应类型

```rust
// crates/vol-llm-agent/src/response.rs

use vol_llm_core::ToolCall;

/// Agent 响应
#[derive(Debug, Clone)]
pub struct AgentResponse {
    /// 最终响应内容
    pub content: String,
    /// 推理过程（可选）
    pub reasoning: String,
    /// 使用的迭代次数
    pub iterations: u32,
    /// 调用的工具列表
    pub tool_calls: Vec<ToolCall>,
}

/// Agent 错误
#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(#[from] vol_llm_core::LLMError),
    
    #[error("Tool execution failed: {tool}: {error}")]
    ToolExecution {
        tool: String,
        error: String,
    },
    
    #[error("Max iterations ({max}) reached without final response")]
    MaxIterationsReached { max: u32 },
    
    #[error("Invalid tool response: {0}")]
    InvalidToolResponse(String),
    
    #[error("Context error: {0}")]
    Context(String),
}
```

### 5.4 系统提示词模板

```rust
// crates/vol-llm-agent/src/prompt.rs

/// 默认系统提示词
pub fn default_system_prompt() -> &'static str {
    r#"你是一个专业的加密货币期权交易分析助手。

你的任务是分析监控系统的告警，为用户提供深入的市场洞察。

## 可用工具

你可以使用以下工具获取额外信息：

- `alert_history(symbol, tenor?)`: 查询历史告警
- `iv_curve(symbol, tenor?)`: 获取 IV 曲线数据
- `market_data(symbol, data_type?)`: 获取市场数据
- `rule_info(alert_type)`: 查询告警规则

## 工作流程

1. **分析告警** - 理解告警的类型、标的、期限
2. **决定行动** - 判断是否需要调用工具获取更多信息
3. **综合结论** - 基于所有信息给出分析结论

## 输出格式

当你需要调用工具时，请使用工具调用格式。
当你有足够信息时，直接给出最终分析结论。

## 注意事项

- 只调用必要的工具
- 如果一次工具调用不足以得出结论，可以进行多轮查询
- 最终结论应该清晰、可操作"#
}

/// 带上下文的系统提示词构建器
pub struct SystemPromptBuilder {
    available_tools: String,
    custom_instructions: Option<String>,
}

impl SystemPromptBuilder {
    pub fn new() -> Self {
        Self {
            available_tools: String::new(),
            custom_instructions: None,
        }
    }
    
    pub fn with_tools(mut self, tools: &[vol_llm_core::ToolDefinition]) -> Self {
        let tools_desc = tools.iter()
            .map(|t| format!("- `{}`: {}", t.name, t.description.as_deref().unwrap_or("无描述")))
            .collect::<Vec<_>>()
            .join("\n");
        
        self.available_tools = tools_desc;
        self
    }
    
    pub fn with_instructions(mut self, instructions: &str) -> Self {
        self.custom_instructions = Some(instructions.to_string());
        self
    }
    
    pub fn build(self) -> String {
        let base = default_system_prompt();
        
        let mut prompt = base.to_string();
        
        if !self.available_tools.is_empty() {
            prompt = prompt.replace(
                "## 可用工具\n\n你可以使用以下工具获取额外信息：",
                &format!("## 可用工具\n\n{}", self.available_tools),
            );
        }
        
        if let Some(instructions) = self.custom_instructions {
            prompt.push_str(&format!("\n\n## 额外指示\n\n{}", instructions));
        }
        
        prompt
    }
}

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

### 5.5 Agent Builder

```rust
// crates/vol-llm-agent/src/builder.rs

use vol_llm_core::LLMClient;
use vol_llm_tool::{Tool, ToolRegistry};
use crate::{ReActAgent, AgentConfig};

/// Agent 构建器
pub struct AgentBuilder {
    llm: Option<Box<dyn LLMClient>>,
    tools: Vec<Box<dyn Tool>>,
    config: AgentConfig,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: Vec::new(),
            config: AgentConfig::default(),
        }
    }
    
    pub fn with_llm(mut self, llm: Box<dyn LLMClient>) -> Self {
        self.llm = Some(llm);
        self
    }
    
    pub fn with_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
        self
    }
    
    pub fn with_tools<I>(mut self, tools: I) -> Self
    where
        I: IntoIterator<Item = Box<dyn Tool>>,
    {
        self.tools.extend(tools);
        self
    }
    
    pub fn with_config(mut self, config: AgentConfig) -> Self {
        self.config = config;
        self
    }
    
    pub fn with_max_iterations(mut self, max: u32) -> Self {
        self.config.max_iterations = max;
        self
    }
    
    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.config.system_prompt = prompt;
        self
    }
    
    pub fn with_verbose(mut self, verbose: bool) -> Self {
        self.config.verbose = verbose;
        self
    }
    
    pub fn build(self) -> Result<ReActAgent, AgentBuilderError> {
        let llm = self.llm.ok_or(AgentBuilderError::MissingLlm)?;
        
        let mut registry = ToolRegistry::new();
        for tool in self.tools {
            registry.register_tool(tool);
        }
        
        Ok(ReActAgent::new(llm, registry, self.config))
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentBuilderError {
    #[error("LLM client is required")]
    MissingLlm,
}
```

### 5.6 使用示例

```rust
// 示例：使用 ReAct Agent 分析告警

use vol_llm_agent::{ReActAgent, AgentBuilder, AgentConfig, ToolContext};
use vol_llm_provider::{create_provider, LLMConfig};
use vol_llm_tool::tools::{AlertHistoryTool, IvCurveTool, MarketDataTool, RuleInfoTool};
use vol_core::Alert;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. 创建 LLM Provider
    let config = LLMConfig::load("config/llm.toml")?;
    let llm = create_provider(&config)?;
    
    // 2. 创建工具集
    let agent = AgentBuilder::new()
        .with_llm(llm)
        .with_tool(AlertHistoryTool::new(24))  // 24 小时历史
        .with_tool(IvCurveTool)
        .with_tool(MarketDataTool)
        .with_tool(RuleInfoTool)
        .with_max_iterations(5)
        .with_verbose(true)
        .build()?;
    
    // 3. 构建上下文
    let context = ToolContext {
        alert: Some(alert),  // 当前告警
        messages: vec![],
        metadata: std::collections::HashMap::new(),
    };
    
    // 4. 运行 Agent
    let user_input = "分析这个 ETH IV 告警，给出操作建议";
    let response = agent.run(user_input, context).await?;
    
    println!("分析结论：{}", response.content);
    println!("迭代次数：{}", response.iterations);
    
    Ok(())
}
```

### 5.7 流式 Agent（扩展）

```rust
// crates/vol-llm-agent/src/stream.rs

/// 流式 Agent 事件
#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    /// 推理开始
    ReasoningStart { iteration: u32 },
    /// 推理内容（流式）
    ReasoningDelta { delta: String },
    /// 工具调用开始
    ToolCallStart { call_id: String, name: String },
    /// 工具调用完成
    ToolCallComplete { call_id: String, result: String },
    /// 最终响应开始
    ResponseStart,
    /// 最终响应内容（流式）
    ResponseDelta { delta: String },
    /// Agent 完成
    Complete { iterations: u32 },
    /// 错误
    Error { error: String },
}

/// 流式 Agent
pub struct StreamingAgent {
    inner: ReActAgent,
}

impl StreamingAgent {
    /// 运行流式 ReAct 循环
    pub async fn run_stream(
        &self,
        user_input: &str,
        context: ToolContext,
    ) -> Result<impl Stream<Item = AgentStreamEvent>, AgentError> {
        // 实现流式版本
        todo!()
    }
}
```

---

## 6. 配置示例

### 6.1 LLM 配置

```toml
# config/llm.toml

[llm]
provider = "anthropic"
model = "claude-sonnet-4-20251001"
api_key_env = "ANTHROPIC_API_KEY"

[agent]
max_iterations = 5
verbose = true
system_prompt = """
你是一个专业的加密货币期权交易分析助手...
"""

[tools.alert_history]
window_hours = 24

[tools.iv_curve]
enabled = true

[tools.market_data]
enabled = true
```

### 6.2 环境变量

```bash
# .env

ANTHROPIC_API_KEY=sk-ant-xxx
OPENAI_API_KEY=sk-xxx
```

---

## 7. 错误处理

### 7.1 错误类型层次

```
AgentError
├── Llm(LLMError)
│   ├── Network
│   ├── Api { status, message }
│   ├── Auth
│   ├── RateLimit
│   └── Parse
├── ToolExecution { tool, error }
├── MaxIterationsReached { max }
├── InvalidToolResponse
└── Context
```

### 7.2 重试策略

```rust
// crates/vol-llm-agent/src/retry.rs

use vol_llm_core::LLMError;

/// LLM 请求重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
}

/// 判断错误是否可重试
pub fn is_retryable(error: &LLMError) -> bool {
    match error {
        LLMError::Network(_) => true,
        LLMError::RateLimit { .. } => true,
        LLMError::Api { status, .. } => *status >= 500,
        _ => false,
    }
}

/// 带重试的 LLM 调用
pub async fn converse_with_retry(
    llm: &dyn LLMClient,
    request: ConversationRequest,
    config: &RetryConfig,
) -> Result<ConversationResponse, LLMError> {
    // 实现指数退避重试
    todo!()
}
```

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{Message, ConversationResponse, TokenUsage, FinishReason};
    
    // Mock LLM Client
    struct MockLlm {
        responses: Vec<ConversationResponse>,
    }
    
    #[async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        fn supported_params(&self) -> &[SupportedParam] { &[] }
        
        async fn converse(&self, _req: ConversationRequest) -> Result<ConversationResponse, LLMError> {
            Ok(self.responses[0].clone())
        }
        
        async fn converse_stream(&self, _req: ConversationRequest) -> Result<StreamReceiver, LLMError> {
            unimplemented!()
        }
    }
    
    #[tokio::test]
    async fn test_agent_single_tool_call() {
        let mock_llm = MockLlm {
            responses: vec![
                // 第一次调用：返回工具调用
                ConversationResponse {
                    message: Message::assistant_with_tools(
                        "查询历史告警",
                        vec![ToolCall {
                            id: "call_1".to_string(),
                            name: "alert_history".to_string(),
                            arguments: r#"{"symbol":"BTC"}"#.to_string(),
                        }],
                    ),
                    model: "mock".to_string(),
                    usage: TokenUsage::default(),
                    finish_reason: FinishReason::ToolCalls,
                    logprobs: None,
                    raw: None,
                },
                // 第二次调用：返回最终响应
                ConversationResponse {
                    message: Message::assistant("基于历史数据..."),
                    model: "mock".to_string(),
                    usage: TokenUsage::default(),
                    finish_reason: FinishReason::Stop,
                    logprobs: None,
                    raw: None,
                },
            ],
        };
        
        let agent = ReActAgent::new(
            Box::new(mock_llm),
            ToolRegistry::new(),
            AgentConfig::default(),
        );
        
        let response = agent.run("分析 BTC 告警", ToolContext::default()).await;
        assert!(response.is_ok());
    }
}
```

### 8.2 集成测试

```rust
// tests/agent_integration.rs

#[tokio::test]
#[ignore] // 需要真实 API Key
async fn test_agent_with_real_llm() {
    let config = LLMConfig::load("config/llm.test.toml").unwrap();
    let llm = create_provider(&config).unwrap();
    
    let agent = AgentBuilder::new()
        .with_llm(llm)
        .with_tool(AlertHistoryTool::new(24))
        .build()
        .unwrap();
    
    let response = agent.run(
        "分析 ETH IV 告警",
        ToolContext::default(),
    ).await;
    
    assert!(response.is_ok());
    let resp = response.unwrap();
    assert!(!resp.content.is_empty());
}
```

---

## 9. 参考

- [ReAct Paper](https://arxiv.org/abs/2210.03629) - Reasoning + Acting in Language Models
- [Anthropic Tool Use](https://docs.anthropic.com/claude/docs/tool-use)
- [OpenAI Function Calling](https://platform.openai.com/docs/guides/function-calling)

## 10. Wiki

- [[react-pattern]]: ReAct 循环的实现模式
- [[agent-plugin-system]]: Agent 插件系统架构
- [[tool-registry]]: 工具注册和执行框架
- [[vol-llm-agent-crate]]: ReAct Agent 核心 crate
