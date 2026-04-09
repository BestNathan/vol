# ReAct Agent 源码

**日期：** 2026-04-09
**模块路径：** crates/vol-llm-agent/src/react/

---

## 模块结构

```
react/
├── mod.rs           # 模块入口和导出
├── agent.rs         # ReActAgent 核心实现
├── run_context.rs   # 统一运行上下文
├── plugin.rs        # 插件系统 trait
├── plugin_stream.rs # 插件流包装器
├── builder.rs       # Agent 构建器
├── stream.rs        # 流事件定义
├── response.rs      # 响应和错误类型
├── prompt.rs        # 系统提示模板
└── hitl.rs          # Human-in-the-Loop 支持
```

---

## mod.rs - 模块入口

```rust
//! ReAct Agent module.
//!
//! Provides `ReActAgent` for reasoning and acting with tool integration.

pub mod agent;
pub mod builder;
pub mod response;
pub mod stream;
pub mod prompt;
pub mod plugin;
pub mod plugin_stream;
pub mod hitl;
pub mod run_context;

pub use agent::{ReActAgent, AgentConfig};
pub use builder::AgentBuilder;
pub use response::{AgentResponse, AgentError};
pub use stream::{AgentStreamEvent, AgentStreamReceiver};
pub use prompt::{default_system_prompt, SystemPromptBuilder};
pub use plugin::{AgentPlugin, PluginAction, PluginRegistry};
pub use plugin_stream::{PluginStream, create_shortcircuit_stream, create_skip_stream};
pub use run_context::RunContext;
pub use hitl::{ApprovalChannel, ApprovalRequest, ApprovalResponse, ApprovalType, HitlConfig, ApprovalTrigger, TimeoutBehavior};
```

---

## agent.rs - ReActAgent 核心

```rust
//! ReAct Agent implementation.

use std::sync::Arc;
use tokio::sync::mpsc;
use vol_llm_core::{LLMClient, Message, ConversationRequest, ToolChoice, StreamEventData, StreamReceiver};
use vol_llm_tool::ToolContext;
use tracing::{info, debug};
use super::{
    AgentResponse, AgentStreamEvent, AgentStreamReceiver, PluginRegistry, RunContext,
    PluginStream, PluginAction, create_shortcircuit_stream, create_skip_stream,
};
use crate::session::{Session, SessionMessage};

/// Agent configuration
#[derive(Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,
    pub system_prompt: String,
    pub verbose: bool,
    pub plugin_registry: PluginRegistry,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            max_iterations: 5,
            max_history_messages: 20,
            system_prompt: super::default_system_prompt().to_string(),
            verbose: false,
            plugin_registry: PluginRegistry::new(),
        }
    }
}

/// ReAct Agent
pub struct ReActAgent {
    llm: Arc<dyn LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    config: AgentConfig,
    session: Arc<Session>,
}

impl ReActAgent {
    pub fn new(llm: Arc<dyn LLMClient>, tools: Arc<vol_llm_tool::ToolRegistry>, config: AgentConfig, session: Arc<Session>) -> Self {
        Self { llm, tools, config, session }
    }

    /// Run ReAct loop with streaming events
    pub async fn run(
        &self,
        user_input: &str,
        context: ToolContext,
    ) -> Result<AgentStreamReceiver, crate::AgentError> {
        // Phase 1: Generate run_id and create RunContext
        let run_id = format!("run_{}", uuid::Uuid::new_v4().simple());
        let run_ctx = RunContext::new(run_id.clone(), user_input.to_string(), self.session.id.clone(), session, tools, config);

        // Phase 2: Execute on_start hooks
        for plugin in self.config.plugin_registry.plugins() {
            match plugin.on_start(&run_ctx).await {
                PluginAction::Continue(()) => {}
                PluginAction::ShortCircuit(response) => return create_shortcircuit_stream(response, run_ctx, run_id).await,
                PluginAction::Skip => return create_skip_stream(run_ctx, run_id).await,
                PluginAction::Abort(error) => return Err(error),
            }
        }

        // Phase 3: Spawn task
        let (tx, rx) = mpsc::channel(100);
        tokio::spawn(async move {
            // Main ReAct loop implementation
        });

        // Phase 4: Wrap with plugin stream
        let raw_receiver = AgentStreamReceiver::new(rx);
        let plugin_stream = PluginStream::new(raw_receiver, plugins, run_ctx_for_stream);
        Ok(plugin_stream.into_receiver())
    }
}
```

---

## run_context.rs - 统一运行上下文

