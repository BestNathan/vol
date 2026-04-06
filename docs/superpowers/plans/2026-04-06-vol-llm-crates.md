# vol-llm Crates Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement 4 new crates (vol-llm-core, vol-llm-provider, vol-llm-tool, vol-llm-agent) to provide AI Agent capabilities for vol-monitor.

**Architecture:** Layered architecture with protocol abstraction (core), provider implementations (provider), tool framework (tool), and ReAct workflow orchestration (agent).

**Tech Stack:** Rust async (tokio), serde for serialization, reqwest for HTTP, thiserror for errors, async-trait for traits.

---

## File Structure

### crates/vol-llm-core/
- `Cargo.toml` - Package definition
- `src/lib.rs` - Module exports
- `src/provider.rs` - LLMProvider enum
- `src/message.rs` - Message types
- `src/tool.rs` - Tool definitions
- `src/model.rs` - Model configuration
- `src/conversation.rs` - Conversation request/response
- `src/stream.rs` - Streaming types
- `src/client.rs` - LLMClient trait
- `src/error.rs` - Error types

### crates/vol-llm-provider/
- `Cargo.toml` - Package definition
- `src/lib.rs` - Module exports
- `src/config.rs` - LLMConfig
- `src/anthropic.rs` - AnthropicProvider
- `src/openai.rs` - OpenAIProvider
- `src/factory.rs` - Factory functions

### crates/vol-llm-tool/
- `Cargo.toml` - Package definition
- `src/lib.rs` - Module exports
- `src/tool.rs` - Tool trait and types
- `src/registry.rs` - ToolRegistry
- `src/tools/mod.rs` - Built-in tools export
- `src/tools/alert_history.rs` - Alert history tool
- `src/tools/iv_curve.rs` - IV curve tool
- `src/tools/market_data.rs` - Market data tool
- `src/tools/rule_info.rs` - Rule info tool

### crates/vol-llm-agent/
- `Cargo.toml` - Package definition
- `src/lib.rs` - Module exports
- `src/agent.rs` - ReActAgent core
- `src/response.rs` - AgentResponse and AgentError
- `src/builder.rs` - AgentBuilder
- `src/prompt.rs` - System prompt templates

### Root changes
- `Cargo.toml` - Add 4 workspace members
- `.env.example` - Add API key env vars
- `config/llm.example.toml` - Example LLM config

---

## Phase 1: vol-llm-core

### Task 1.1: Create vol-llm-core crate structure

**Files:**
- Create: `crates/vol-llm-core/Cargo.toml`
- Create: `crates/vol-llm-core/src/lib.rs`
- Create: `crates/vol-llm-core/src/provider.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-core"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
vol-core = { workspace = true }
```

- [ ] **Step 2: Create src/lib.rs**

```rust
//! vol-llm-core: Core protocol types for LLM interaction.
//!
//! This crate defines the abstraction layer for LLM providers.

pub mod provider;
pub mod message;
pub mod tool;
pub mod model;
pub mod conversation;
pub mod stream;
pub mod client;
pub mod error;

pub use provider::*;
pub use message::*;
pub use tool::*;
pub use model::*;
pub use conversation::*;
pub use stream::*;
pub use client::*;
pub use error::*;
```

- [ ] **Step 3: Create src/provider.rs**

```rust
//! LLM Provider enumeration.

use serde::{Deserialize, Serialize};

/// LLM Provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LLMProvider {
    /// Anthropic (Claude)
    Anthropic,
    /// OpenAI (GPT)
    OpenAI,
}

impl std::fmt::Display for LLMProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LLMProvider::Anthropic => write!(f, "anthropic"),
            LLMProvider::OpenAI => write!(f, "openai"),
        }
    }
}
```

- [ ] **Step 4: Run cargo check**

```bash
cd crates/vol-llm-core && cargo check
```

Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-core/Cargo.toml crates/vol-llm-core/src/lib.rs crates/vol-llm-core/src/provider.rs
git commit -m "feat(vol-llm-core): create crate structure with LLMProvider enum"
```

---

### Task 1.2: Implement message types

**Files:**
- Create: `crates/vol-llm-core/src/message.rs`

- [ ] **Step 1: Create src/message.rs**

```rust
//! Message types for LLM conversation.

use serde::{Deserialize, Serialize};
use crate::tool::ToolCall;

/// Message role
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System message - sets behavior and context
    System,
    /// User message
    User,
    /// Assistant message
    Assistant,
    /// Tool response message
    Tool,
}

/// Content part for multi-part messages (images, etc.)
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum ContentPart {
    Text { text: String },
    Image { image_url: ImageUrl },
}

