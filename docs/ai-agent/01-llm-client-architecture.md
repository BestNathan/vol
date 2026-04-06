# AI Agent - LLM Client 架构设计

**创建日期**: 2026-04-06  
**更新日期**: 2026-04-06  
**状态**: 设计中  
**作者**: vol-monitor team

---

## 1. 概述

### 1.1 背景

vol-monitor 系统需要集成 LLM 能力，为告警提供 AI 驱动的分析与洞察。设计一个简洁、可扩展的 LLM Client 架构，支持 Anthropic 和 OpenAI 两家主流 Provider。

### 1.2 设计目标

| 目标 | 说明 |
|------|------|
| **统一抽象** | Agent 通过统一消息类型与 LLM 交互，不关心底层 Provider |
| **协议适配** | Provider 层负责协议转换，隔离差异 |
| **配置驱动** | 所有参数通过 TOML 配置，支持环境变量覆盖 |
| **异步非阻塞** | 完全异步设计，AI 任务不影响主流程告警发送 |

### 1.3 架构范围

```
┌─────────────────────────────────────────────────────────────┐
│                    AIAgentService                            │
│                          │                                   │
│                          ▼                                   │
│              LLMClient (统一抽象层)                           │
│                          │                                   │
│          ┌───────────────┴───────────────┐                  │
│          ▼                               ▼                   │
│   AnthropicProvider               OpenAI Provider           │
│   (协议适配层)                    (协议适配层)               │
└─────────────────────────────────────────────────────────────┘
```

### 1.4 关键设计决策

1. **只支持两家原生协议** - Anthropic 和 OpenAI，移除通用的 Compatible Provider
2. **统一消息在抽象层** - Agent 与 LLM 交互使用统一的消息类型
3. **Provider 负责协议转换** - 每家 Provider 实现统一的 LLMClient Trait

---

## 2. 统一消息设计（LLM 抽象层）

### 2.1 消息类型

```rust
/// 消息角色 - Agent 内部使用
pub enum MessageRole {
    System,
    User,
    Assistant,
}

/// 消息 - Agent 与 LLM 交互的统一格式
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

impl Message {
    pub fn system(content: &str) -> Self {
        Self { role: MessageRole::System, content: content.to_string() }
    }
    
    pub fn user(content: &str) -> Self {
        Self { role: MessageRole::User, content: content.to_string() }
    }
    
    pub fn assistant(content: &str) -> Self {
        Self { role: MessageRole::Assistant, content: content.to_string() }
    }
}
```

### 2.2 对话请求

```rust
/// 对话请求 - Agent 调用 LLM 的统一格式
pub struct Conversation {
    /// 系统提示词
    pub system: Option<String>,
    /// 对话历史
    pub messages: Vec<Message>,
}
```

### 2.3 LLM 响应

```rust
/// LLM 响应 - Agent 接收的统一格式
pub struct LLMResponse {
    /// 生成的内容
    pub content: String,
    /// 使用的模型
    pub model: String,
    /// Token 使用统计
    pub usage: Option<TokenUsage>,
}

pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}
```

### 2.4 错误类型

```rust
#[derive(thiserror::Error, Debug)]
pub enum LLMError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    #[error("API error: {status} - {message}")]
    Api { status: u16, message: String },
    
    #[error("Authentication failed: {0}")]
    Auth(String),
    
    #[error("Rate limit exceeded")]
    RateLimit,
    
    #[error("Invalid response format: {0}")]
    Parse(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
}

pub type Result<T> = std::result::Result<T, LLMError>;
```

---

## 3. LLM Client 抽象层