```rust
//! RunContext - Unified run state management for ReAct Agent.

use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;
use vol_llm_core::Message;
use vol_llm_core::ToolCall;
use crate::session::Session;
use vol_llm_tool::ToolRegistry;
use super::AgentConfig;

pub struct RunContext {
    // Immutable fields
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,

    // Mutable state (internal mutability)
    pub iteration: AtomicU32,
    pub messages: Arc<RwLock<Vec<Message>>>,
    pub all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,

    // Resource references
    pub session: Arc<Session>,
    pub tools: Arc<ToolRegistry>,
    pub config: AgentConfig,
}

impl RunContext {
    pub fn new(run_id: String, user_input: String, session_id: String, session: Arc<Session>, tools: Arc<ToolRegistry>, config: AgentConfig) -> Self {
        Self {
            run_id, user_input, session_id,
            iteration: AtomicU32::new(0),
            messages: Arc::new(RwLock::new(Vec::new())),
            all_tool_calls: Arc::new(RwLock::new(Vec::new())),
            current_tool_calls: Arc::new(RwLock::new(Vec::new())),
            data: Arc::new(RwLock::new(HashMap::new())),
            session, tools, config,
        }
    }

    pub fn current_iteration(&self) -> u32 {
        self.iteration.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn next_iteration(&self) {
        self.iteration.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    pub async fn add_message(&self, message: Message) {
        self.messages.write().await.push(message);
    }

    pub async fn get_messages(&self) -> Vec<Message> {
        self.messages.read().await.clone()
    }

    pub async fn add_tool_call(&self, tool_call: ToolCall) {
        self.current_tool_calls.write().await.push(tool_call.clone());
        self.all_tool_calls.write().await.push(tool_call);
    }

    pub async fn clear_current_tool_calls(&self) {
        self.current_tool_calls.write().await.clear();
    }

    pub async fn get_current_tool_calls(&self) -> Vec<ToolCall> {
        self.current_tool_calls.read().await.clone()
    }

    pub async fn get_all_tool_calls(&self) -> Vec<ToolCall> {
        self.all_tool_calls.read().await.clone()
    }

    pub async fn get<T: for<'de> serde::Deserialize<'de>>(&self, key: &str) -> Option<T> {
        self.data.read().await.get(key).and_then(|v| serde_json::from_value(v.clone()).ok())
    }

    pub async fn set<T: serde::Serialize>(&self, key: &str, value: T) -> Result<(), serde_json::Error> {
        self.data.write().await.insert(key.to_string(), serde_json::to_value(value)?);
        Ok(())
    }
}

impl Clone for RunContext {
    fn clone(&self) -> Self {
        Self {
            run_id: self.run_id.clone(),
            user_input: self.user_input.clone(),
            session_id: self.session_id.clone(),
            iteration: AtomicU32::new(self.current_iteration()),
            messages: self.messages.clone(),
            all_tool_calls: self.all_tool_calls.clone(),
            current_tool_calls: self.current_tool_calls.clone(),
            data: self.data.clone(),
            session: self.session.clone(),
            tools: self.tools.clone(),
            config: self.config.clone(),
        }
    }
}
```

---

## plugin.rs - 插件系统

```rust
//! Plugin system for ReAct Agent.

use async_trait::async_trait;
use std::sync::Arc;
use super::run_context::RunContext;
use super::{AgentStreamEvent, AgentResponse, AgentError};

pub type PluginId = String;
pub type StreamEvent = Result<AgentStreamEvent, AgentError>;

#[derive(Debug)]
pub enum PluginAction<T = ()> {
    Continue(T),
    ShortCircuit(AgentResponse),
    Skip,
    Abort(AgentError),
}

#[async_trait]
pub trait AgentPlugin: Send + Sync {
    fn id(&self) -> PluginId;
    fn priority(&self) -> u32 { 100 }

    async fn on_start(&self, _ctx: &RunContext) -> PluginAction<()> {
        PluginAction::Continue(())
    }

    async fn intercept(&self, event: StreamEvent, ctx: &RunContext) -> PluginAction<Option<StreamEvent>>;

    async fn on_complete(&self, ctx: &RunContext, response: &AgentResponse) -> PluginAction<()>;

    async fn on_error(&self, _ctx: &RunContext, _error: &AgentError) -> PluginAction<()> {
        PluginAction::Continue(())
    }
}

#[derive(Clone)]
pub struct PluginRegistry {
    plugins: Vec<Arc<dyn AgentPlugin>>,
}

impl PluginRegistry {
    pub fn new() -> Self {
        Self { plugins: Vec::new() }
    }

    pub fn register<P: AgentPlugin + 'static>(&mut self, plugin: P) {
        let plugin = Arc::new(plugin);
        let pos = self.plugins.iter()
            .position(|p| p.priority() > plugin.priority())
            .unwrap_or(self.plugins.len());
        self.plugins.insert(pos, plugin);
    }

    pub fn plugins(&self) -> &[Arc<dyn AgentPlugin>] {
        &self.plugins
    }
}
```

