# AI Agent - 通用 LLM Client 架构设计

**创建日期**: 2026-04-06  
**状态**: 设计中  
**作者**: vol-monitor team

---

## 1. 概述

### 1.1 背景

vol-monitor 系统需要集成 LLM 能力，为告警提供 AI 驱动的分析与洞察。为避免锁定单一 Provider，并支持未来扩展（本地模型、多 Provider 负载均衡），需要设计一个通用的 LLM Client 架构。

### 1.2 设计目标

| 目标 | 说明 |
|------|------|
| **Provider 无关** | 上层代码无需关心底层使用的是 Claude、GPT 还是本地模型 |
| **多端点支持** | 支持配置多个 LLM 端点，可切换、可降级 |
| **协议兼容** | 原生支持 Anthropic、OpenAI 协议，兼容 OpenAI 协议的第三方服务 |
| **配置驱动** | 所有参数通过 TOML 配置，支持环境变量覆盖 |
| **异步非阻塞** | 完全异步设计，AI 任务不影响主流程告警发送 |

### 1.3 架构范围

```
┌─────────────────────────────────────────────────────────────┐
│                      vol-agent crate                         │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                  AIAgentService                      │   │
│  │  - 订阅 Alert broadcast                              │   │
│  │  - 异步分析 + 存储洞察                                │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│                          ▼                                  │
│  ┌─────────────────────────────────────────────────────┐   │
│  │                LLMClientRegistry                     │   │
│  │  - claude-main ──▶ AnthropicClient                  │   │
│  │  - gpt-backup  ──▶ OpenAIClient                     │   │
│  │  - local-model ──▶ CompatibleClient                 │   │
│  └─────────────────────────────────────────────────────┘   │
│                          │                                  │
│          ┌───────────────┼───────────────┐                 │
│          ▼               ▼               ▼                 │
│  ┌─────────────┐ ┌─────────────┐ ┌─────────────┐          │
│  │ Anthropic   │ │  OpenAI     │ │ Compatible  │          │
│  │ Client      │ │  Client     │ │  Client     │          │
│  │ (Claude API)│ │ (OpenAI API)│ │ (vllm/ollama)│         │
│  └─────────────┘ └─────────────┘ └─────────────┘          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 2. 核心抽象

### 2.1 LLMClient Trait

```rust
/// 通用 LLM Client trait
/// 
/// 抽象了不同 Provider 的差异，提供统一接口
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// 获取 Provider 名称
    fn provider(&self) -> LLMProvider;
    
    /// 获取配置的模型名
    fn model(&self) -> &str;
    
    /// 发送聊天请求
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse>;
    
    /// 简单聊天（快捷方法）
    async fn chat_simple(&self, prompt: &str) -> Result<String>;
}
```

### 2.2 Provider 枚举

```rust
/// Provider 类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    /// Anthropic (Claude API)
    Anthropic,
    /// OpenAI (GPT API)
    OpenAI,
    /// 兼容 OpenAI 协议的第三方服务 (vllm, ollama, localai 等)
    Compatible,
}
```

### 2.3 统一消息类型

```rust
/// 聊天消息角色
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// 聊天消息
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    /// 可选：工具调用（用于 function calling）
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// 工具调用
pub struct ToolCall {
    pub name: String,
    pub arguments: String,  // JSON string
}

impl Message {
    pub fn system(content: &str) -> Self {
        Self { role: MessageRole::System, content: content.to_string(), tool_calls: None }
    }
    
    pub fn user(content: &str) -> Self {
        Self { role: MessageRole::User, content: content.to_string(), tool_calls: None }
    }
    
    pub fn assistant(content: &str) -> Self {
        Self { role: MessageRole::Assistant, content: content.to_string(), tool_calls: None }
    }
}
```

### 2.4 请求/响应类型

```rust
/// 聊天请求
pub struct ChatRequest {
    /// 消息历史
    pub messages: Vec<Message>,
    
    /// 最大生成 tokens
    pub max_tokens: Option<u32>,
    
    /// 温度参数 (0.0 - 2.0)
    pub temperature: Option<f64>,
    
    /// 可选：工具定义（用于 function calling）
    pub tools: Option<Vec<ToolDefinition>>,
    
    /// 可选：强制使用某个工具
    pub tool_choice: Option<ToolChoice>,
}

/// 工具定义
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,  // JSON Schema
}

/// 工具选择
pub enum ToolChoice {
    Auto,       // 自动决定是否使用工具
    Required,   // 必须使用工具
    Specific(String),  // 指定工具名
}

