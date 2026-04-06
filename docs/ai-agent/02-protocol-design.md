# AI Agent - 交互协议设计

**创建日期**: 2026-04-06  
**状态**: 设计中

---

## 1. 完整协议设计

### 1.1 设计原则

1. **以 OpenAI Chat Completion 和 Anthropic Messages API 为参考** - 两家 API 覆盖了主流 LLM 交互模式
2. **统一抽象，允许差异** - 核心字段统一，Provider 可以选择性支持高级参数
3. **多轮对话原生支持** - 消息历史、工具调用、多模态预留扩展

---

## 2. 核心类型定义

### 2.1 消息角色

```rust
/// 消息角色
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// 系统消息 - 设定行为和上下文
    System,
    /// 用户消息
    User,
    /// 助手消息
    Assistant,
    /// 工具响应消息
    Tool,
}
```

### 2.2 内容类型

```rust
/// 消息内容 - 支持文本和多模态
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// 纯文本
    Text(String),
    
    /// 多模态内容（预留扩展）
    MultiPart(Vec<ContentPart>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    Text { text: String },
    Image { image_url: ImageUrl },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,  // "auto", "low", "high"
}

impl From<String> for MessageContent {
    fn from(s: String) -> Self {
        MessageContent::Text(s)
    }
}

impl From<&str> for MessageContent {
    fn from(s: &str) -> Self {
        MessageContent::Text(s.to_string())
    }
}
```

### 2.3 工具调用

```rust
/// 工具定义
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolDefinition {
    /// 工具名称
    pub name: String,
    /// 工具描述
    pub description: Option<String>,
    /// 参数 schema (JSON Schema)
    pub parameters: Option<serde_json::Value>,
}

/// 工具选择策略
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    /// 自动决定是否使用工具
    Auto,
    /// 必须使用至少一个工具
    Required,
    /// 不使用工具
    None,
    /// 强制使用指定工具
    Specific { name: String },
}

/// 工具调用响应
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    /// 工具调用 ID（用于关联响应）
    pub id: String,
    /// 工具名称
    pub name: String,
    /// 工具参数 (JSON)
    pub arguments: String,
}

/// 工具响应
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolResponse {
    /// 关联的工具调用 ID
    pub call_id: String,
    /// 工具输出内容
    pub content: String,
}
```

### 2.4 消息结构

```rust
/// 对话消息
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    /// 消息角色
    pub role: MessageRole,
    /// 消息内容
    pub content: Option<MessageContent>,
    /// 工具调用列表（仅 Assistant 消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// 工具响应 ID（仅 Tool 消息）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// 消息名称（可选，用于标识特定用户/工具）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// 创建系统消息
    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::System,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    
    /// 创建用户消息
    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::User,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    
    /// 创建助手消息
    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }
    
    /// 创建带工具调用的助手消息
    pub fn assistant_with_tools(
        content: impl Into<MessageContent>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Some(content.into()),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            name: None,
        }
    }
    
    /// 创建工具响应消息
    pub fn tool(content: impl Into<MessageContent>, call_id: String) -> Self {
        Self {
            role: MessageRole::Tool,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: Some(call_id),
            name: None,
        }
    }
}
```

---

## 3. 对话请求

### 3.1 完整请求结构