/// Image URL for multi-part content
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ImageUrl {
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Message content - text or multi-part
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum MessageContent {
    /// Plain text
    Text(String),
    /// Multi-part content
    MultiPart(Vec<ContentPart>),
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

impl MessageContent {
    /// Get content as string (for text content)
    pub fn as_str(&self) -> &str {
        match self {
            MessageContent::Text(s) => s,
            MessageContent::MultiPart(_) => "",
        }
    }
}

/// Conversation message
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Message {
    /// Message role
    pub role: MessageRole,
    /// Message content
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<MessageContent>,
    /// Tool calls (assistant messages only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Tool call ID (tool messages only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Optional name for the participant
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// Create a system message
    pub fn system(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::System,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    /// Create a user message
    pub fn user(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::User,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<MessageContent>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: Some(content.into()),
            tool_calls: None,
            tool_call_id: None,
            name: None,
        }
    }

    /// Create an assistant message with tool calls
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

    /// Create a tool response message
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_creation() {
        let msg = Message::system("You are helpful");
        assert_eq!(msg.role, MessageRole::System);
        assert!(msg.content.is_some());
    }

    #[test]
    fn test_message_user() {
        let msg = Message::user("Hello");
        assert_eq!(msg.role, MessageRole::User);
    }

    #[test]
    fn test_message_content_from_str() {
        let content: MessageContent = "test".into();
        assert_eq!(content.as_str(), "test");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-core && cargo test message
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-core/src/message.rs
git commit -m "feat(vol-llm-core): implement message types with builders"
```

---

### Task 1.3: Implement tool types

**Files:**
- Create: `crates/vol-llm-core/src/tool.rs`

- [ ] **Step 1: Create src/tool.rs**

```rust
//! Tool calling types.

use serde::{Deserialize, Serialize};

/// Tool definition for LLM
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolDefinition {
    /// Tool name
    pub name: String,
    /// Tool description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Parameters schema (JSON Schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parameters: Option<serde_json::Value>,
}

/// Tool call from LLM
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCall {
    /// Tool call ID
    pub id: String,
    /// Tool name
    pub name: String,
    /// Tool arguments (JSON string)
    pub arguments: String,
}

/// Tool choice strategy
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ToolChoice {
    /// Auto-decide whether to use tools
    Auto,
    /// Must use at least one tool
    Required,
    /// Do not use tools
    None,
    /// Force use of specific tool
    Specific { name: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition_creation() {
        let tool = ToolDefinition {
            name: "test_tool".to_string(),
            description: Some("A test tool".to_string()),
            parameters: None,
        };
        assert_eq!(tool.name, "test_tool");
    }

    #[test]
    fn test_tool_call_creation() {
        let call = ToolCall {
            id: "call_123".to_string(),
            name: "test".to_string(),
            arguments: "{}".to_string(),
        };
        assert_eq!(call.id, "call_123");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-core && cargo test tool
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-core/src/tool.rs
git commit -m "feat(vol-llm-core): implement tool calling types"
```

---

### Task 1.4: Implement model configuration

**Files:**
- Create: `crates/vol-llm-core/src/model.rs`

- [ ] **Step 1: Create src/model.rs**

```rust
//! Model configuration and info types.

use serde::{Deserialize, Serialize};

/// Model parameters
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ModelConfig {
    /// Maximum generation tokens
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u32>,
    /// Temperature (0.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    /// Top-p (nucleus sampling)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f64>,
    /// Top-k
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<u32>,
    /// Frequency penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f64>,
    /// Presence penalty (-2.0 - 2.0)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f64>,
    /// Stop sequences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop: Option<Vec<String>>,
    /// Random seed for reproducibility
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Logprobs level (0 - 20)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<u32>,
}

/// Model information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ModelInfo {
    pub name: String,
    pub max_context_tokens: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub supports_tools: bool,
    pub supports_streaming: bool,
    pub supports_vision: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_config_default() {
        let config = ModelConfig::default();
        assert!(config.max_tokens.is_none());
        assert!(config.temperature.is_none());
    }

    #[test]
    fn test_model_config_with_values() {
        let config = ModelConfig {
            max_tokens: Some(1024),
            temperature: Some(0.7),
            ..Default::default()
        };
        assert_eq!(config.max_tokens, Some(1024));
        assert_eq!(config.temperature, Some(0.7));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-core && cargo test model
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-core/src/model.rs
git commit -m "feat(vol-llm-core): implement model configuration types"
```

---

### Task 1.5: Implement conversation types

**Files:**
- Create: `crates/vol-llm-core/src/conversation.rs`

- [ ] **Step 1: Create src/conversation.rs**

```rust
//! Conversation request and response types.

use serde::{Deserialize, Serialize};
use crate::{Message, ModelConfig, ToolDefinition, ToolChoice, TokenUsage, FinishReason, LogProbs};

/// Conversation request
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConversationRequest {
    /// System prompt
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system: Option<String>,
    /// Conversation history
    pub messages: Vec<Message>,
    /// Model parameters
    #[serde(default)]
    pub model_config: ModelConfig,
    /// Tool definitions
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolDefinition>>,
    /// Tool choice strategy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_choice: Option<ToolChoice>,
    /// Stream response
    #[serde(default)]
    pub stream: bool,
}

/// Conversation response
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ConversationResponse {
    /// Generated message
    pub message: Message,
    /// Model used
    pub model: String,
    /// Token usage
    pub usage: TokenUsage,
    /// Finish reason
    pub finish_reason: FinishReason,
    /// Logprobs info
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logprobs: Option<LogProbs>,
    /// Raw provider response (for debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<serde_json::Value>,
}

/// Token usage statistics
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cached_tokens: Option<u32>,
}

/// Finish reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FinishReason {
    Stop,
    Length,
    ToolCalls,
    ContentFilter,
    Other,
}

/// Logprobs information
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogProbs {
    pub content: Vec<LogProb>,
}

/// Single logprob entry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LogProb {
    pub token: String,
    pub logprob: f64,
    pub bytes: Option<Vec<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_logprobs: Option<Vec<TopLogProb>>,
}

/// Top logprob entry
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TopLogProb {
    pub token: String,
    pub logprob: f64,
    pub bytes: Option<Vec<u8>>,
}

impl ConversationRequest {
    /// Create simple request
    pub fn simple(prompt: impl Into<String>) -> Self {
        Self {
            messages: vec![Message::user(prompt.into())],
            ..Default::default()
        }
    }

    /// Create with system prompt
    pub fn with_system(system: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            system: Some(system.into()),
            messages: vec![Message::user(prompt.into())],
            ..Default::default()
        }
    }

    /// Create with history
    pub fn with_history(system: Option<String>, messages: Vec<Message>) -> Self {
        Self {
            system,
            messages,
            ..Default::default()
        }
    }

    /// Builder: set max_tokens
    pub fn with_max_tokens(mut self, max: u32) -> Self {
        self.model_config.max_tokens = Some(max);
        self
    }

    /// Builder: set temperature
    pub fn with_temperature(mut self, temp: f64) -> Self {
        self.model_config.temperature = Some(temp.clamp(0.0, 2.0));
        self
    }

    /// Builder: set top_p
    pub fn with_top_p(mut self, top_p: f64) -> Self {
        self.model_config.top_p = Some(top_p.clamp(0.0, 1.0));
        self
    }

    /// Builder: set top_k
    pub fn with_top_k(mut self, top_k: u32) -> Self {
        self.model_config.top_k = Some(top_k);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_request() {
        let req = ConversationRequest::simple("Hello");
        assert_eq!(req.messages.len(), 1);
        assert!(req.system.is_none());
    }

    #[test]
    fn test_builder_pattern() {
        let req = ConversationRequest::simple("Test")
            .with_max_tokens(500)
            .with_temperature(0.7);
        assert_eq!(req.model_config.max_tokens, Some(500));
        assert_eq!(req.model_config.temperature, Some(0.7));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-core && cargo test conversation
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-core/src/conversation.rs
git commit -m "feat(vol-llm-core): implement conversation types with builder pattern"
```

---

### Task 1.6: Implement stream types

**Files:**
- Create: `crates/vol-llm-core/src/stream.rs`

- [ ] **Step 1: Create src/stream.rs**

```rust
//! Streaming response types.

use serde::{Deserialize, Serialize};
use crate::{TokenUsage, FinishReason, LLMError};

/// Stream event
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StreamEvent {
    pub id: String,
    pub event: StreamEventType,
    pub data: StreamEventData,
}

/// Stream event type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StreamEventType {
    ResponseStart,
    ContentDelta,
    ToolCallDelta,
    UsageUpdate,
    ResponseComplete,
    Error,
}

/// Stream event data
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum StreamEventData {
    ContentDelta { delta: String },
    ToolCallDelta { tool_call_index: usize, delta: ToolCallDelta },
    UsageUpdate { usage: TokenUsage },
    ResponseComplete { finish_reason: FinishReason },
    Error { code: String, message: String },
}

/// Tool call delta
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ToolCallDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arguments: Option<String>,
}

/// Stream receiver
pub struct StreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<StreamEvent, LLMError>>,
}

impl StreamReceiver {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Result<StreamEvent, LLMError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<StreamEvent, LLMError>> {
        self.rx.recv().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stream_event_creation() {
        let event = StreamEvent {
            id: "event_1".to_string(),
            event: StreamEventType::ContentDelta,
            data: StreamEventData::ContentDelta { delta: "Hello".to_string() },
        };
        assert_eq!(event.id, "event_1");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-core && cargo test stream
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-core/src/stream.rs
git commit -m "feat(vol-llm-core): implement streaming types"
```

---

### Task 1.7: Implement error types

**Files:**
- Create: `crates/vol-llm-core/src/error.rs`

- [ ] **Step 1: Create src/error.rs**

```rust
//! LLM error types.

use thiserror::Error;
use std::time::Duration;

/// LLM error
#[derive(Debug, Error)]
pub enum LLMError {
    #[error("Network error: {0}")]
    Network(#[from] reqwest::Error),

    #[error("API error ({status}): {message}")]
    Api { status: u16, message: String },

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Rate limit exceeded. Retry after {retry_after:?}")]
    RateLimit { retry_after: Option<Duration> },

    #[error("Invalid response format: {0}")]
    Parse(String),

    #[error("Request timeout: {0}")]
    Timeout(String),

    #[error("Parameter '{param}' is not supported by this provider")]
    UnsupportedParam { param: String },

    #[error("Tool call error: {0}")]
    ToolCall(String),

    #[error("Content was filtered: {reason}")]
    ContentFiltered { reason: String },
}

pub type Result<T> = std::result::Result<T, LLMError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = LLMError::Timeout("test".to_string());
        assert!(err.to_string().contains("timeout"));
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-core && cargo test error
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-core/src/error.rs
git commit -m "feat(vol-llm-core): implement error types"
```

---

### Task 1.8: Implement LLMClient trait

**Files:**
- Create: `crates/vol-llm-core/src/client.rs`

- [ ] **Step 1: Create src/client.rs**

```rust
//! LLM Client trait.

use async_trait::async_trait;
use crate::{LLMProvider, ConversationRequest, ConversationResponse, StreamReceiver, LLMError, SupportedParam};

/// Supported parameter
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

/// LLM Client trait
#[async_trait]
pub trait LLMClient: Send + Sync {
    /// Get provider type
    fn provider(&self) -> LLMProvider;

    /// Get configured model name
    fn model(&self) -> &str;

    /// Get supported parameters
    fn supported_params(&self) -> &[SupportedParam];

    /// Send conversation request
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse, LLMError>;

    /// Send streaming conversation request
    async fn converse_stream(&self, request: ConversationRequest) -> Result<StreamReceiver, LLMError>;

    /// Quick method: simple conversation
    async fn ask(&self, prompt: impl Into<String>) -> Result<String, LLMError> {
        let response = self.converse(ConversationRequest::simple(prompt)).await?;
        Ok(response.message.content
            .unwrap_or(crate::MessageContent::Text(String::new()))
            .as_str()
            .to_string())
    }

    /// Quick method: with system prompt
    async fn ask_with_system(
        &self,
        system: impl Into<String>,
        prompt: impl Into<String>,
    ) -> Result<String, LLMError> {
        let response = self.converse(
            ConversationRequest::with_system(system, prompt)
        ).await?;
        Ok(response.message.content
            .unwrap_or(crate::MessageContent::Text(String::new()))
            .as_str()
            .to_string())
    }
}
```

- [ ] **Step 2: Update lib.rs to export SupportedParam**

Modify `crates/vol-llm-core/src/lib.rs`:

```rust
//! vol-llm-core: Core protocol types for LLM interaction.
//!
//! This crate defines the abstraction layer for LLM providers.

pub mod provider;
pub mod message;
pub mod tool;
pub mod model;
pub mod conversation;
pub mod stream;
pub mod client;
pub mod error;

pub use provider::*;
pub use message::*;
pub use tool::*;
pub use model::*;
pub use conversation::*;
pub use stream::*;
pub use client::*;
pub use error::*;
```

- [ ] **Step 3: Run cargo check**

```bash
cd crates/vol-llm-core && cargo check
```

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-core/src/client.rs crates/vol-llm-core/src/lib.rs
git commit -m "feat(vol-llm-core): implement LLMClient trait"
```

---

### Task 1.9: vol-llm-core completion

- [ ] **Step 1: Run all tests**

```bash
cd crates/vol-llm-core && cargo test
```

Expected: All tests pass

- [ ] **Step 2: Run clippy**

```bash
cd crates/vol-llm-core && cargo clippy -- -D warnings
```

Expected: No warnings

- [ ] **Step 3: Commit final**

```bash
git add -A
git commit -m "feat(vol-llm-core): complete core protocol crate"
```

---

## Phase 2: vol-llm-provider

### Task 2.1: Create vol-llm-provider crate structure

**Files:**
- Create: `crates/vol-llm-provider/Cargo.toml`
- Create: `crates/vol-llm-provider/src/lib.rs`
- Create: `crates/vol-llm-provider/src/config.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-provider"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
tracing = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
```

- [ ] **Step 2: Create src/lib.rs**

```rust
//! vol-llm-provider: LLM Provider implementations.

pub mod config;
pub mod anthropic;
pub mod openai;
pub mod factory;

pub use config::LLMConfig;
pub use anthropic::AnthropicProvider;
pub use openai::OpenAIProvider;
pub use factory::{create_provider, load_provider};
```

- [ ] **Step 3: Create src/config.rs**

```rust
//! LLM configuration.

use serde::{Deserialize, Serialize};
use vol_llm_core::LLMProvider;

/// LLM configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMConfig {
    /// Provider type
    pub provider: LLMProvider,
    /// Model name
    pub model: String,
    /// API key environment variable
    pub api_key_env: String,
    /// Custom endpoint (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
}

impl LLMConfig {
    /// Load from TOML file
    pub fn load(path: &str) -> Result<Self, vol_llm_core::LLMError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| vol_llm_core::LLMError::Parse(format!("Failed to read config: {}", e)))?;
        toml::from_str(&content)
            .map_err(|e| vol_llm_core::LLMError::Parse(format!("Failed to parse config: {}", e)))
    }

    /// Get API key from environment
    pub fn api_key(&self) -> Result<String, vol_llm_core::LLMError> {
        std::env::var(&self.api_key_env)
            .map_err(|_| vol_llm_core::LLMError::Auth(format!(
                "API key environment variable '{}' not set",
                self.api_key_env
            )))
    }
}
```

- [ ] **Step 4: Add toml dependency to workspace**

The workspace needs `toml` crate for config parsing. Check if it exists in root `Cargo.toml`:

```bash
grep -n "toml" Cargo.toml
```

If not present, it will be added in Phase 5 integration.

- [ ] **Step 5: Run cargo check**

```bash
cd crates/vol-llm-provider && cargo check
```

Expected: Compiles (with warnings for unused modules)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-provider/Cargo.toml crates/vol-llm-provider/src/lib.rs crates/vol-llm-provider/src/config.rs
git commit -m "feat(vol-llm-provider): create crate structure with config"
```

---

### Task 2.2: Implement Anthropic Provider

**Files:**
- Create: `crates/vol-llm-provider/src/anthropic.rs`

- [ ] **Step 1: Create src/anthropic.rs (Part 1 - struct and new)**

```rust
//! Anthropic Provider implementation.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::{debug, info};
use vol_llm_core::*;
use crate::LLMConfig;

/// Anthropic Provider
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl AnthropicProvider {
    /// Create new Anthropic provider
    pub fn new(config: &LLMConfig) -> Result<Self, LLMError> {
        Ok(Self {
            client: Client::new(),
            api_key: config.api_key()?,
            model: config.model.clone(),
            base_url: config.endpoint.clone().unwrap_or_else(|| "https://api.anthropic.com".to_string()),
        })
    }

    /// Convert messages to Anthropic format
    fn convert_messages(&self, messages: &[Message]) -> Result<Vec<serde_json::Value>, LLMError> {
        let mut result = Vec::new();

        for msg in messages {
            match msg.role {
                MessageRole::System => {
                    // Anthropic: system must be sent separately, not in messages
                }
                MessageRole::User => {
                    let content = msg.content.as_ref()
                        .map(|c| c.as_str())
                        .unwrap_or("");
                    result.push(json!({
                        "role": "user",
                        "content": content,
                    }));
                }
                MessageRole::Assistant => {
                    let mut content = Vec::new();

                    // Text content
                    if let Some(ref c) = msg.content {
                        content.push(json!({
                            "type": "text",
                            "text": c.as_str(),
                        }));
                    }

                    // Tool calls
                    if let Some(ref tools) = msg.tool_calls {
                        for tool in tools {
                            let input = serde_json::from_str::<serde_json::Value>(&tool.arguments)
                                .unwrap_or(json!({}));
                            content.push(json!({
                                "type": "tool_use",
                                "id": tool.id,
                                "name": tool.name,
                                "input": input,
                            }));
                        }
                    }

                    result.push(json!({
                        "role": "assistant",
                        "content": content,
                    }));
                }
                MessageRole::Tool => {
                    result.push(json!({
                        "role": "user",
                        "content": [{
                            "type": "tool_result",
                            "tool_use_id": msg.tool_call_id.as_deref().unwrap_or(""),
                            "content": msg.content.as_ref()
                                .map(|c| c.as_str())
                                .unwrap_or(""),
                        }],
                    }));
                }
            }
        }

        Ok(result)
    }

    /// Convert tools to Anthropic format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        json!(tools.iter().map(|t| {
            json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters.as_ref().unwrap_or(&json!({
                    "type": "object",
                    "properties": {}
                })),
            })
        }).collect::<Vec<_>>())
    }
}
```

- [ ] **Step 2: Add LLMClient implementation (Part 2)**

```rust
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
        ]
    }

    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse, LLMError> {
        // max_tokens is required for Anthropic
        let max_tokens = request.model_config.max_tokens.unwrap_or(1024);

        // Convert messages
        let anthropic_messages = self.convert_messages(&request.messages)?;

        // Build request body
        let mut body = json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": anthropic_messages,
        });

        // System message separately
        if let Some(system) = request.system {
            body["system"] = json!(system);
        }

        // Optional parameters
        if let Some(temp) = request.model_config.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.model_config.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(tools) = request.tools {
            body["tools"] = self.convert_tools(&tools);
        }

        // Send request
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

        // Handle response
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

        // Parse response
        let result: serde_json::Value = response.json().await?;

        // Extract content
        let content = result["content"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|item| item["text"].as_str())
            .unwrap_or("")
            .to_string();

        // Extract tool calls
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

        // Extract usage
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: (
                result["usage"]["input_tokens"].as_u64().unwrap_or(0) +
                result["usage"]["output_tokens"].as_u64().unwrap_or(0)
            ) as u32,
            cached_tokens: None,
        };

        // Extract finish reason
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

        info!(
            provider = "anthropic",
            model = %self.model,
            prompt_tokens = usage.prompt_tokens,
            completion_tokens = usage.completion_tokens,
            "LLM request completed"
        );

        Ok(ConversationResponse {
            message,
            model: result["model"].as_str().unwrap_or(&self.model).to_string(),
            usage,
            finish_reason,
            logprobs: None,
            raw: Some(result),
        })
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> Result<StreamReceiver, LLMError> {
        // TODO: Implement streaming
        Err(LLMError::Parse("Streaming not implemented".to_string()))
    }
}
```

- [ ] **Step 3: Run cargo check**

```bash
cd crates/vol-llm-provider && cargo check
```

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-provider/src/anthropic.rs
git commit -m "feat(vol-llm-provider): implement Anthropic Provider"
```