---

## plugin_stream.rs - 插件流包装器

```rust
//! Plugin stream wrapper and short-circuit utilities.

use super::plugin::*;
use super::{AgentStreamEvent, AgentResponse, AgentStreamReceiver, AgentError};
use super::run_context::RunContext;
use tokio::sync::mpsc;
use std::sync::Arc;

pub struct PluginStream {
    inner: AgentStreamReceiver,
    plugins: Vec<Arc<dyn AgentPlugin>>,
    ctx: RunContext,
}

impl PluginStream {
    pub fn new(inner: AgentStreamReceiver, plugins: Vec<Arc<dyn AgentPlugin>>, ctx: RunContext) -> Self {
        Self { inner, plugins, ctx }
    }

    pub async fn recv(&mut self) -> Option<Result<AgentStreamEvent, AgentError>> {
        loop {
            let raw_event = self.inner.recv().await?;
            let mut current = Some(raw_event);

            for plugin in &self.plugins {
                match current {
                    Some(event) => {
                        match plugin.intercept(event, &self.ctx).await {
                            PluginAction::Continue(Some(e)) => current = Some(e),
                            PluginAction::Continue(None) => { current = None; break; }
                            PluginAction::ShortCircuit(response) => {
                                return Some(Ok(AgentStreamEvent::AgentComplete { response }));
                            }
                            PluginAction::Skip => { current = None; break; }
                            PluginAction::Abort(e) => return Some(Err(e)),
                        }
                    }
                    None => { current = None; break; }
                }
            }

            if current.is_some() {
                return current;
            }
        }
    }

    pub fn into_receiver(self) -> AgentStreamReceiver {
        let (tx, rx) = mpsc::channel(100);
        tokio::spawn(async move {
            let mut stream = self;
            while let Some(event) = stream.recv().await {
                if tx.send(event).await.is_err() { break; }
            }
        });
        AgentStreamReceiver::new(rx)
    }
}
```

---

## builder.rs - Agent 构建器

```rust
//! Agent builder.

use std::sync::Arc;
use vol_llm_core::LLMClient;
use vol_llm_tool::{Tool, ToolRegistry};
use super::agent::{AgentConfig, ReActAgent};
use super::plugin::AgentPlugin;
use crate::session::{Session, InMemorySessionStore, InMemoryMessageStore};

pub struct AgentBuilder {
    llm: Option<Arc<dyn LLMClient>>,
    tools: Vec<Box<dyn Tool>>,
    config: AgentConfig,
    session: Option<Arc<Session>>,
}

impl AgentBuilder {
    pub fn new() -> Self {
        Self {
            llm: None,
            tools: Vec::new(),
            config: AgentConfig::default(),
            session: None,
        }
    }

    pub fn with_llm(mut self, llm: Arc<dyn LLMClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn with_tool<T: Tool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
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

    pub fn with_max_history_messages(mut self, max: usize) -> Self {
        self.config.max_history_messages = max;
        self
    }

    pub fn with_session(mut self, session: Arc<Session>) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_plugin<P: AgentPlugin + 'static>(mut self, plugin: P) -> Self {
        self.config.plugin_registry.register(plugin);
        self
    }

    pub fn build(self) -> Result<ReActAgent, AgentBuilderError> {
        let llm = self.llm.ok_or(AgentBuilderError::MissingLlm)?;
        let mut registry = ToolRegistry::new();
        for tool in self.tools {
            registry.register_boxed(tool);
        }
        let session = self.session.unwrap_or_else(|| {
            Arc::new(Session::new(
                uuid::Uuid::new_v4().to_string(),
                Arc::new(InMemorySessionStore::new()),
                Arc::new(InMemoryMessageStore::new()),
            ))
        });
        Ok(ReActAgent::new(llm, Arc::new(registry), self.config, session))
    }
}
```

---

## stream.rs - 流事件定义