```rust
/// 对话请求
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConversationRequest {
    /// 系统提示词（可选，某些 Provider 要求放在 messages 外）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    
    /// 对话历史
    pub messages: Vec<Message>,
    
    /// 模型参数
    #[serde(default)]
    pub model_config: ModelConfig,
    
    /// 工具定义列表
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    
    /// 工具选择策略
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    
    /// 流式响应
    #[serde(default)]
    pub stream: bool,
}

/// 模型参数配置
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelConfig {
    /// 最大生成 tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    
    /// 温度 (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    
    /// Top-p (核采样)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    
    /// Top-k
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    
    /// 频率惩罚 (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    
    /// 存在惩罚 (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    
    /// 停止序列
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    
    /// 随机种子（用于复现）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    
    /// 日志概率级别 (0 - 20)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<u32>,
}

impl ConversationRequest {
    /// 创建简单请求
    pub fn simple(prompt: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(prompt.into())],
            ..Default::default()
        }
    }
    
    /// 创建带系统提示词的请求
    pub fn with_system(system: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            system: Some(system.into()),
            messages: vec![Message::user(prompt.into())],
            ..Default::default()
        }
    }
    
    /// 创建多轮对话请求
    pub fn with_history(
        system: Option<String>,
        messages: Vec<Message>,
    ) -> Self {
        Self {
            system,
            messages,
            ..Default::default()
        }
    }
    
    // ========= 链式构建器 =========
    
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.model_config.max_tokens = Some(max);
        self
    }
    
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.model_config.temperature = Some(temp.clamp(0.0, 2.0));
        self
    }
    
    pub fn with_top_p(mut self, top_p: f64) -> Self {
        self.model_config.top_p = Some(top_p.clamp(0.0, 1.0));
        self
    }
    
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.model_config.top_k = Some(top_k);
        self
    }
    
    pub fn with_frequency_penalty(mut self, penalty: f64) -> Self {
        self.model_config.frequency_penalty = Some(penalty.clamp(-2.0, 2.0));
        self
    }
    
    pub fn with_presence_penalty(mut self, penalty: f64) -> Self {
        self.model_config.presence_penalty = Some(penalty.clamp(-2.0, 2.0));
        self
    }
    
    pub fn with_stop(mut self, stop: Vec<String>) -> Self {
        self.model_config.stop = Some(stop);
        self
    }
    
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.model_config.seed = Some(seed);
        self
    }
    
    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = Some(tools);
        self
    }
    
    pub fn with_tool_choice(mut self, choice: ToolChoice) -> Self {
        self.tool_choice = Some(choice);
        self
    }
    
    pub fn with_stream(mut self, stream: bool) -> Self {
        self.stream = stream;
        self
    }
}
```

---

## 4. 对话响应

### 4.1 完整响应结构

```rust
/// 对话响应
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConversationResponse {
    /// 生成的消息
    pub message: Message,
    
    /// 使用的模型
    pub model: String,
    
    /// Token 使用统计
    pub usage: TokenUsage,
    
    /// 完成原因
    pub finish_reason: FinishReason,
    
    /// 日志概率信息（如果请求了）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
    
    /// Provider 原始响应（用于调试）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// Token 使用统计
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TokenUsage {
    /// 输入 tokens
    pub prompt_tokens: u32,
    /// 输出 tokens
    pub completion_tokens: u32,
    /// 总 tokens
    pub total_tokens: u32,
    /// 缓存命中 tokens（某些 Provider 支持）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u32>,
}

/// 完成原因
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FinishReason {
    /// 完整响应
    Stop,
    /// 达到 max_tokens 限制
    Length,
    /// 调用了工具
    ToolCalls,
    /// 内容过滤
    ContentFilter,
    /// 其他原因
    Other,
}

/// 日志概率信息
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogProbs {
    /// 内容的日志概率
    pub content: Vec<LogProb>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogProb {
    /// token 文本
    pub token: String,
    /// 日志概率
    pub logprob: f64,
    /// 字节偏移
    pub bytes: Option<Vec<u8>>,
    /// 前 N 个备选 tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<TopLogProb>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopLogProb {
    pub token: String,
    pub logprob: f64,
    pub bytes: Option<Vec<u8>>,
}
```

---

## 5. 流式响应

### 5.1 流式事件