/// 聊天响应
pub struct ChatResponse {
    /// 生成的消息
    pub message: Message,
    
    /// 使用的模型
    pub model: String,
    
    /// Usage 统计
    pub usage: Option<Usage>,
    
    /// Provider 原始响应（用于调试）
    pub raw: Option<serde_json::Value>,
}

/// Token Usage 统计
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

### 2.5 错误类型

```rust
#[derive(thiserror::Error, Debug)]
pub enum LLMError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    
    #[error("Authentication failed: {0}")]
    Auth(String),
    
    #[error("Rate limit exceeded, retry after {retry_after:?}")]
    RateLimit { retry_after: Option<Duration> },
    
    #[error("Invalid response format: {0}")]
    Parse(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("Unknown endpoint: {0}")]
    UnknownEndpoint(String),
}

pub type Result<T> = std::result::Result<T, LLMError>;
```

---

## 3. Provider 实现

### 3.1 Anthropic Client

```rust
pub struct AnthropicClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    endpoint: String,
    timeout: Duration,
}

impl AnthropicClient {
    pub fn new(config: &LLMEndpointConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: Self::get_api_key(&config.api_key_env),
            model: config.model.clone(),
            endpoint: config.endpoint.clone(),
            timeout: Duration::from_secs(config.timeout_secs),
        }
    }
    
    fn get_api_key(env_var: &str) -> String {
        std::env::var(env_var)
            .ok()
            .expect("API key env var not set")
    }
}

#[async_trait]
impl LLMClient for AnthropicClient {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }
    
    fn model(&self) -> &str {
        &self.model
    }
    
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        // Anthropic Messages API: POST /v1/messages
        let url = format!("{}/v1/messages", self.endpoint);
        
        // Anthropic 特定要求：max_tokens 是必填字段
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": request.max_tokens.unwrap_or(500),
            "messages": self.convert_messages(&request.messages),
            "temperature": request.temperature.unwrap_or(0.7),
        });
        
        let response = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;
            
        self.parse_response(response).await
    }
}

/// Anthropic 消息格式转换
impl AnthropicClient {
    fn convert_messages(&self, messages: &[Message]) -> serde_json::Value {
        serde_json::json!(messages.iter().map(|m| {
            serde_json::json!({
                "role": match m.role {
                    MessageRole::System => "system",  // Anthropic 支持 system 角色
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                },
                "content": m.content,
            })
        }).collect::<Vec<_>>())
    }
}
```

### 3.2 OpenAI Client

```rust
pub struct OpenAIClient {
    client: reqwest::Client,
    api_key: String,
    model: String,
    endpoint: String,
    timeout: Duration,
}

#[async_trait]
impl LLMClient for OpenAIClient {
    fn provider(&self) -> LLMProvider {
        LLMProvider::OpenAI
    }
    
    fn model(&self) -> &str {
        &self.model
    }
    
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        // OpenAI Chat Completions API: POST /chat/completions
        let url = format!("{}/chat/completions", self.endpoint);
        
        let body = serde_json::json!({
            "model": self.model,
            "messages": self.convert_messages(&request.messages),
            "max_tokens": request.max_tokens,
            "temperature": request.temperature.unwrap_or(0.7),
            // 可选：tools 和 tool_choice
        });
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .timeout(self.timeout)
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;
            
        self.parse_response(response).await
    }
}
```

### 3.3 Compatible Client

```rust
/// 兼容 OpenAI 协议的客户端
/// 用于 vllm, ollama, localai, siliconflow 等
pub struct CompatibleClient {
    inner: OpenAIClient,
    custom_headers: HashMap<String, String>,
}

#[async_trait]
impl LLMClient for CompatibleClient {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Compatible
    }
    
    fn model(&self) -> &str {
        self.inner.model()
    }
    
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse> {
        // 复用 OpenAI 实现
        // 可以在这里添加自定义 headers 处理兼容性问题
        let mut req = self.inner.chat(request).await?;
        
        // 处理不同兼容服务的响应格式差异
        // 例如：某些服务返回的字段名略有不同
        
        Ok(req)
    }
}
```

---

## 4. 配置设计

### 4.1 完整配置示例