```rust
/// LLM Client 抽象层
/// 
/// Agent 通过此接口与任意 LLM Provider 交互
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// 获取 Provider 名称
    fn provider(&self) -> LLMProvider;
    
    /// 获取配置的模型名
    fn model(&self) -> &str;
    
    /// 发送对话请求，返回统一格式的响应
    async fn converse(&self, conversation: Conversation) -> Result<LLMResponse>;
    
    /// 快捷方法：单次对话
    async fn ask(&self, prompt: &str) -> Result<String> {
        let response = self.converse(Conversation {
            system: None,
            messages: vec![Message::user(prompt)],
        }).await?;
        Ok(response.content)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LLMProvider {
    Anthropic,
    OpenAI,
}
```

---

## 4. Provider 实现（协议适配层）

### 4.1 Anthropic Provider

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl AnthropicProvider {
    pub fn new(config: &LLMConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: std::env::var(&config.api_key_env)
                .expect("API key env var not set"),
            model: config.model.clone(),
        }
    }
}

#[async_trait]
impl LLMClient for AnthropicProvider {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }
    
    fn model(&self) -> &str {
        &self.model
    }
    
    async fn converse(&self, conversation: Conversation) -> Result<LLMResponse> {
        // ========== 协议转换：统一消息 → Anthropic 格式 ==========
        // Anthropic 要求：system 和 messages 分离
        // system message 不能出现在 messages 数组中
        
        let anthropic_messages = conversation.messages.iter()
            .map(|m| {
                serde_json::json!({
                    "role": match m.role {
                        MessageRole::System | MessageRole::Assistant => "assistant",
                        MessageRole::User => "user",
                    },
                    "content": m.content,
                })
            })
            .collect::<Vec<_>>();
        
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 1024,
            "messages": anthropic_messages,
        });
        
        // System message 单独传
        if let Some(system) = conversation.system {
            body["system"] = serde_json::Value::String(system);
        }
        
        // ========== 发送请求 ==========
        let response = self.client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;
        
        if !response.status().is_success() {
            return Err(LLMError::Api {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }
        
        // ========== 协议转换：Anthropic 格式 → 统一响应 ==========
        let result: serde_json::Value = response.json().await?;
        
        Ok(LLMResponse {
            content: result["content"][0]["text"].as_str().unwrap_or("").to_string(),
            model: result["model"].as_str().unwrap_or("").to_string(),
            usage: Some(TokenUsage {
                prompt_tokens: result["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: result["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: (
                    result["usage"]["input_tokens"].as_u64().unwrap_or(0) +
                    result["usage"]["output_tokens"].as_u64().unwrap_or(0)
                ) as u32,
            }),
        })
    }
}
```

### 4.2 OpenAI Provider

```rust
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
}

impl OpenAIProvider {
    pub fn new(config: &LLMConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: std::env::var(&config.api_key_env)
                .expect("API key env var not set"),
            model: config.model.clone(),
        }
    }
}

#[async_trait]
impl LLMClient for OpenAIProvider {
    fn provider(&self) -> LLMProvider {
        LLMProvider::OpenAI
    }
    
    fn model(&self) -> &str {
        &self.model
    }
    