```rust
/// 流式响应事件
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamEvent {
    /// 事件 ID
    pub id: String,
    
    /// 事件类型
    pub event: StreamEventType,
    
    /// 事件数据
    pub data: StreamEventData,
}

/// 流式事件类型
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamEventType {
    /// 响应开始
    ResponseStart,
    /// 内容增量
    ContentDelta,
    /// 工具调用增量
    ToolCallDelta,
    /// Token 使用更新
    UsageUpdate,
    /// 响应完成
    ResponseComplete,
    /// 错误
    Error,
}

/// 流式事件数据
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StreamEventData {
    ContentDelta {
        delta: String,
    },
    ToolCallDelta {
        tool_call_index: usize,
        delta: ToolCallDelta,
    },
    UsageUpdate {
        usage: TokenUsage,
    },
    ResponseComplete {
        finish_reason: FinishReason,
    },
    Error {
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallDelta {
    pub id: Option<String>,
    pub name: Option<String>,
    pub arguments: Option<String>,
}

/// 流式响应接收器
pub struct StreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<StreamEvent, LLMError>>,
}

impl StreamReceiver {
    pub async fn recv(&mut self) -> Option<Result<StreamEvent, LLMError>> {
        self.rx.recv().await
    }
    
    /// 收集完整响应
    pub async fn collect(self) -> Result<ConversationResponse, LLMError> {
        let mut content = String::new();
        let mut tool_calls = Vec::new();
        let mut model = String::new();
        let mut finish_reason = FinishReason::Stop;
        let mut usage = None;
        
        let mut current_tool_call: Option<ToolCall> = None;
        
        let mut rx = self.rx;
        while let Some(event_result) = rx.recv().await {
            match event_result? {
                StreamEvent { data: StreamEventData::ContentDelta { delta }, .. } => {
                    content.push_str(&delta);
                }
                StreamEvent { data: StreamEventData::ToolCallDelta { delta, .. }, .. } => {
                    // 处理工具调用增量
                    if let Some(id) = delta.id {
                        current_tool_call = Some(ToolCall {
                            id,
                            name: delta.name.unwrap_or_default(),
                            arguments: delta.arguments.unwrap_or_default(),
                        });
                    } else if let Some(ref mut tc) = current_tool_call {
                        if let Some(args) = delta.arguments {
                            tc.arguments.push_str(&args);
                        }
                    }
                }
                StreamEvent { data: StreamEventData::UsageUpdate { usage: u }, .. } => {
                    usage = Some(u);
                }
                StreamEvent { data: StreamEventData::ResponseComplete { finish_reason: fr }, .. } => {
                    finish_reason = fr;
                }
                StreamEvent { data: StreamEventData::Error { code, message }, .. } => {
                    return Err(LLMError::Api {
                        status: 0,
                        message: format!("{}: {}", code, message),
                    });
                }
                _ => {}
            }
        }
        
        // 收集完成的工具调用
        if let Some(tc) = current_tool_call {
            tool_calls.push(tc);
        }
        
        let message = if tool_calls.is_empty() {
            Message::assistant(content)
        } else {
            Message::assistant_with_tools(content, tool_calls)
        };
        
        Ok(ConversationResponse {
            message,
            model,
            usage: usage.unwrap_or_default(),
            finish_reason,
            logprobs: None,
            raw: None,
        })
    }
}
```

---

## 6. LLM Client Trait

### 6.1 完整 Trait 定义