```toml
# config.dev.toml

# ============== AI Agent 配置 ==============

# Agent 主配置
[agent]
enabled = true                    # 是否启用 Agent
default_endpoint = "claude-main"  # 默认使用的 endpoint ID
push_to_dm = false                # 是否推送 AI 洞察到 Feishu DM

# 多个 LLM 端点配置
[[agent.llm_endpoints]]
id = "claude-main"
provider = "anthropic"
endpoint = "https://api.anthropic.com"
model = "claude-sonnet-4-20251001"
api_key_env = "ANTHROPIC_API_KEY"
timeout_secs = 30
max_tokens = 500
temperature = 0.7

[[agent.llm_endpoints]]
id = "gpt-backup"
provider = "openai"
endpoint = "https://api.openai.com/v1"
model = "gpt-4o"
api_key_env = "OPENAI_API_KEY"
timeout_secs = 30
max_tokens = 500
temperature = 0.7

[[agent.llm_endpoints]]
id = "local-model"
provider = "compatible"
endpoint = "http://localhost:8000/v1"
model = "qwen2.5-72b"
api_key_env = "LOCAL_API_KEY"  # 可以为空
timeout_secs = 60
max_tokens = 1000
temperature = 0.8

# 告警过滤配置 - 只分析特定告警
[agent.alert_filters]
types = ["absolute_iv", "rate_change"]  # 告警类型白名单
symbols = ["BTC", "ETH"]                 # Symbol 白名单
min_iv_threshold = 0.70                  # 只分析 IV > 70% 的告警

# 存储配置
[agent.storage]
db_path = "./agent.db"
retention_days = 30
```

### 4.2 配置结构定义

```rust
/// Agent 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    /// 是否启用 Agent
    #[serde(default)]
    pub enabled: bool,
    
    /// 默认使用的 endpoint ID
    #[serde(default = "default_endpoint")]
    pub default_endpoint: String,
    
    /// 是否推送到 Feishu DM
    #[serde(default)]
    pub push_to_dm: bool,
    
    /// LLM 端点配置（支持多个）
    #[serde(default)]
    pub llm_endpoints: Vec<LLMEndpointConfig>,
    
    /// 告警过滤配置
    #[serde(default)]
    pub alert_filters: AlertFilters,
    
    /// 存储配置
    #[serde(default)]
    pub storage: StorageConfig,
}

fn default_endpoint() -> String { "claude-main".to_string() }

/// 单个 LLM 端点配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMEndpointConfig {
    /// 端点 ID（用于引用）
    pub id: String,
    
    /// Provider 类型
    pub provider: LLMProvider,
    
    /// API 端点 URL
    pub endpoint: String,
    
    /// 使用的模型名
    pub model: String,
    
    /// API Key 所在环境变量名
    pub api_key_env: String,
    
    /// 请求超时（秒）
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    
    /// 最大生成 tokens
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    
    /// 温度参数
    #[serde(default = "default_temperature")]
    pub temperature: f64,
}

fn default_timeout() -> u64 { 30 }
fn default_max_tokens() -> u32 { 500 }
fn default_temperature() -> f64 { 0.7 }

/// 告警过滤配置
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct AlertFilters {
    /// 只分析特定类型的告警
    #[serde(default)]
    pub types: Vec<String>,
    
    /// 只分析特定 symbol
    #[serde(default)]
    pub symbols: Vec<String>,
    
    /// 最小 IV 阈值
    #[serde(default)]
    pub min_iv_threshold: Option<f64>,
}

/// 存储配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StorageConfig {
    /// SQLite 数据库路径
    #[serde(default = "default_db_path")]
    pub db_path: String,
    
    /// 数据保留天数
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
}

fn default_db_path() -> String { "./agent.db".to_string() }
fn default_retention_days() -> u32 { 30 }
```

---

## 5. Client 工厂与 Registry

### 5.1 LLMClientFactory

```rust
pub struct LLMClientFactory;

impl LLMClientFactory {
    /// 根据配置创建对应的 Client
    pub fn create(config: &LLMEndpointConfig) -> Result<Box<dyn LLMClient>> {
        let client: Box<dyn LLMClient> = match config.provider {
            LLMProvider::Anthropic => {
                Box::new(AnthropicClient::new(config))
            }
            LLMProvider::OpenAI => {
                Box::new(OpenAIClient::new(config))
            }
            LLMProvider::Compatible => {
                Box::new(CompatibleClient::new(config))
            }
        };
        Ok(client)
    }
    
    /// 创建多个 endpoints 的 registry
    pub fn create_registry(
        configs: &[LLMEndpointConfig]
    ) -> Result<LLMClientRegistry> {
        let mut clients = HashMap::new();
        
        for config in configs {
            let client = Self::create(config)?;
            clients.insert(config.id.clone(), client);
        }
        
        Ok(LLMClientRegistry { clients })
    }
}
```