    async fn converse(&self, conversation: Conversation) -> Result<LLMResponse> {
        // ========== 协议转换：统一消息 → OpenAI 格式 ==========
        // OpenAI 要求：messages 数组，system 作为第一条消息
        
        let mut openai_messages = Vec::new();
        
        // System message 作为第一条
        if let Some(system) = conversation.system {
            openai_messages.push(serde_json::json!({
                "role": "system",
                "content": system,
            }));
        }
        
        // 对话历史
        for msg in conversation.messages {
            openai_messages.push(serde_json::json!({
                "role": match msg.role {
                    MessageRole::System => "system",
                    MessageRole::User => "user",
                    MessageRole::Assistant => "assistant",
                },
                "content": msg.content,
            }));
        }
        
        let body = serde_json::json!({
            "model": self.model,
            "messages": openai_messages,
        });
        
        // ========== 发送请求 ==========
        let response = self.client
            .post("https://api.openai.com/v1/chat/completions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;
        
        if !response.status().is_success() {
            return Err(LLMError::Api {
                status: response.status().as_u16(),
                message: response.text().await.unwrap_or_default(),
            });
        }
        
        // ========== 协议转换：OpenAI 格式 → 统一响应 ==========
        let result: serde_json::Value = response.json().await?;
        
        let choices = &result["choices"];
        let content = choices[0]["message"]["content"]
            .as_str()
            .unwrap_or("")
            .to_string();
        
        Ok(LLMResponse {
            content,
            model: result["model"].as_str().unwrap_or("").to_string(),
            usage: Some(TokenUsage {
                prompt_tokens: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
                completion_tokens: result["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
                total_tokens: result["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            }),
        })
    }
}
```

---

## 5. 配置设计

### 5.1 配置结构

```rust
/// LLM 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMConfig {
    /// Provider 类型
    pub provider: LLMProvider,
    
    /// 使用的模型名
    pub model: String,
    
    /// API Key 所在环境变量名
    pub api_key_env: String,
}

/// Agent 配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    /// 是否启用 Agent
    #[serde(default)]
    pub enabled: bool,
    
    /// LLM 配置
    pub llm: LLMConfig,
    
    /// 告警过滤配置
    #[serde(default)]
    pub alert_filters: AlertFilters,
    
    /// 存储配置
    #[serde(default)]
    pub storage: StorageConfig,
}

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

### 5.2 配置示例

```toml
# config.dev.toml

[agent]
enabled = true

[agent.llm]
provider = "anthropic"  # 或 "openai"
model = "claude-sonnet-4-20251001"
api_key_env = "ANTHROPIC_API_KEY"

[agent.alert_filters]
types = ["absolute_iv", "rate_change"]
symbols = ["BTC", "ETH"]
min_iv_threshold = 0.70

[agent.storage]
db_path = "./agent.db"
retention_days = 30
```

---

## 6. Agent 使用示例

### 6.1 创建 Provider

```rust
use vol_agent::{LLMConfig, AnthropicProvider, OpenAIProvider, LLMClient};

// 从配置创建 Provider
fn create_provider(config: &LLMConfig) -> Box<dyn LLMClient> {
    match config.provider {
        LLMProvider::Anthropic => Box::new(AnthropicProvider::new(config)),
        LLMProvider::OpenAI => Box::new(OpenAIProvider::new(config)),
    }
}

let config = LLMConfig {
    provider: LLMProvider::Anthropic,
    model: "claude-sonnet-4-20251001".to_string(),
    api_key_env: "ANTHROPIC_API_KEY".to_string(),
};

let llm = create_provider(&config);
```

### 6.2 调用 LLM

```rust
// 简单调用
let response = llm.ask("分析这个告警：ETH IV 超过 90%").await?;
println!("AI 分析：{}", response);

// 带系统提示词的对话
let conversation = Conversation {
    system: Some("你是一个专业的加密货币期权交易助手。".to_string()),
    messages: vec![
        Message::user("ETH-6APR26-2025-C IV 达到 100.8%，请分析"),
    ],
};

let response = llm.converse(conversation).await?;
println!("AI 分析：{}", response.content);
println!("使用模型：{}", response.model);
println!("Token 使用：{:?}", response.usage);
```

### 6.3 AIAgentService 集成

```rust
pub struct AIAgentService {
    llm: Box<dyn LLMClient>,
    // ... 其他字段
}

impl AIAgentService {
    pub fn new(llm: Box<dyn LLMClient>) -> Self {
        Self { llm }
    }
    
    /// 分析告警
    pub async fn analyze_alert(&self, alert: &Alert, context: &AnalysisContext) -> Result<String> {
        let conversation = Conversation {
            system: Some(SYSTEM_PROMPT.to_string()),
            messages: vec![
                Message::user(&build_prompt(alert, context)),
            ],
        };
        
        let response = self.llm.converse(conversation).await?;
        Ok(response.content)
    }
    
    /// 订阅 alert broadcast，异步处理
    pub async fn run(
        mut self,
        mut alert_rx: broadcast::Receiver<TracedEvent<Alert>>
    ) {
        while let Ok(traced_alert) = alert_rx.recv().await {
            let (alert, _span, trace_id) = traced_alert.split();
            
            if !self.should_process(&alert) {
                continue;
            }
            
            let context = self.fetch_context(&alert).await;
            
            match self.analyze_alert(&alert, &context).await {
                Ok(insight) => {
                    self.store_insight(&alert, &insight, &trace_id).await;
                }
                Err(e) => {
                    tracing::error!("LLM analysis failed: {}", e);
                    // 失败不影响主流程
                }
            }
        }
    }
}
```

---

## 7. 架构决策

### 7.1 为什么只支持两家原生协议？

| 方案 | 优点 | 缺点 |
|------|------|------|
| **只支持原生** | 代码简洁，维护成本低，覆盖主流需求 | 不支持本地模型 |
| **支持兼容协议** | 可以接入 vllm/ollama | 增加复杂度，需求不强烈 |

**决策**: 先满足主流需求，后续有需要再通过实现 `LLMClient` Trait 扩展。

### 7.2 为什么统一消息在抽象层？

| 层面 | 职责 |
|------|------|
| **统一消息层** | Agent 与 LLM 交互的标准格式 |
| **Provider 层** | 将统一消息转换为目标 API 协议 |

**优点**:
1. Agent 代码完全解耦，不关心 Provider 差异
2. 添加新 Provider 只需实现协议转换，不影响 Agent
3. 易于测试：Mock Provider 返回固定响应

### 7.3 为什么配置驱动？

1. **无需改代码**: 调整模型/参数只需改配置
2. **环境隔离**: dev/staging/prod 可以用不同配置
3. **密钥管理**: API Key 通过环境变量，不进入配置文件

---

## 8. 测试策略

### 8.1 单元测试（Mock Provider）

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // Mock Provider 用于测试
    struct MockProvider;
    
    #[async_trait]
    impl LLMClient for MockProvider {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        
        async fn converse(&self, _conv: Conversation) -> Result<LLMResponse> {
            Ok(LLMResponse {
                content: "mock insight".to_string(),
                model: "mock".to_string(),
                usage: None,
            })
        }
    }
    
    #[tokio::test]
    async fn test_agent_with_mock() {
        let llm = Box::new(MockProvider);
        let agent = AIAgentService::new(llm);
        
        let alert = create_test_alert();
        let context = create_test_context();
        
        let insight = agent.analyze_alert(&alert, &context).await.unwrap();
        assert_eq!(insight, "mock insight");
    }
}
```

### 8.2 集成测试

```rust
// tests/llm_integration.rs
// 需要设置环境变量：export ANTHROPIC_API_KEY=xxx

#[tokio::test]
#[ignore]  // 默认跳过
async fn test_anthropic_real_api() {
    let config = LLMConfig {
        provider: LLMProvider::Anthropic,
        model: "claude-sonnet-4-20251001".to_string(),
        api_key_env: "ANTHROPIC_API_KEY".to_string(),
    };
    
    let llm = AnthropicProvider::new(&config);
    let response = llm.ask("Hello").await.unwrap();
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

**结论**: 成本可控。

---

## 10. 后续扩展

### 10.1 Function Calling

```rust
// 未来可以扩展工具调用能力
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub struct Conversation {
    pub system: Option<String>,
    pub messages: Vec<Message>,
    pub tools: Option<Vec<ToolDefinition>>,  // 新增
}
```

### 10.2 流式响应

```rust
#[async_trait]
pub trait LLMClient {
    // ... 现有方法
    
    /// 流式响应
    async fn converse_stream(
        &self,
        conversation: Conversation
    ) -> Result<impl Stream<Item = Result<String>>>;
}
```

---

## 11. 参考

- [Anthropic Messages API](https://docs.anthropic.com/claude/reference/messages_post)
- [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat)
- [OpenAI Compatible API](https://platform.openai.com/docs/api-reference)