```rust
/// LLM Client Trait - 统一抽象层
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// 获取 Provider 名称
    fn provider(&self) -> LLMProvider;
    
    /// 获取配置的模型名
    fn model(&self) -> &str;
    
    /// 获取支持的参数列表
    fn supported_params(&self) -> &[SupportedParam];
    
    /// 发送对话请求
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse, LLMError>;
    
    /// 流式对话请求
    async fn converse_stream(
        &self,
        request: ConversationRequest,
    ) -> Result<StreamReceiver, LLMError>;
    
    /// 快捷方法：简单对话
    async fn ask(&self, prompt: impl Into<String>) -> Result<String, LLMError> {
        let response = self.converse(ConversationRequest::simple(prompt)).await?;
        Ok(response.message.content
            .unwrap_or(MessageContent::Text(String::new()))
            .as_str()
            .to_string())
    }
    
    /// 快捷方法：带系统提示词的对话
    async fn ask_with_system(
        &self,
        system: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Result<String, LLMError> {
        let response = self.converse(
            ConversationRequest::with_system(system, prompt)
        ).await?;
        Ok(response.message.content
            .unwrap_or(MessageContent::Text(String::new()))
            .as_str()
            .to_string())
    }
    
    /// 获取模型信息（可选）
    async fn get_model_info(&self) -> Result<Option<ModelInfo>, LLMError> {
        Ok(None)
    }
}

/// 支持的参数
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SupportedParam {
    MaxTokens,
    Temperature,
    TopP,
    TopK,
    FrequencyPenalty,
    PresencePenalty,
    Stop,
    Seed,
    LogProbs,
    Tools,
    Stream,
}

/// 模型信息
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub max_context_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
}
```

---

## 7. Provider 实现示例