### 5.2 LLMClientRegistry

```rust
/// Client Registry - 管理多个 endpoint
pub struct LLMClientRegistry {
    clients: HashMap<String, Box<dyn LLMClient>>,
}

impl LLMClientRegistry {
    /// 获取指定 endpoint 的 client
    pub fn get(&self, id: &str) -> Option<&dyn LLMClient> {
        self.clients.get(id).map(|c| c.as_ref())
    }
    
    /// 获取默认 client
    pub fn default_client(&self) -> Option<&dyn LLMClient> {
        self.clients.values().next().map(|c| c.as_ref())
    }
    
    /// 获取所有可用的 endpoint IDs
    pub fn available_endpoints(&self) -> Vec<&str> {
        self.clients.keys().map(|s| s.as_str()).collect()
    }
}
```

---

## 6. 使用示例

### 6.1 基础使用

```rust
use vol_agent::{AgentConfig, AIAgentService, LLMClientFactory};

// 1. 加载配置
let config: AgentConfig = toml::from_str(&config_content)?;

// 2. 创建 Client Registry
let clients = LLMClientFactory::create_registry(&config.llm_endpoints)?;

// 3. 获取特定 endpoint 的 client
let claude = clients.get("claude-main").unwrap();

// 4. 发送请求
let response = claude.chat_simple("分析这个告警：ETH IV 超过 90%").await?;
println!("AI 分析：{}", response);

// 5. 使用不同 endpoint
let gpt = clients.get("gpt-backup").unwrap();
let response2 = gpt.chat_simple("分析这个告警：ETH IV 超过 90%").await?;
```

### 6.2 AIAgentService 集成

```rust
pub struct AIAgentService {
    clients: LLMClientRegistry,
    default_endpoint: String,
    config: AgentConfig,
    // ... 其他字段 (context store, storage 等)
}

impl AIAgentService {
    pub async fn new(config: AgentConfig) -> Result<Self> {
        let clients = LLMClientFactory::create_registry(&config.llm_endpoints)?;
        
        Ok(Self {
            clients,
            default_endpoint: config.default_endpoint.clone(),
            config,
            // ...
        })
    }
    
    /// 订阅 alert broadcast，异步处理
    pub async fn run(
        mut self,
        mut alert_rx: broadcast::Receiver<TracedEvent<Alert>>
    ) {
        while let Ok(traced_alert) = alert_rx.recv().await {
            let (alert, _span, trace_id) = traced_alert.split();
            
            // 1. 过滤告警
            if !self.should_process(&alert) {
                continue;
            }
            
            // 2. 获取上下文
            let context = self.fetch_context(&alert).await;
            
            // 3. 构建 prompt
            let prompt = self.build_analysis_prompt(&alert, &context);
            
            // 4. 调用 LLM
            let client = self.clients.get(&self.default_endpoint).unwrap();
            match client.chat_simple(&prompt).await {
                Ok(insight) => {
                    // 5. 存储洞察
                    self.store_insight(&alert, &insight, &trace_id).await;
                    
                    // 6. 可选推送
                    if self.config.push_to_dm {
                        self.send_feishu_dm(&alert, &insight).await;
                    }
                }
                Err(e) => {
                    tracing::error!("LLM analysis failed: {}", e);
                    // 失败不影响主流程，继续处理下一个
                }
            }
        }
    }
    
    /// 支持指定 endpoint（用于 A/B 测试或降级）
    pub async fn analyze_with_endpoint(
        &self,
        endpoint_id: &str,
        alert: &Alert,
        context: &AnalysisContext,
    ) -> Result<String> {
        let client = self.clients.get(endpoint_id)
            .ok_or_else(|| LLMError::UnknownEndpoint(endpoint_id.to_string()))?;
        
        let prompt = self.build_analysis_prompt(alert, context);
        client.chat_simple(&prompt).await
    }
}
```

---

## 7. 架构决策

### 7.1 为什么使用 Trait 抽象？

| 方案 | 优点 | 缺点 |
|------|------|------|
| **Trait 抽象** | 上层代码完全解耦，易于扩展新 Provider | 需要统一类型设计 |
| **各用各的** | 实现简单 | 调用方需要 if/else 判断 Provider |
| **Enum 分发** | 类型安全 | 每次新增 Provider 要修改所有匹配代码 |

**决策**: Trait 抽象提供了最佳的扩展性和解耦性。

### 7.2 为什么支持多 Endpoint？