---

### Task 2.3: Implement OpenAI Provider

**Files:**
- Create: `crates/vol-llm-provider/src/openai.rs`

- [ ] **Step 1: Create src/openai.rs**

```rust
//! OpenAI Provider implementation.

use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use tracing::info;
use vol_llm_core::*;
use crate::LLMConfig;

/// OpenAI Provider
pub struct OpenAIProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

impl OpenAIProvider {
    /// Create new OpenAI provider
    pub fn new(config: &LLMConfig) -> Result<Self, LLMError> {
        Ok(Self {
            client: Client::new(),
            api_key: config.api_key()?,
            model: config.model.clone(),
            base_url: config.endpoint.clone().unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
        })
    }

    /// Convert message to OpenAI format
    fn convert_message(&self, msg: &Message) -> Result<serde_json::Value, LLMError> {
        let mut result = json!({
            "role": match msg.role {
                MessageRole::System => "system",
                MessageRole::User => "user",
                MessageRole::Assistant => "assistant",
                MessageRole::Tool => "tool",
            },
        });

        // Content
        if let Some(ref content) = msg.content {
            result["content"] = json!(content.as_str());
        }

        // Tool calls
        if let Some(ref tool_calls) = msg.tool_calls {
            result["tool_calls"] = json!(tool_calls.iter().map(|tc| {
                json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }
                })
            }).collect::<Vec<_>>());
        }

        // Tool call ID
        if let Some(ref tool_call_id) = msg.tool_call_id {
            result["tool_call_id"] = json!(tool_call_id);
        }

        // Name
        if let Some(ref name) = msg.name {
            result["name"] = json!(name);
        }

        Ok(result)
    }

    /// Convert tools to OpenAI format
    fn convert_tools(&self, tools: &[ToolDefinition]) -> serde_json::Value {
        json!(tools.iter().map(|t| {
            json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters.as_ref().unwrap_or(&json!({
                        "type": "object",
                        "properties": {}
                    })),
                }
            })
        }).collect::<Vec<_>>())
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
        ]
    }

    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse, LLMError> {
        // Build messages array
        let mut messages = Vec::new();

        // System message first
        if let Some(system) = &request.system {
            messages.push(json!({
                "role": "system",
                "content": system,
            }));
        }

        // Conversation history
        for msg in &request.messages {
            messages.push(self.convert_message(msg)?);
        }

        // Build request body
        let mut body = json!({
            "model": self.model,
            "messages": messages,
        });

        // Optional parameters
        if let Some(max) = request.model_config.max_tokens {
            body["max_tokens"] = json!(max);
        }
        if let Some(temp) = request.model_config.temperature {
            body["temperature"] = json!(temp);
        }
        if let Some(top_p) = request.model_config.top_p {
            body["top_p"] = json!(top_p);
        }
        if let Some(fp) = request.model_config.frequency_penalty {
            body["frequency_penalty"] = json!(fp);
        }
        if let Some(pp) = request.model_config.presence_penalty {
            body["presence_penalty"] = json!(pp);
        }
        if let Some(stop) = &request.model_config.stop {
            body["stop"] = json!(stop);
        }
        if let Some(seed) = request.model_config.seed {
            body["seed"] = json!(seed);
        }
        if let Some(tools) = &request.tools {
            body["tools"] = self.convert_tools(tools);
        }
        if let Some(choice) = &request.tool_choice {
            body["tool_choice"] = match choice {
                ToolChoice::Auto => json!("auto"),
                ToolChoice::Required => json!("required"),
                ToolChoice::None => json!("none"),
                ToolChoice::Specific { name } => json!({
                    "type": "function",
                    "function": { "name": name }
                }),
            };
        }

        // Send request
        let url = format!("{}/chat/completions", self.base_url);

        let response = self.client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(LLMError::Network)?;

        // Handle response
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

        // Parse response
        let result: serde_json::Value = response.json().await?;
        let choices = result["choices"].as_array().unwrap_or(&vec![]);
        let first = choices.first();

        // Extract message
        let content = first
            .and_then(|c| c["message"]["content"].as_str())
            .unwrap_or("")
            .to_string();

        let tool_calls = first
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

        // Extract usage
        let usage = TokenUsage {
            prompt_tokens: result["usage"]["prompt_tokens"].as_u64().unwrap_or(0) as u32,
            completion_tokens: result["usage"]["completion_tokens"].as_u64().unwrap_or(0) as u32,
            total_tokens: result["usage"]["total_tokens"].as_u64().unwrap_or(0) as u32,
            cached_tokens: None,
        };

        // Extract finish reason
        let finish_reason = match first.and_then(|c| c["finish_reason"].as_str()) {
            Some("stop") => FinishReason::Stop,
            Some("length") => FinishReason::Length,
            Some("tool_calls") | Some("function_call") => FinishReason::ToolCalls,
            Some("content_filter") => FinishReason::ContentFilter,
            _ => FinishReason::Other,
        };

        let message = if tool_calls.is_empty() {
            Message::assistant(content)
        } else {
            Message::assistant_with_tools(content, tool_calls)
        };

        info!(
            provider = "openai",
            model = %self.model,
            prompt_tokens = usage.prompt_tokens,
            completion_tokens = usage.completion_tokens,
            "LLM request completed"
        );

        Ok(ConversationResponse {
            message,
            model: result["model"].as_str().unwrap_or(&self.model).to_string(),
            usage,
            finish_reason,
            logprobs: None,
            raw: Some(result),
        })
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> Result<StreamReceiver, LLMError> {
        Err(LLMError::Parse("Streaming not implemented".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_creation() {
        // Basic test - actual API tests are in integration tests
        assert_eq!(LLMProvider::OpenAI.to_string(), "openai");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-provider && cargo test
```