### 7.1 Anthropic Provider

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    pub fn new(config: &LLMConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: std::env::var(&config.api_key_env)
                .expect("API key env var not set"),
            model: config.model.clone(),
            base_url: config.endpoint.clone().unwrap_or_else(|| 
                "https://api.anthropic.com".to_string()),
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
    
    fn supported_params(&self) -> &[SupportedParam] {
        &[
            SupportedParam::MaxTokens,
            SupportedParam::Temperature,
            SupportedParam::TopP,
            SupportedParam::Stream,
            SupportedParam::Tools,
            // Anthropic 不支持 top_k, frequency_penalty, presence_penalty
        ]
    }
    
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse, LLMError> {
        // ========== 协议转换：统一请求 → Anthropic 格式 ==========
        
        // Anthropic 要求 max_tokens 必填
        let max_tokens = request.model_config.max_tokens.unwrap_or(1024);
        
        // 转换消息 - Anthropic 有特殊的消息格式要求
        let anthropic_messages = self.convert_messages(&request.messages)?;
        
        // 构建请求体
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": anthropic_messages,
        });
        
        // System message 单独传（不能放在 messages 中）
        if let Some(system) = request.system {
            body["system"] = serde_json::Value::String(system);
        }
        
        // Temperature
        if let Some(temp) = request.model_config.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        
        // Top-p
        if let Some(top_p) = request.model_config.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        
        // Tools (Anthropic 格式)
        if let Some(tools) = request.tools {
            body["tools"] = self.convert_tools(&tools);
        }
        
        // ========== 发送请求 ==========
        let url = format!("{}/v1/messages", self.base_url);
        
        let response = self.client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;
        
        // 错误处理
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            
            // 解析 Anthropic 错误格式
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                let message = error_json["error"]["message"]
                    .as_str()
                    .unwrap_or(&error_text)
                    .to_string();
                return Err(LLMError::Api { status, message });
            }
            
            return Err(LLMError::Api { status, message: error_text });
        }
        
        // ========== 协议转换：Anthropic 响应 → 统一响应 ==========
        let result: serde_json::Value = response.json().await?;
        
        // 解析内容
        let content = result["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["text"].as_str())
            .unwrap_or("")
            .to_string();
        
        // 解析工具调用
        let tool_calls = result["content"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter(|item| item["type"].as_str() == Some("tool_use"))
                    .map(|item| ToolCall {
                        id: item["id"].as_str().unwrap_or("").to_string(),
                        name: item["name"].as_str().unwrap_or("").to_string(),
                        arguments: item["input"].to_string(),
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        
        // 解析 usage
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: (
                result["usage"]["input_tokens"].as_u64().unwrap_or(0) +
                result["usage"]["output_tokens"].as_u64().unwrap_or(0)
            ) as u32,
            cached_tokens: None,  // Anthropic 暂未支持
        };
        
        // 解析完成原因
        let finish_reason = match result["stop_reason"].as_str() {
            Some("end_turn") | Some("stop_sequence") => FinishReason::Stop,
            Some("max_tokens") => FinishReason::Length,
            Some("tool_use") => FinishReason::ToolCalls,
            _ => FinishReason::Other,
        };
        
        let message = if tool_calls.is_empty() {
            Message::assistant(content)
        } else {
            Message::assistant_with_tools(content, tool_calls)
        };
        
        Ok(ConversationResponse {
            message,
            model: result["model"].as_str().unwrap_or(&self.model).to_string(),
            usage,
            finish_reason,
            logprobs: None,  // Anthropic 暂未支持
            raw: Some(result),
        })
    }
    
    async fn converse_stream(
        &self,
        request: ConversationRequest,
    ) -> Result<StreamReceiver, LLMError> {
        // 实现流式响应逻辑
        // 使用 SSE (Server-Sent Events) 接收响应
        todo!()
    }
}

impl AnthropicProvider {
    /// 转换消息为 Anthropic 格式
    fn convert_messages(&self, messages: &[Message]) -> Result<Vec<serde_json::Value>, LLMError> {
        let mut result = Vec::new();
        
        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // Anthropic: system 不能放在 messages 中
                    // 这里跳过，由上层处理
                }
                MessageRole::User => {
                    result.push(serde_json::json!({
                        "role": "user",
                        "content": msg.content.as_ref().map(|c| c.as_str()).unwrap_or(""),
                    }));
                }
                MessageRole::Assistant => {
                    let mut content = Vec::new();
                    
                    // 文本内容
                    if let Some(ref c) = msg.content {
                        content.push(serde_json::json!({
                            "type": "text",
                            "text": c.as_str(),
                        }));
                    }
                    
                    // 工具调用
                    if let Some(ref tools) = msg.tool_calls {
                        for tool in tools {
                            content.push(serde_json::json!({
                                "type": "tool_use",
                                "id": tool.id,
                                "name": tool.name,
                                "input": serde_json::from_str::<serde_json::Value>(&tool.arguments)
                                    .unwrap_or(serde_json::json!({})),
                            }));
                        }
                    }
                    
                    result.push(serde_json::json!({
                        "role": "assistant",
                        "content": content,
                    }));
                }
                MessageRole::Tool => {
                    // Anthropic 工具响应格式
                    result.push(serde_json::json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.tool_call_id,
                            "content": msg.content.as_ref().map(|c| c.as_str()).unwrap_or(""),
                        }],
                    }));
                }
            }
        }
        
        Ok(result)
    }
    
    /// 转换工具定义为 Anthropic 格式
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        serde_json::json!(tools.iter().map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters.as_ref().unwrap_or(&serde_json::json!({
                    "type": "object",
                    "properties": {}
                })),
            })
        }).collect::<Vec<_>>())
    }
}
```

### 7.2 OpenAI Provider

```rust
pub struct OpenAIProvider {
    client: reqwest::Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    pub fn new(config: &LLMConfig) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: std::env::var(&config.api_key_env)
                .expect("API key env var not set"),
            model: config.model.clone(),
            base_url: config.endpoint.clone().unwrap_or_else(|| 
                "https://api.openai.com/v1".to_string()),
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
    