1. **降级容灾**: Claude API 挂了可以切换到 GPT
2. **成本优化**: 简单分析用便宜模型，复杂分析用高端模型
3. **A/B 测试**: 同时测试不同模型的输出质量
4. **本地优先**: 优先使用本地模型，失败时 fallback 到云端

### 7.3 为什么配置驱动？

1. **无需改代码**: 调整模型/参数只需改配置
2. **环境隔离**: dev/staging/prod 可以用不同配置
3. **密钥管理**: API Key 通过环境变量，不进入配置文件

---

## 8. 测试策略

### 8.1 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // Mock Client 用于测试
    struct MockClient {
        response: String,
    }
    
    #[async_trait]
    impl LLMClient for MockClient {
        fn provider(&self) -> LLMProvider {
            LLMProvider::Compatible
        }
        
        fn model(&self) -> &str {
            "mock-model"
        }
        
        async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse> {
            Ok(ChatResponse {
                message: Message::assistant(&self.response),
                model: self.model().to_string(),
                usage: None,
                raw: None,
            })
        }
    }
    
    #[tokio::test]
    async fn test_agent_service_with_mock() {
        let mock = MockClient { response: "test insight".to_string() };
        // ... 测试 Agent 逻辑
    }
}
```

### 8.2 集成测试

```rust
// tests/llm_integration.rs

// 需要设置环境变量
// export ANTHROPIC_API_KEY=xxx
// export OPENAI_API_KEY=xxx

#[tokio::test]
#[ignore]  // 默认跳过，需要时运行
async fn test_anthropic_real_api() {
    let config = LLMEndpointConfig {
        id: "test".to_string(),
        provider: LLMProvider::Anthropic,
        endpoint: "https://api.anthropic.com".to_string(),
        model: "claude-sonnet-4-20251001".to_string(),
        api_key_env: "ANTHROPIC_API_KEY".to_string(),
        timeout_secs: 30,
        max_tokens: 100,
        temperature: 0.7,
    };
    
    let client = AnthropicClient::new(&config);
    let response = client.chat_simple("Hello").await.unwrap();
    assert!(!response.is_empty());
}
```

---

## 9. 成本估算

### 9.1 Token 使用量预估

以单次告警分析为例：

| 阶段 | Prompt Tokens | Completion Tokens | 合计 |
|------|--------------|-------------------|------|
| 告警分析 | ~500 | ~200 | ~700 |

### 9.2 日成本估算

假设日均 100 条告警：

| Provider | 单价 ($/1M tokens) | 日成本 | 月成本 |
|----------|-------------------|--------|--------|
| Claude Sonnet 4 | ~$3 | ~$0.02 | ~$0.60 |
| GPT-4o | ~$5 | ~$0.04 | ~$1.20 |
| 本地模型 | $0 | $0 | $0 |

**结论**: 成本可控，可以放心使用。

---

## 10. 后续扩展

### 10.1 Function Calling

未来可以扩展工具调用能力：

```rust
// 定义工具
let tools = vec![ToolDefinition {
    name: "get_historical_iv".to_string(),
    description: "获取历史 IV 数据".to_string(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "symbol": {"type": "string"},
            "days": {"type": "integer"}
        },
        "required": ["symbol", "days"]
    }),
}];

// LLM 可以决定调用工具
let request = ChatRequest {
    messages: vec![Message::user("查询 BTC 过去 30 天的 IV")],
    tools: Some(tools),
    tool_choice: Some(ToolChoice::Auto),
    ..Default::default()
};

let response = client.chat(request).await?;
// response.message.tool_calls 包含工具调用信息
```

### 10.2 流式响应

```rust
/// 流式聊天
#[async_trait]
pub trait LLMClient: Send + Sync {
    // ... 现有方法
    
    /// 流式响应
    async fn chat_stream(
        &self,
        request: ChatRequest
    ) -> Result<impl Stream<Item = Result<String>>>;
}
```

### 10.3 批量分析

```rust
/// 批量分析告警（降低延迟）
pub async fn analyze_batch(
    &self,
    alerts: Vec<&Alert>,
) -> Result<Vec<AnalysisResult>> {
    // 并发调用 LLM
    let futures = alerts.iter().map(|a| self.analyze(a));
    futures::future::join_all(futures).await
}
```

---

## 11. 参考

- [Anthropic Messages API](https://docs.anthropic.com/claude/reference/messages_post)
- [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat)
- [OpenAI Compatible API](https://platform.openai.com/docs/api-reference)