```rust
//! Agent streaming events and receiver.

use vol_llm_core::ToolCall;
use super::response::AgentResponse;

#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    AgentStart { input: String },
    ThinkingComplete { thinking: String },
    ToolCallBegin { tool_name: String, arguments: String },
    ToolCallComplete { tool_name: String, result: String },
    IterationComplete {
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },
    AgentComplete { response: AgentResponse },
}

pub struct AgentStreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, super::response::AgentError>>,
}

impl AgentStreamReceiver {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, super::response::AgentError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<AgentStreamEvent, super::response::AgentError>> {
        self.rx.recv().await
    }
}
```

---

## response.rs - 响应和错误类型

```rust
//! Agent response and error types.

use thiserror::Error;
use vol_llm_core::LLMError;
use vol_llm_core::ToolCall;

#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub reasoning: String,
    pub iterations: u32,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Error)]
pub enum AgentError {
    #[error("LLM error: {0}")]
    Llm(#[from] LLMError),

    #[error("Tool execution failed: {tool}: {error}")]
    ToolExecution { tool: String, error: String },

    #[error("Max iterations ({max}) reached without final response")]
    MaxIterationsReached { max: u32 },

    #[error("Invalid tool response: {0}")]
    InvalidToolResponse(String),

    #[error("Context error: {0}")]
    Context(String),
}
```

---

## prompt.rs - 系统提示模板

```rust
//! System prompt templates.

pub fn default_system_prompt() -> &'static str {
    r#"你是一个专业的衍生品市场风险分析师。

你的任务是分析监控系统的告警，为用户提供深入的市场洞察和风险评估。

## 可用工具

你可以使用以下工具获取额外信息：

- `alert_history(symbol, tenor?, alert_type?)`: 查询历史告警
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
- 最终结论应该清晰、可操作，包括风险等级和具体建议"#
}

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
        if let Some(instructions) = self.custom_instructions {
            prompt.push_str(&format!("\n\n## 额外指示\n\n{}", instructions));
        }
        prompt
    }
}
```

---

## hitl.rs - Human-in-the-Loop 支持

```rust
//! Human-in-the-Loop support for ReAct Agent.

use async_trait::async_trait;
use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Clone)]
pub struct ApprovalRequest {
    pub run_id: String,
    pub request_type: ApprovalType,
    pub message: String,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalType {
    ToolExecution { tool_name: String },
    ContinueIteration { iteration: u32 },
    FinalAnswer,
    Custom { name: String },
}

#[derive(Debug, Clone)]
pub enum ApprovalResponse {
    Approved,
    Rejected { reason: String },
}

#[async_trait]
pub trait ApprovalChannel: Send + Sync {
    async fn request_approval(
        &self,
        request: ApprovalRequest,
        timeout: Option<Duration>,
    ) -> Result<Option<ApprovalResponse>, ApprovalError>;
}

#[derive(Debug, Error)]
pub enum ApprovalError {
    #[error("Channel closed")]
    ChannelClosed,
    #[error("Timeout waiting for approval")]
    Timeout,
    #[error("Transport error: {0}")]
    Transport(String),
}

#[derive(Debug, Clone)]
pub struct HitlConfig {
    pub triggers: Vec<ApprovalTrigger>,
    pub timeout_secs: u64,
    pub on_timeout: TimeoutBehavior,
    pub timeout_message: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ApprovalTrigger {
    ToolExecution { tools: Option<Vec<String>> },
    AfterIteration,
    BeforeFinalAnswer,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeoutBehavior {
    Approve,
    Reject { reason: String },
    Stop,
}

pub struct HitlPlugin<C: ApprovalChannel> {
    config: HitlConfig,
    channel: Arc<C>,
}

impl<C: ApprovalChannel> HitlPlugin<C> {
    pub fn new(config: HitlConfig, channel: Arc<C>) -> Self {
        Self { config, channel }
    }
}

#[async_trait]
impl<C: ApprovalChannel + 'static> AgentPlugin for HitlPlugin<C> {
    fn id(&self) -> PluginId { "human_in_loop".to_string() }
    fn priority(&self) -> u32 { 25 }

    async fn intercept(&self, event: StreamEvent, ctx: &RunContext) -> PluginAction<Option<StreamEvent>> {
        // Implementation for tool execution approval and iteration pause
        PluginAction::Continue(Some(event))
    }

    async fn on_complete(&self, _ctx: &RunContext, _response: &AgentResponse) -> PluginAction<()> {
        PluginAction::Continue(())
    }
}
```

---

## 测试结果

```
cargo test -p vol-llm-agent

test result: ok. 62 passed; 0 failed
- 49 lib tests
- 4 plugin tests  
- 2 integration tests
- 2 session tests
- 5 doc tests
```

---

**完整代码位置：** `crates/vol-llm-agent/src/react/`