    fn supported_params(&self) -> &[SupportedParam] {
        &[
            SupportedParam::MaxTokens,
            SupportedParam::Temperature,
            SupportedParam::TopP,
            SupportedParam::FrequencyPenalty,
            SupportedParam::PresencePenalty,
            SupportedParam::Stop,
            SupportedParam::Seed,
            SupportedParam::LogProbs,
            SupportedParam::Stream,
            SupportedParam::Tools,
            // OpenAI 不支持 top_k
        ]
    }
    
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse, LLMError> {
        // ========== 协议转换：统一请求 → OpenAI 格式 ==========
        
        let mut messages = Vec::new();
        
        // System message 作为第一条
        if let Some(system) = request.system {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system,
            }));
        }
        
        // 对话历史
        for msg in &request.messages {
            messages.push(self.convert_message(msg)?);
        }
        
        // 构建请求体
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": messages,
        });
        
        // Max tokens
        if let Some(max) = request.model_config.max_tokens {
            body["max_tokens"] = serde_json::json!(max);
        }
        
        // Temperature
        if let Some(temp) = request.model_config.temperature {
            body["temperature"] = serde_json::json!(temp);
        }
        
        // Top-p
        if let Some(top_p) = request.model_config.top_p {
            body["top_p"] = serde_json::json!(top_p);
        }
        
        // Frequency penalty
        if let Some(fp) = request.model_config.frequency_penalty {
            body["frequency_penalty"] = serde_json::json!(fp);
        }
        
        // Presence penalty
        if let Some(pp) = request.model_config.presence_penalty {
            body["presence_penalty"] = serde_json::json!(pp);
        }
        
        // Stop sequences
        if let Some(stop) = request.model_config.stop {
            body["stop"] = serde_json::json!(stop);
        }
        
        // Seed
        if let Some(seed) = request.model_config.seed {
            body["seed"] = serde_json::json!(seed);
        }
        
        // Logprobs
        if let Some(logprobs) = request.model_config.logprobs {
            body["logprobs"] = serde_json::json!(true);
            body["top_logprobs"] = serde_json::json!(logprobs);
        }
        
        // Tools
        if let Some(tools) = request.tools {
            body["tools"] = self.convert_tools(&tools);
        }
        
        // Tool choice
        if let Some(choice) = request.tool_choice {
            body["tool_choice"] = match choice {
                ToolChoice::Auto => serde_json::json!("auto"),
                ToolChoice::Required => serde_json::json!("required"),
                ToolChoice::None => serde_json::json!("none"),
                ToolChoice::Specific { name } => serde_json::json!({
                    "type": "function",
                    "function": { "name": name }
                }),
            };
        }
        
        // Stream
        if request.stream {
            body["stream"] = serde_json::json!(true);
        }
        
        // ========== 发送请求 ==========
        let url = format!("{}/chat/completions", self.base_url);
        
        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;
        
        // 错误处理
        if !response.status().is_success() {
            let status = response.status().as_u16();
            let error_text = response.text().await.unwrap_or_default();
            
            if let Ok(error_json) = serde_json::from_str::<serde_json::Value>(&error_text) {
                let message = error_json["error"]["message"]
                    .as_str()
                    .unwrap_or(&error_text)
                    .to_string();
                return Err(LLMError::Api { status, message });
            }
            
            return Err(LLMError::Api { status, message: error_text });
        }
        
        // ========== 协议转换：OpenAI 响应 → 统一响应 ==========
        let result: serde_json::Value = response.json().await?;
        
        let choices = result["choices"].as_array().unwrap_or(&vec![]);
        let first_choice = choices.first();
        
        // 解析消息
        let message_content = first_choice
            .and_then(|c| c["message"]["content"].as_str())
            .unwrap_or("")
            .to_string();
        
        let tool_calls = first_choice
            .and_then(|c| c["message"]["tool_calls"].as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|tc| {
                        let function = &tc["function"];
                        Some(ToolCall {
                            id: tc["id"].as_str().unwrap_or("").to_string(),
                            name: function["name"].as_str().unwrap_or("").to_string(),
                            arguments: function["arguments"].as_str().unwrap_or("").to_string(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        
        // 解析 usage
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: result["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            cached_tokens: result["usage"]["prompt_tokens_details"]["cached_tokens"]
                .as_u64()
                .map(|v| v as u32),
        };
        
        // 解析完成原因
        let finish_reason = match first_choice.and_then(|c| c["finish_reason"].as_str()) {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("tool_calls") | Some("function_call") => FinishReason::ToolCalls,
            Some("content_filter") => FinishReason::ContentFilter,
            _ => FinishReason::Other,
        };
        
        // 解析 logprobs
        let logprobs = first_choice
            .and_then(|c| c["logprobs"].as_object())
            .map(|_| LogProbs {
                content: vec![],  // 简化处理
            });
        
        let message = if tool_calls.is_empty() {
            Message::assistant(message_content)
        } else {
            Message::assistant_with_tools(message_content, tool_calls)
        };
        
        Ok(ConversationResponse {
            message,
            model: result["model"].as_str().unwrap_or(&self.model).to_string(),
            usage,
            finish_reason,
            logprobs,
            raw: Some(result),
        })
    }
    
    async fn converse_stream(
        &self,
        request: ConversationRequest,
    ) -> Result<StreamReceiver, LLMError> {
        // 实现流式响应逻辑
        todo!()
    }
}

impl OpenAIProvider {
    /// 转换消息为 OpenAI 格式
    fn convert_message(&self, msg: &Message) -> Result<serde_json::Value, LLMError> {
        let mut result = serde_json::json!({
            "role": match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            },
        });
        
        // 内容
        if let Some(ref content) = msg.content {
            result["content"] = serde_json::json!(content.as_str());
        }
        
        // 工具调用
        if let Some(ref tool_calls) = msg.tool_calls {
            result["tool_calls"] = serde_json::json!(tool_calls.iter().map(|tc| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }
                })
            }).collect::<Vec<_>>());
        }
        
        // 工具响应 ID
        if let Some(ref tool_call_id) = msg.tool_call_id {
            result["tool_call_id"] = serde_json::json!(tool_call_id);
        }
        
        // 名称
        if let Some(ref name) = msg.name {
            result["name"] = serde_json::json!(name);
        }
        
        Ok(result)
    }
    
    /// 转换工具定义为 OpenAI 格式
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        serde_json::json!(tools.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters.as_ref().unwrap_or(&serde_json::json!({
                        "type": "object",
                        "properties": {}
                    })),
                }
            })
        }).collect::<Vec<_>>())
    }
}
```

---

## 8. 错误处理

### 8.1 错误类型

```rust
#[derive(thiserror::Error, Debug)]
pub enum LLMError {
    /// 网络错误
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),
    
    /// API 错误
    #[error("API error ({status}): {message}")]
    Api { 
        status: u16, 
        message: String,
    },
    
    /// 认证失败
    #[error("Authentication failed: {0}")]
    Auth(String),
    
    /// 速率限制
    #[error("Rate limit exceeded. Retry after {retry_after:?}")]
    RateLimit { 
        retry_after: Option<std::time::Duration>,
    },
    
    /// 响应解析错误
    #[error("Invalid response format: {0}")]
    Parse(String),
    
    /// 超时
    #[error("Request timeout: {0}")]
    Timeout(String),
    
    /// 不支持的参数
    #[error("Parameter '{param}' is not supported by this provider")]
    UnsupportedParam { param: String },
    
    /// 工具调用错误
    #[error("Tool call error: {0}")]
    ToolCall(String),
    
    /// 内容过滤
    #[error("Content was filtered: {reason}")]
    ContentFiltered { reason: String },
}