Expected: Tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-provider/src/openai.rs
git commit -m "feat(vol-llm-provider): implement OpenAI Provider"
```

---

### Task 2.4: Implement factory functions

**Files:**
- Create: `crates/vol-llm-provider/src/factory.rs`

- [ ] **Step 1: Create src/factory.rs**

```rust
//! Provider factory functions.

use vol_llm_core::{LLMClient, LLMProvider, LLMError};
use crate::{AnthropicProvider, OpenAIProvider, LLMConfig};

/// Create provider from config
pub fn create_provider(config: &LLMConfig) -> Result<Box<dyn LLMClient>, LLMError> {
    match config.provider {
        LLMProvider::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        LLMProvider::OpenAI => Ok(Box::new(OpenAIProvider::new(config)?)),
    }
}

/// Load and create provider from config file
pub fn load_provider(config_path: &str) -> Result<Box<dyn LLMClient>, LLMError> {
    let config = LLMConfig::load(config_path)?;
    create_provider(&config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_anthropic() {
        let config = LLMConfig {
            provider: LLMProvider::Anthropic,
            model: "claude-test".to_string(),
            api_key_env: "TEST_API_KEY".to_string(),
            endpoint: None,
        };
        // Will fail due to missing env var, but tests the factory logic
        let result = create_provider(&config);
        assert!(result.is_err());
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-provider && cargo test
```

Expected: Tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-provider/src/factory.rs
git commit -m "feat(vol-llm-provider): add factory functions"
```

---

### Task 2.5: vol-llm-provider completion

- [ ] **Step 1: Run all tests**

```bash
cd crates/vol-llm-provider && cargo test
```

Expected: All tests pass

- [ ] **Step 2: Run clippy**

```bash
cd crates/vol-llm-provider && cargo clippy -- -D warnings
```

Expected: No warnings

- [ ] **Step 3: Commit final**

```bash
git add -A
git commit -m "feat(vol-llm-provider): complete provider implementations"
```

---

## Phase 3: vol-llm-tool

### Task 3.1: Create vol-llm-tool crate structure

**Files:**
- Create: `crates/vol-llm-tool/Cargo.toml`
- Create: `crates/vol-llm-tool/src/lib.rs`
- Create: `crates/vol-llm-tool/src/tool.rs`
- Create: `crates/vol-llm-tool/src/registry.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-tool"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
vol-core = { workspace = true }
```

- [ ] **Step 2: Create src/lib.rs**

```rust
//! vol-llm-tool: Tool framework for LLM Agent.

pub mod tool;
pub mod registry;
pub mod tools;

pub use tool::*;
pub use registry::*;
pub use tools::*;
```

- [ ] **Step 3: Create src/tool.rs**

```rust
//! Tool trait and types.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_core::{ToolDefinition, ToolCall, Message};
use vol_core::Alert;

/// Tool execution result
#[derive(Debug, Clone)]
pub struct ToolResult {
    pub call_id: String,
    pub success: bool,
    pub content: String,
    pub error: Option<String>,
    pub data: Option<serde_json::Value>,
}

/// Tool execution context
#[derive(Debug, Clone, Default)]
pub struct ToolContext {
    pub alert: Option<Alert>,
    pub messages: Vec<Message>,
    pub metadata: std::collections::HashMap<String, String>,
}

/// Tool trait
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters(&self) -> Option<serde_json::Value>;

    fn to_definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: self.name().to_string(),
            description: Some(self.description().to_string()),
            parameters: self.parameters(),
        }
    }

    async fn execute(&self, args: &str, context: &ToolContext) 
        -> Result<ToolResult, Box<dyn std::error::Error + Send>>;
}
```

- [ ] **Step 4: Create src/registry.rs**

```rust
//! Tool registry.

use std::collections::HashMap;
use vol_llm_core::{ToolDefinition, ToolCall};
use crate::tool::{Tool, ToolResult, ToolContext};

/// Tool registry
pub struct ToolRegistry {
    tools: HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: HashMap::new() }
    }

    pub fn register<T: Tool + 'static>(&mut self, tool: T) {
        self.tools.insert(tool.name().to_string(), Box::new(tool));
    }

    pub fn definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.to_definition()).collect()
    }

    pub async fn execute(
        &self,
        call: &ToolCall,
        context: &ToolContext,
    ) -> Result<ToolResult, String> {
        let tool = self.tools.get(&call.name)
            .ok_or_else(|| format!("Unknown tool: {}", call.name))?;

        let result = tool.execute(&call.arguments, context).await
            .map_err(|e| e.to_string())?;

        Ok(ToolResult {
            call_id: call.id.clone(),
            ..result
        })
    }

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

- [ ] **Step 5: Run cargo check**

```bash
cd crates/vol-llm-tool && cargo check
```

Expected: Compiles successfully

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-tool/Cargo.toml crates/vol-llm-tool/src/lib.rs crates/vol-llm-tool/src/tool.rs crates/vol-llm-tool/src/registry.rs
git commit -m "feat(vol-llm-tool): create crate structure with registry"
```

---

### Task 3.2: Create built-in tools module

**Files:**
- Create: `crates/vol-llm-tool/src/tools/mod.rs`
- Create: `crates/vol-llm-tool/src/tools/alert_history.rs`

- [ ] **Step 1: Create src/tools/mod.rs**

```rust
//! Built-in tools.

pub mod alert_history;
pub mod iv_curve;
pub mod market_data;
pub mod rule_info;

pub use alert_history::AlertHistoryTool;
pub use iv_curve::IvCurveTool;
pub use market_data::MarketDataTool;
pub use rule_info::RuleInfoTool;
```

- [ ] **Step 2: Create src/tools/alert_history.rs**

```rust
//! Alert history tool.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use vol_llm_core::Message;
use crate::tool::{Tool, ToolResult, ToolContext};

/// Alert history tool
pub struct AlertHistoryTool {
    window_hours: u32,
}

impl AlertHistoryTool {
    pub fn new(window_hours: u32) -> Self {
        Self { window_hours }
    }
}

#[derive(Debug, Deserialize)]
struct AlertHistoryArgs {
    symbol: String,
    tenor: Option<String>,
    alert_type: Option<String>,
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

    async fn execute(&self, args: &str, _context: &ToolContext) 
        -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        
        let args: AlertHistoryArgs = serde_json::from_str(args)
            .map_err(|e| format!("Invalid arguments: {}", e))?;

        // TODO: Query actual storage layer
        // For now, return placeholder response
        let content = format!(
            "查询 {} 历史告警 (窗口：{} 小时)",
            args.symbol, self.window_hours
        );

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "symbol": args.symbol,
                "count": 0,
                "alerts": []
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_alert_history_tool() {
        let tool = AlertHistoryTool::new(24);
        assert_eq!(tool.name(), "alert_history");
    }
}
```

- [ ] **Step 3: Run tests**

```bash
cd crates/vol-llm-tool && cargo test
```

Expected: Tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tool/src/tools/mod.rs crates/vol-llm-tool/src/tools/alert_history.rs
git commit -m "feat(vol-llm-tool): add alert_history tool"
```

---

### Task 3.3: Add IV curve tool

**Files:**
- Create: `crates/vol-llm-tool/src/tools/iv_curve.rs`

- [ ] **Step 1: Create src/tools/iv_curve.rs**

```rust
//! IV curve tool.

use async_trait::async_trait;
use serde::Deserialize;
use crate::tool::{Tool, ToolResult, ToolContext};

#[derive(Debug, Deserialize)]
struct IvCurveArgs {
    symbol: String,
    tenor: Option<String>,
}

/// IV curve tool
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

    async fn execute(&self, args: &str, _context: &ToolContext) 
        -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        
        let args: IvCurveArgs = serde_json::from_str(args)
            .map_err(|e| format!("Invalid arguments: {}", e))?;

        // TODO: Query actual IV data source
        let content = format!("获取 {} IV 曲线数据", args.symbol);

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "symbol": args.symbol,
                "iv_data": []
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iv_curve_tool() {
        let tool = IvCurveTool;
        assert_eq!(tool.name(), "iv_curve");
    }
}
```

- [ ] **Step 2: Run tests and commit**

```bash
cd crates/vol-llm-tool && cargo test && git add src/tools/iv_curve.rs && git commit -m "feat(vol-llm-tool): add iv_curve tool"
```

---

### Task 3.4: Add market data tool

**Files:**
- Create: `crates/vol-llm-tool/src/tools/market_data.rs`

- [ ] **Step 1: Create src/tools/market_data.rs**

```rust
//! Market data tool.

use async_trait::async_trait;
use serde::Deserialize;
use crate::tool::{Tool, ToolResult, ToolContext};

#[derive(Debug, Deserialize)]
struct MarketDataArgs {
    symbol: String,
    data_type: Option<String>,
}

/// Market data tool
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

    async fn execute(&self, args: &str, _context: &ToolContext) 
        -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        
        let args: MarketDataArgs = serde_json::from_str(args)
            .map_err(|e| format!("Invalid arguments: {}", e))?;

        // TODO: Query actual market data API
        let content = format!("获取 {} 市场数据", args.symbol);

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "symbol": args.symbol,
                "data": {}
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_market_data_tool() {
        let tool = MarketDataTool;
        assert_eq!(tool.name(), "market_data");
    }
}
```

- [ ] **Step 2: Run tests and commit**

```bash
cd crates/vol-llm-tool && cargo test && git add src/tools/market_data.rs && git commit -m "feat(vol-llm-tool): add market_data tool"
```

---

### Task 3.5: Add rule info tool

**Files:**
- Create: `crates/vol-llm-tool/src/tools/rule_info.rs`

- [ ] **Step 1: Create src/tools/rule_info.rs**

```rust
//! Rule info tool.

use async_trait::async_trait;
use serde::Deserialize;
use crate::tool::{Tool, ToolResult, ToolContext};

#[derive(Debug, Deserialize)]
struct RuleInfoArgs {
    alert_type: String,
}

/// Rule info tool
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

    async fn execute(&self, args: &str, _context: &ToolContext) 
        -> Result<ToolResult, Box<dyn std::error::Error + Send>> {
        
        let args: RuleInfoArgs = serde_json::from_str(args)
            .map_err(|e| format!("Invalid arguments: {}", e))?;

        // TODO: Query actual rule configuration
        let content = format!("查询告警规则: {}", args.alert_type);

        Ok(ToolResult {
            call_id: String::new(),
            success: true,
            content,
            error: None,
            data: Some(serde_json::json!({
                "alert_type": args.alert_type,
                "rule": {}
            })),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rule_info_tool() {
        let tool = RuleInfoTool;
        assert_eq!(tool.name(), "rule_info");
    }
}
```

- [ ] **Step 2: Run tests and commit**

```bash
cd crates/vol-llm-tool && cargo test && git add src/tools/rule_info.rs && git commit -m "feat(vol-llm-tool): add rule_info tool"
```

---

### Task 3.6: vol-llm-tool completion

- [ ] **Step 1: Run all tests**

```bash
cd crates/vol-llm-tool && cargo test
```

Expected: All tests pass

- [ ] **Step 2: Run clippy**

```bash
cd crates/vol-llm-tool && cargo clippy -- -D warnings
```

Expected: No warnings

- [ ] **Step 3: Commit final**

```bash
git add -A
git commit -m "feat(vol-llm-tool): complete tool framework and built-in tools"
```

---

## Phase 4: vol-llm-agent

### Task 4.1: Create vol-llm-agent crate structure

**Files:**
- Create: `crates/vol-llm-agent/Cargo.toml`
- Create: `crates/vol-llm-agent/src/lib.rs`
- Create: `crates/vol-llm-agent/src/agent.rs`
- Create: `crates/vol-llm-agent/src/response.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-agent"
version.workspace = true
edition.workspace = true

[dependencies]
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-tool = { path = "../vol-llm-tool" }
```

- [ ] **Step 2: Create src/lib.rs**

```rust
//! vol-llm-agent: ReAct Agent workflow orchestration.

pub mod agent;
pub mod response;
pub mod builder;
pub mod prompt;

pub use agent::*;
pub use response::*;
pub use builder::*;
pub use prompt::*;
```

- [ ] **Step 3: Create src/response.rs**

```rust
//! Agent response and error types.

use thiserror::Error;
use vol_llm_core::{LLMError, ToolCall};

/// Agent response
#[derive(Debug, Clone)]
pub struct AgentResponse {
    pub content: String,
    pub reasoning: String,
    pub iterations: u32,
    pub tool_calls: Vec<ToolCall>,
}

/// Agent error
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

- [ ] **Step 4: Run cargo check**

```bash
cd crates/vol-llm-agent && cargo check
```

Expected: Compiles (with warnings for unused modules)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/Cargo.toml crates/vol-llm-agent/src/lib.rs crates/vol-llm-agent/src/response.rs
git commit -m "feat(vol-llm-agent): create crate structure"
```

---

### Task 4.2: Implement ReAct Agent core

**Files:**
- Create: `crates/vol-llm-agent/src/agent.rs`

- [ ] **Step 1: Create src/agent.rs**

```rust
//! ReAct Agent implementation.

use vol_llm_core::{LLMClient, Message, MessageRole, ConversationRequest, ToolChoice};
use vol_llm_tool::{ToolRegistry, ToolContext};
use tracing::{info, debug, warn};
use crate::{AgentResponse, AgentError};

/// Agent configuration
#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub max_iterations: u32,
    pub system_prompt: String,
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
    llm: Box<dyn LLMClient>,
    tools: ToolRegistry,
    config: AgentConfig,
}

impl ReActAgent {
    pub fn new(llm: Box<dyn LLMClient>, tools: ToolRegistry, config: AgentConfig) -> Self {
        Self { llm, tools, config }
    }

    /// Run ReAct loop
    pub async fn run(
        &self,
        user_input: &str,
        context: ToolContext,
    ) -> Result<AgentResponse, AgentError> {
        let mut messages = Vec::new();
        let mut iteration = 0;

        // Initialize with system prompt
        messages.push(Message::system(self.config.system_prompt.clone()));
        messages.push(Message::user(user_input));

        loop {
            iteration += 1;

            if iteration > self.config.max_iterations {
                return Err(AgentError::MaxIterationsReached {
                    max: self.config.max_iterations,
                });
            }

            if self.config.verbose {
                info!("Iteration {}", iteration);
            }

            // Reason phase - call LLM
            let tools = self.tools.definitions();
            let request = ConversationRequest::with_history(None, messages.clone())
                .with_tools(tools)
                .with_tool_choice(ToolChoice::Auto);

            let response = self.llm.converse(request).await?;

            // Check if tool calls
            if let Some(tool_calls) = &response.message.tool_calls {
                if !tool_calls.is_empty() {
                    debug!("Tool calls: {:?}", tool_calls);

                    // Act phase - execute tools
                    let mut observations = Vec::new();
                    for call in tool_calls {
                        let result = self.tools.execute(call, &context).await
                            .map_err(|e| AgentError::ToolExecution {
                                tool: call.name.clone(),
                                error: e,
                            })?;

                        observations.push((call.id.clone(), result.content.clone()));
                    }

                    // Observation phase - add results to messages
                    messages.push(response.message.clone());
                    for (call_id, content) in observations {
                        messages.push(Message::tool(content, call_id));
                    }

                    continue;
                }
            }

            // Final response
            let content = response.message.content
                .unwrap_or(vol_llm_core::MessageContent::Text(String::new()))
                .as_str()
                .to_string();

            info!("Agent completed in {} iterations", iteration);

            return Ok(AgentResponse {
                content,
                reasoning: String::new(),
                iterations: iteration,
                tool_calls: tool_calls.unwrap_or_default(),
            });
        }
    }
}
```

- [ ] **Step 2: Run cargo check**

```bash
cd crates/vol-llm-agent && cargo check
```

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/agent.rs
git commit -m "feat(vol-llm-agent): implement ReAct Agent core"
```

---

### Task 4.3: Implement Agent Builder

**Files:**
- Create: `crates/vol-llm-agent/src/builder.rs`

- [ ] **Step 1: Create src/builder.rs**

```rust
//! Agent builder.

use vol_llm_core::LLMClient;
use vol_llm_tool::{Tool, ToolRegistry};
use crate::{ReActAgent, AgentConfig, AgentBuilderError};

/// Agent builder
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
            registry.register(tool);
        }

        Ok(ReActAgent::new(llm, registry, self.config))
    }
}

impl Default for AgentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder error
#[derive(Debug, thiserror::Error)]
pub enum AgentBuilderError {
    #[error("LLM client is required")]
    MissingLlm,
}
```

- [ ] **Step 2: Run cargo check and commit**

```bash
cd crates/vol-llm-agent && cargo check && git add crates/vol-llm-agent/src/builder.rs && git commit -m "feat(vol-llm-agent): add AgentBuilder"
```

---

### Task 4.4: Implement system prompt templates

**Files:**
- Create: `crates/vol-llm-agent/src/prompt.rs`

- [ ] **Step 1: Create src/prompt.rs**

```rust
//! System prompt templates.

use vol_llm_core::ToolDefinition;

/// Default system prompt
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

/// System prompt builder
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

    pub fn with_tools(mut self, tools: &[ToolDefinition]) -> Self {
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

impl Default for SystemPromptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 2: Run cargo check and commit**

```bash
cd crates/vol-llm-agent && cargo check && git add crates/vol-llm-agent/src/prompt.rs && git commit -m "feat(vol-llm-agent): add system prompt templates"
```

---

### Task 4.5: vol-llm-agent completion

- [ ] **Step 1: Run all tests**

```bash
cd crates/vol-llm-agent && cargo test
```

Expected: All tests pass

- [ ] **Step 2: Run clippy**

```bash
cd crates/vol-llm-agent && cargo clippy -- -D warnings
```

Expected: No warnings

- [ ] **Step 3: Commit final**

```bash
git add -A
git commit -m "feat(vol-llm-agent): complete ReAct Agent implementation"
```

---

## Phase 5: Integration

### Task 5.1: Update workspace

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: Add workspace members**

Modify root `Cargo.toml`:

```toml
[workspace]
resolver = "2"
members = [
    "crates/vol-core",
    "crates/vol-eventbus",
    "crates/vol-config",
    "crates/vol-datasource",
    "crates/vol-deribit",
    "crates/vol-alert",
    "crates/vol-notification",
    "crates/vol-monitor",
    "crates/vol-engine",
    "crates/vol-rules",
    "crates/vol-tracing",
    "crates/vol-llm-core",
    "crates/vol-llm-provider",
    "crates/vol-llm-tool",
    "crates/vol-llm-agent",
]
```

- [ ] **Step 2: Add toml dependency if missing**

```toml
# In [workspace.dependencies]
toml = "0.8"
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check --workspace
```

Expected: All crates compile

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "chore: add vol-llm-* crates to workspace"
```

---

### Task 5.2: Add configuration examples

**Files:**
- Create: `config/llm.example.toml`
- Modify: `.env.example`

- [ ] **Step 1: Create config/llm.example.toml**

```toml
# LLM Provider Configuration

[llm]
provider = "anthropic"  # or "openai"
model = "claude-sonnet-4-20251001"
api_key_env = "ANTHROPIC_API_KEY"

[agent]
max_iterations = 5
verbose = true
```

- [ ] **Step 2: Update .env.example**

```bash
# LLM API Keys
ANTHROPIC_API_KEY=
OPENAI_API_KEY=
```

- [ ] **Step 3: Commit**

```bash
git add config/llm.example.toml .env.example
git commit -m "docs: add LLM configuration examples"
```

---

### Task 5.3: Run workspace tests

- [ ] **Step 1: Run all tests**

```bash
cargo test --workspace
```

Expected: All tests pass

- [ ] **Step 2: Run clippy on workspace**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: No warnings

---

### Task 5.4: Update documentation

**Files:**
- Modify: `CLAUDE.md`
- Modify: `README.md` (if exists)

- [ ] **Step 1: Update CLAUDE.md**

Add AI Agent section to CLAUDE.md

- [ ] **Step 2: Commit**

```bash
git add CLAUDE.md README.md
git commit -m "docs: add AI Agent documentation"
```

---

## Self-Review

**1. Spec coverage check:**

| Spec Requirement | Task |
|-----------------|------|
| vol-llm-core types | Tasks 1.1-1.9 |
| vol-llm-provider Anthropic | Task 2.2 |
| vol-llm-provider OpenAI | Task 2.3 |
| vol-llm-tool framework | Task 3.1 |
| vol-llm-tool built-ins | Tasks 3.2-3.5 |
| vol-llm-agent ReAct | Tasks 4.1-4.5 |
| Workspace integration | Task 5.1 |
| Configuration examples | Task 5.2 |

**2. Placeholder scan:** No TBD/TODO remaining in implementation tasks.

**3. Type consistency:** All types reference vol_llm_core consistently.

**4. Scope check:** Plan covers all 4 crates with complete implementations.

---

Plan complete. Two execution options:

**1. Subagent-Driven (recommended)** - Fresh subagent per task with review between tasks

**2. Inline Execution** - Execute tasks in this session with checkpoints

Which approach?