/// 重试配置
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: std::time::Duration,
    pub max_delay: std::time::Duration,
    pub exponential_base: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: std::time::Duration::from_secs(1),
            max_delay: std::time::Duration::from_secs(30),
            exponential_base: 2.0,
        }
    }
}

/// 带重试的请求
pub async fn converse_with_retry(
    client: &dyn LLMClient,
    request: ConversationRequest,
    config: &RetryConfig,
) -> Result<ConversationResponse, LLMError> {
    let mut delay = config.initial_delay;
    
    for attempt in 0..config.max_retries {
        match client.converse(request.clone()).await {
            Ok(response) => return Ok(response),
            Err(e) => {
                // 只有特定错误才重试
                let should_retry = match &e {
                    LLMError::Network(_) => true,
                    LLMError::RateLimit { .. } => true,
                    LLMError::Api { status, .. } => {
                        // 5xx 错误可重试
                        *status >= 500 && *status < 600
                    }
                    _ => false,
                };
                
                if !should_retry || attempt >= config.max_retries - 1 {
                    return Err(e);
                }
                
                // 指数退避
                tokio::time::sleep(delay).await;
                delay = std::cmp::min(
                    delay.mul_f64(config.exponential_base),
                    config.max_delay,
                );
            }
        }
    }
    
    unreachable!()
}
```

---

## 9. 使用示例

### 9.1 基础使用

```rust
use vol_agent::{
    AnthropicProvider, OpenAIProvider, 
    LLMClient, LLMConfig, LLMProvider,
    ConversationRequest, Message, ModelConfig,
};

// 创建 Provider
let config = LLMConfig {
    provider: LLMProvider::Anthropic,
    model: "claude-sonnet-4-20251001".to_string(),
    api_key_env: "ANTHROPIC_API_KEY".to_string(),
    endpoint: None,
};

let llm: Box<dyn LLMClient> = Box::new(AnthropicProvider::new(&config));

// 简单对话
let response = llm.ask("你好").await?;
println!("{}", response);

// 带参数的对话
let request = ConversationRequest::simple("分析这个告警")
    .with_max_tokens(500)
    .with_temperature(0.7)
    .with_top_p(0.9);

let response = llm.converse(request).await?;
println!("内容：{}", response.message.content.unwrap().as_str());
println!("Token 使用：{:?}", response.usage);
println!("完成原因：{:?}", response.finish_reason);
```

### 9.2 多轮对话

```rust
let messages = vec![
    Message::system("你是一个专业的交易助手。"),
    Message::user("ETH IV 100% 意味着什么？"),
    Message::assistant("IV 100% 表示隐含波动率处于极高水平..."),
    Message::user("那我应该怎么操作？"),
];

let request = ConversationRequest::with_history(None, messages)
    .with_temperature(0.7);

let response = llm.converse(request).await?;
```

### 9.3 工具调用

```rust
let tools = vec![ToolDefinition {
    name: "get_historical_iv".to_string(),
    description: Some("获取历史 IV 数据".to_string()),
    parameters: Some(serde_json::json!({
        "type": "object",
        "properties": {
            "symbol": {"type": "string"},
            "days": {"type": "integer"},
        },
        "required": ["symbol", "days"],
    })),
}];

let request = ConversationRequest::simple("查询 BTC 过去 30 天的 IV")
    .with_tools(tools)
    .with_tool_choice(ToolChoice::Auto);

let response = llm.converse(request).await?;

// 处理工具调用
if let Some(tool_calls) = response.message.tool_calls {
    for call in tool_calls {
        println!("调用工具：{}", call.name);
        println!("参数：{}", call.arguments);
        
        // 执行工具...
        let result = execute_tool(&call.name, &call.arguments).await?;
        
        // 发送工具响应
        let messages = vec![
            Message::user("查询 BTC 过去 30 天的 IV"),
            response.message.clone(),
            Message::tool(result, call.id),
        ];
        
        let followup = ConversationRequest::with_history(None, messages);
        let final_response = llm.converse(followup).await?;
    }
}
```

### 9.4 流式响应

```rust
let request = ConversationRequest::simple("写一首诗")
    .with_stream(true);

let mut stream = llm.converse_stream(request).await?;

while let Some(event) = stream.recv().await {
    match event? {
        StreamEvent { data: StreamEventData::ContentDelta { delta }, .. } => {
            print!("{}", delta);
            stdout().flush()?;
        }
        StreamEvent { data: StreamEventData::ResponseComplete { finish_reason }, .. } => {
            println!("\n完成：{:?}", finish_reason);
        }
        _ => {}
    }
}
```

---

## 10. 参考

- [OpenAI Chat Completions API](https://platform.openai.com/docs/api-reference/chat)
- [Anthropic Messages API](https://docs.anthropic.com/claude/reference/messages_post)
- [JSON Schema](https://json-schema.org/)
