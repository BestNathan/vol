# AgentInput Multimodal Run Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add structured multimodal `AgentInput` support to `ReActAgent` while preserving the existing `run(&str)` text API.

**Architecture:** Introduce a focused input module in `vol-llm-agent` that converts run input parts into existing `vol_llm_core::MessageContent`. Route text calls through `run_input`, update Anthropic provider conversion so multipart user content is preserved, and make agent-channel request input deserialize from either old strings or the new structured envelope.

**Tech Stack:** Rust, Tokio async tests, Serde, `vol_llm_core::MessageContent`, `vol_llm_agent::ReActAgent`, `vol-llm-agent-channel` protocol/request types.

---

## File Structure

- Create `crates/vol-llm-agent/src/react/input.rs` — owns `AgentInput`, `InputPart`, validation, display text, and conversion into `MessageContent`.
- Modify `crates/vol-llm-agent/src/react/mod.rs` — export the new input types.
- Modify `crates/vol-llm-agent/src/lib.rs` — re-export `AgentInput` and `InputPart` from the crate root.
- Modify `crates/vol-llm-agent/src/react/agent.rs` — add `run_input`, make `run(&str)` a wrapper, use caller-provided `run_id`, and persist structured content.
- Modify `crates/vol-llm-provider/src/anthropic.rs` — convert `MessageContent::MultiPart` for user messages into Anthropic content blocks.
- Modify `crates/vol-llm-agent-channel/src/request.rs` — change `AgentRequest.input` to `AgentInput` while preserving plain-text constructors.
- Modify `crates/vol-llm-agent-channel/src/protocol.rs` — make submit message input serde-compatible with both string and structured object.
- Modify `crates/vol-llm-agent-channel/src/dispatcher.rs` — call `run_input` and update tests for `AgentInput` equality/access.
- Test files: `crates/vol-llm-agent/tests/agent_run_tests.rs`, inline tests in `input.rs`, inline tests in `anthropic.rs`, inline or integration tests in `vol-llm-agent-channel`.

---

### Task 1: Add AgentInput types and conversion

**Files:**
- Create: `crates/vol-llm-agent/src/react/input.rs`
- Modify: `crates/vol-llm-agent/src/react/mod.rs`
- Modify: `crates/vol-llm-agent/src/lib.rs`

- [ ] **Step 1: Write failing unit tests for text and multipart conversion**

Create `crates/vol-llm-agent/src/react/input.rs` with the tests first. The production items referenced by the tests do not exist yet, so this should fail to compile.

```rust
use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use vol_llm_core::{ContentPart, ImageUrl, MessageContent};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InputPart {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInput {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_converts_to_text_message_content() {
        let input = AgentInput::text("hello");

        assert_eq!(input.display_text(), "hello");
        assert_eq!(input.to_message_content().unwrap(), MessageContent::Text("hello".to_string()));
    }

    #[test]
    fn text_and_image_convert_to_multipart_message_content() {
        let input = AgentInput::new()
            .text_part("look")
            .image_url_with_detail("https://example.test/image.png", "high");

        assert_eq!(input.display_text(), "look");
        assert_eq!(
            input.to_message_content().unwrap(),
            MessageContent::MultiPart(vec![
                ContentPart::Text { text: "look".to_string() },
                ContentPart::Image {
                    image_url: ImageUrl {
                        url: "https://example.test/image.png".to_string(),
                        detail: Some("high".to_string()),
                    },
                },
            ])
        );
    }

    #[test]
    fn empty_input_returns_error() {
        let err = AgentInput::new().to_message_content().unwrap_err();
        assert_eq!(err.to_string(), "Agent input must contain at least one part");
    }

    #[test]
    fn string_deserializes_as_text_input() {
        let input: AgentInput = serde_json::from_str(r#"\"hello\""#).unwrap();
        assert_eq!(input, AgentInput::text("hello"));
    }

    #[test]
    fn object_deserializes_as_structured_input() {
        let input: AgentInput = serde_json::from_str(r#"
        {
          "run_id": "run-1",
          "parts": [
            { "type": "text", "text": "look" },
            { "type": "image_url", "url": "data:image/png;base64,AAAA", "detail": "low" }
          ],
          "metadata": { "source": "test" }
        }
        "#).unwrap();

        assert_eq!(input.run_id.as_deref(), Some("run-1"));
        assert_eq!(input.parts.len(), 2);
        assert_eq!(input.metadata.get("source"), Some(&serde_json::json!("test")));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p vol-llm-agent react::input --features test-utils
```

Expected: FAIL with compile errors because `AgentInput::text`, `AgentInput::new`, `InputPart` variants, and conversion methods are not implemented.

- [ ] **Step 3: Implement AgentInput and InputPart**

Replace the placeholder contents of `crates/vol-llm-agent/src/react/input.rs` with:

```rust
use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use vol_llm_core::{ContentPart, ImageUrl, MessageContent};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentInputError;

impl fmt::Display for AgentInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Agent input must contain at least one part")
    }
}

impl std::error::Error for AgentInputError {}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InputPart {
    Text { text: String },
    ImageUrl {
        url: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AgentInput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    pub parts: Vec<InputPart>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum AgentInputWire {
    Text(String),
    Structured {
        #[serde(default)]
        run_id: Option<String>,
        #[serde(default)]
        parts: Vec<InputPart>,
        #[serde(default)]
        metadata: HashMap<String, serde_json::Value>,
    },
}

impl<'de> Deserialize<'de> for AgentInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        match AgentInputWire::deserialize(deserializer)? {
            AgentInputWire::Text(text) => Ok(Self::text(text)),
            AgentInputWire::Structured { run_id, parts, metadata } => Ok(Self { run_id, parts, metadata }),
        }
    }
}

impl AgentInput {
    pub fn new() -> Self {
        Self {
            run_id: None,
            parts: Vec::new(),
            metadata: HashMap::new(),
        }
    }

    pub fn text(text: impl Into<String>) -> Self {
        Self::new().text_part(text)
    }

    pub fn with_run_id(mut self, run_id: impl Into<String>) -> Self {
        self.run_id = Some(run_id.into());
        self
    }

    pub fn with_metadata_value(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }

    pub fn text_part(mut self, text: impl Into<String>) -> Self {
        self.parts.push(InputPart::Text { text: text.into() });
        self
    }

    pub fn image_url(mut self, url: impl Into<String>) -> Self {
        self.parts.push(InputPart::ImageUrl { url: url.into(), detail: None });
        self
    }

    pub fn image_url_with_detail(mut self, url: impl Into<String>, detail: impl Into<String>) -> Self {
        self.parts.push(InputPart::ImageUrl { url: url.into(), detail: Some(detail.into()) });
        self
    }

    pub fn display_text(&self) -> String {
        self.parts
            .iter()
            .filter_map(|part| match part {
                InputPart::Text { text } => Some(text.as_str()),
                InputPart::ImageUrl { .. } => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn to_message_content(&self) -> Result<MessageContent, AgentInputError> {
        if self.parts.is_empty() {
            return Err(AgentInputError);
        }

        if let [InputPart::Text { text }] = self.parts.as_slice() {
            return Ok(MessageContent::Text(text.clone()));
        }

        let parts = self.parts.iter().map(|part| match part {
            InputPart::Text { text } => ContentPart::Text { text: text.clone() },
            InputPart::ImageUrl { url, detail } => ContentPart::Image {
                image_url: ImageUrl { url: url.clone(), detail: detail.clone() },
            },
        }).collect();

        Ok(MessageContent::MultiPart(parts))
    }
}

impl Default for AgentInput {
    fn default() -> Self {
        Self::new()
    }
}

impl From<String> for AgentInput {
    fn from(value: String) -> Self {
        Self::text(value)
    }
}

impl From<&str> for AgentInput {
    fn from(value: &str) -> Self {
        Self::text(value)
    }
}
```

Keep the tests from Step 1 at the bottom of the file.

- [ ] **Step 4: Export the new types**

Modify `crates/vol-llm-agent/src/react/mod.rs`:

```rust
pub mod input;
```

Add the public export near the other `pub use` lines:

```rust
pub use input::{AgentInput, AgentInputError, InputPart};
```

Modify the crate-root export in `crates/vol-llm-agent/src/lib.rs` from:

```rust
pub use react::{
    AgentConfig, AgentConfigBuilder, AgentConfigBuildError, AgentError, AgentResponse, AgentStreamEvent, AgentStreamReceiver,
    ReActAgent,
};
```

to:

```rust
pub use react::{
    AgentConfig, AgentConfigBuilder, AgentConfigBuildError, AgentError, AgentInput, AgentInputError,
    AgentResponse, AgentStreamEvent, AgentStreamReceiver, InputPart, ReActAgent,
};
```

- [ ] **Step 5: Run tests to verify they pass**

Run:

```bash
cargo test -p vol-llm-agent react::input --features test-utils
```

Expected: PASS for all `react::input` tests.

- [ ] **Step 6: Commit**

Only commit if the user has explicitly approved commits in this session. If approved, run:

```bash
git add crates/vol-llm-agent/src/react/input.rs crates/vol-llm-agent/src/react/mod.rs crates/vol-llm-agent/src/lib.rs
git commit -m "feat: add structured agent input"
```

---

### Task 2: Route ReActAgent through run_input

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:188-246`
- Test: `crates/vol-llm-agent/tests/agent_run_tests.rs`

- [ ] **Step 1: Add failing tests for run_input behavior**

Add imports in `crates/vol-llm-agent/tests/agent_run_tests.rs`:

```rust
use vol_llm_agent::{AgentInput, InputPart};
use vol_llm_core::MessageContent;
```

Add these tests after `test_agent_run_single_iteration`:

```rust
#[tokio::test]
async fn test_agent_run_input_text_matches_run() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![content_complete_event("ok")]).await;

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock))
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    let result = agent.run_input(AgentInput::text("Hi")).await.unwrap();

    assert_eq!(result.content, "ok");
    assert!(result.error.is_none());
}

#[tokio::test]
async fn test_agent_run_input_uses_provided_run_id() {
    let mock = MockLlmClient::new();
    mock.set_stream_events(vec![content_complete_event("ok")]).await;

    let config = AgentConfig::builder()
        .with_llm(Arc::new(mock))
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    let result = agent
        .run_input(AgentInput::text("Hi").with_run_id("caller-run-id"))
        .await
        .unwrap();

    assert_eq!(result.run_id, "caller-run-id");
}

#[tokio::test]
async fn test_agent_run_input_sends_multipart_user_message() {
    let mock = Arc::new(MockLlmClient::new());
    mock.set_stream_events(vec![content_complete_event("ok")]).await;

    let config = AgentConfig::builder()
        .with_llm(mock.clone())
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    agent
        .run_input(
            AgentInput::new()
                .text_part("look")
                .image_url("data:image/png;base64,AAAA"),
        )
        .await
        .unwrap();

    let request = mock.last_request().await.unwrap();
    let user_message = request.messages.iter().find(|message| {
        matches!(message.content, Some(MessageContent::MultiPart(_)))
    }).expect("multipart user message should be sent to LLM");

    match user_message.content.as_ref().unwrap() {
        MessageContent::MultiPart(parts) => assert_eq!(parts.len(), 2),
        other => panic!("expected multipart content, got {other:?}"),
    }
}

#[tokio::test]
async fn test_agent_run_input_rejects_empty_parts_before_llm_call() {
    let mock = Arc::new(MockLlmClient::new());

    let config = AgentConfig::builder()
        .with_llm(mock.clone())
        .with_system_prompt("You are a test assistant.".to_string())
        .build()
        .unwrap();
    let agent = ReActAgent::new(config);

    let result = agent.run_input(AgentInput::new()).await;

    assert!(matches!(result, Err(AgentError::InvalidToolResponse(_))));
    assert_eq!(mock.call_count(), 0);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p vol-llm-agent --test agent_run_tests test_agent_run_input --features test-utils
```

Expected: FAIL because `ReActAgent::run_input` does not exist yet. The empty-input test may later use a different `AgentError` variant if Task 2 Step 3 adds one.

- [ ] **Step 3: Add AgentError variant for invalid input**

Modify `crates/vol-llm-agent/src/react/response.rs` by adding this variant after `InvalidToolResponse`:

```rust
#[error("Invalid agent input: {0}")]
InvalidInput(String),
```

Update the empty-input test assertion to:

```rust
assert!(matches!(result, Err(AgentError::InvalidInput(message)) if message == "Agent input must contain at least one part"));
```

- [ ] **Step 4: Implement run_input and make run a wrapper**

Modify imports in `crates/vol-llm-agent/src/react/agent.rs` to include `AgentInput`:

```rust
use super::{
    AgentInput, AgentResponse, AgentStreamEvent, PluginDecision, PluginRegistry, RunContext,
};
```

Replace the `run` signature and beginning through user message persistence with this structure:

```rust
pub async fn run(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
    self.run_input(AgentInput::text(user_input)).await
}

pub async fn run_input(&self, input: AgentInput) -> Result<AgentResponse, crate::AgentError> {
    let user_content = input
        .to_message_content()
        .map_err(|e| crate::AgentError::InvalidInput(e.to_string()))?;
    let user_input = input.display_text();

    let effective_tools = if let Some(def) = &self.config.def {
        let allowed: Option<Vec<&str>> = def.tools.as_ref()
            .map(|t| t.iter().map(|s| s.as_str()).collect());
        let disallowed: Option<Vec<&str>> = def.disallowed_tools.as_ref()
            .map(|t| t.iter().map(|s| s.as_str()).collect());
        ToolRegistry::filter(&self.config.tools, allowed.as_deref(), disallowed.as_deref())
    } else {
        self.config.tools.clone()
    };

    let max_iterations = self.config.def.as_ref()
        .and_then(|d| d.max_iterations)
        .unwrap_or(5);
    let max_history_messages = self.config.def.as_ref()
        .and_then(|d| d.max_history_messages)
        .unwrap_or(20);

    let config = AgentConfig {
        tools: effective_tools.clone(),
        ..self.config.clone()
    };

    let run_id = input
        .run_id
        .clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());

    let session = self.config.session.clone();

    let (run_ctx, plugin_rx) = RunContext::new(
        run_id.clone(),
        user_input.clone(),
        self.config.session.id.clone(),
        session.clone(),
        effective_tools,
        config.clone(),
        max_history_messages,
        config.llm.model().to_string(),
    );

    for (key, value) in input.metadata {
        run_ctx.data.write().await.insert(key, value);
    }

    let user_msg = Message::user(user_content);
    run_ctx.add_message(user_msg).await.map_err(|e| {
        crate::AgentError::SessionError(format!("Failed to persist user message: {}", e))
    })?;

    // keep the existing remainder of run() unchanged from listener setup onward
```

In the existing later code, keep:

```rust
let user_input = user_input.to_string();
```

or simplify it to:

```rust
let user_input = user_input.clone();
```

because `user_input` is now already an owned `String`.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p vol-llm-agent --test agent_run_tests test_agent_run_input --features test-utils
```

Expected: PASS for the four new `run_input` tests.

- [ ] **Step 6: Run existing text run test**

Run:

```bash
cargo test -p vol-llm-agent --test agent_run_tests test_agent_run_single_iteration --features test-utils
```

Expected: PASS, proving `run(&str)` still works.

- [ ] **Step 7: Commit**

Only commit if approved:

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/response.rs crates/vol-llm-agent/tests/agent_run_tests.rs
git commit -m "feat: route agent runs through structured input"
```

---

### Task 3: Preserve multipart input in Anthropic provider

**Files:**
- Modify: `crates/vol-llm-provider/src/anthropic.rs`

- [ ] **Step 1: Add unit tests for Anthropic message conversion**

At the bottom of `crates/vol-llm-provider/src/anthropic.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn provider() -> AnthropicProvider {
        AnthropicProvider {
            client: Client::new(),
            api_key: "test-key".to_string(),
            model: "claude-test".to_string(),
            base_url: "https://example.test".to_string(),
        }
    }

    #[test]
    fn converts_user_multipart_url_image() {
        let provider = provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![
            ContentPart::Text { text: "look".to_string() },
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "https://example.test/image.png".to_string(),
                    detail: Some("high".to_string()),
                },
            },
        ]))];

        let converted = provider.convert_messages(&messages).unwrap();

        assert_eq!(converted[0]["role"], "user");
        assert_eq!(converted[0]["content"][0], json!({ "type": "text", "text": "look" }));
        assert_eq!(converted[0]["content"][1], json!({
            "type": "image",
            "source": { "type": "url", "url": "https://example.test/image.png" },
        }));
    }

    #[test]
    fn converts_user_multipart_data_url_image() {
        let provider = provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png;base64,QUJD".to_string(),
                    detail: None,
                },
            },
        ]))];

        let converted = provider.convert_messages(&messages).unwrap();

        assert_eq!(converted[0]["content"][0], json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": "QUJD"
            },
        }));
    }

    #[test]
    fn rejects_invalid_data_url_image() {
        let provider = provider();
        let messages = vec![Message::user(MessageContent::MultiPart(vec![
            ContentPart::Image {
                image_url: ImageUrl {
                    url: "data:image/png,not-base64".to_string(),
                    detail: None,
                },
            },
        ]))];

        let err = provider.convert_messages(&messages).unwrap_err();
        assert!(err.to_string().contains("Invalid image data URL"));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p vol-llm-provider anthropic::tests::converts_user_multipart --features test-utils
```

Expected: FAIL because multipart content currently converts through `as_str()` and data URL parsing does not exist.

- [ ] **Step 3: Add conversion helpers**

Add helper functions inside `impl AnthropicProvider` before `convert_messages`:

```rust
fn convert_user_content(&self, content: Option<&MessageContent>) -> Result<serde_json::Value> {
    match content {
        Some(MessageContent::Text(text)) => Ok(json!(text)),
        Some(MessageContent::MultiPart(parts)) => {
            let mut blocks = Vec::new();
            for part in parts {
                match part {
                    ContentPart::Text { text } => blocks.push(json!({
                        "type": "text",
                        "text": text,
                    })),
                    ContentPart::Image { image_url } => blocks.push(self.convert_image_block(image_url)?),
                }
            }
            Ok(json!(blocks))
        }
        None => Ok(json!("")),
    }
}

fn convert_image_block(&self, image_url: &ImageUrl) -> Result<serde_json::Value> {
    if image_url.url.starts_with("data:") {
        let (media_type, data) = Self::parse_image_data_url(&image_url.url)?;
        return Ok(json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": media_type,
                "data": data,
            },
        }));
    }

    Ok(json!({
        "type": "image",
        "source": {
            "type": "url",
            "url": image_url.url,
        },
    }))
}

fn parse_image_data_url(url: &str) -> Result<(&str, &str)> {
    let rest = url
        .strip_prefix("data:")
        .ok_or_else(|| LLMError::InvalidRequest("Invalid image data URL".to_string()))?;
    let (metadata, data) = rest
        .split_once(',')
        .ok_or_else(|| LLMError::InvalidRequest("Invalid image data URL".to_string()))?;
    let media_type = metadata
        .strip_suffix(";base64")
        .ok_or_else(|| LLMError::InvalidRequest("Invalid image data URL".to_string()))?;
    if media_type.is_empty() || data.is_empty() {
        return Err(LLMError::InvalidRequest("Invalid image data URL".to_string()));
    }
    Ok((media_type, data))
}
```

If `LLMError::InvalidRequest` does not exist, inspect `crates/vol-llm-core/src/error.rs` and use the closest non-network/non-API variant. If no suitable variant exists, add one there with display text `Invalid request: {0}` and update its tests if present.

- [ ] **Step 4: Use the helper for user messages**

In `convert_messages`, replace the user branch:

```rust
let content = msg.content.as_ref().map(|c| c.as_str()).unwrap_or("");
result.push(json!({
    "role": "user",
    "content": content,
}));
```

with:

```rust
let content = self.convert_user_content(msg.content.as_ref())?;
result.push(json!({
    "role": "user",
    "content": content,
}));
```

- [ ] **Step 5: Run provider tests**

Run:

```bash
cargo test -p vol-llm-provider anthropic::tests --features test-utils
```

Expected: PASS for all Anthropic unit tests.

- [ ] **Step 6: Commit**

Only commit if approved:

```bash
git add crates/vol-llm-provider/src/anthropic.rs crates/vol-llm-core/src/error.rs
git commit -m "feat: convert anthropic multipart user content"
```

---

### Task 4: Extend agent-channel requests to AgentInput

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/request.rs`
- Modify: `crates/vol-llm-agent-channel/src/protocol.rs`
- Modify: `crates/vol-llm-agent-channel/src/dispatcher.rs`
- Test: `crates/vol-llm-agent-channel/tests/jsonrpc_integration.rs` or inline module tests in changed files

- [ ] **Step 1: Add failing request/protocol compatibility tests**

In `crates/vol-llm-agent-channel/src/request.rs`, add to the existing or new `#[cfg(test)]` module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::{AgentInput, InputPart};

    #[test]
    fn text_constructor_wraps_input_as_agent_input() {
        let request = AgentRequest::new("agent-a", "hello");
        assert_eq!(request.input, AgentInput::text("hello"));
    }

    #[test]
    fn structured_constructor_preserves_parts() {
        let input = AgentInput::new().text_part("look").image_url("data:image/png;base64,AAAA");
        let request = AgentRequest::with_input("agent-a", input.clone());
        assert_eq!(request.input, input);
        assert!(matches!(request.input.parts[1], InputPart::ImageUrl { .. }));
    }
}
```

In `crates/vol-llm-agent-channel/src/protocol.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_agent::{AgentInput, InputPart};

    #[test]
    fn submit_accepts_legacy_string_input() {
        let message: Message = serde_json::from_str(r#"
        {
          "type": "submit",
          "req_id": "req-1",
          "sender": "client",
          "receiver": "agent",
          "input": "hello"
        }
        "#).unwrap();

        match message {
            Message::Submit { input, .. } => assert_eq!(input, AgentInput::text("hello")),
            other => panic!("expected submit message, got {other:?}"),
        }
    }

    #[test]
    fn submit_accepts_structured_input() {
        let message: Message = serde_json::from_str(r#"
        {
          "type": "submit",
          "req_id": "req-1",
          "sender": "client",
          "receiver": "agent",
          "input": {
            "run_id": "run-1",
            "parts": [
              { "type": "text", "text": "look" },
              { "type": "image_url", "url": "data:image/png;base64,AAAA" }
            ]
          }
        }
        "#).unwrap();

        match message {
            Message::Submit { input, .. } => {
                assert_eq!(input.run_id.as_deref(), Some("run-1"));
                assert!(matches!(input.parts[1], InputPart::ImageUrl { .. }));
            }
            other => panic!("expected submit message, got {other:?}"),
        }
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p vol-llm-agent-channel request::tests protocol::tests --features test-utils
```

Expected: FAIL because `AgentRequest.input` and `Message::Submit.input` are still `String`, and `with_input` does not exist.

- [ ] **Step 3: Update AgentRequest input type and constructors**

Modify `crates/vol-llm-agent-channel/src/request.rs` imports:

```rust
use vol_llm_agent::{AgentInput, AgentResponse};
```

Change the field:

```rust
pub input: AgentInput,
```

Change constructors:

```rust
pub fn new(target_id: impl Into<String>, input: impl Into<String>) -> Self {
    Self::with_input(target_id, AgentInput::text(input.into()))
}

pub fn with_input(target_id: impl Into<String>, input: AgentInput) -> Self {
    Self {
        req_id: uuid::Uuid::new_v4().simple().to_string(),
        target_id: target_id.into(),
        sender_id: None,
        input,
        metadata: HashMap::new(),
    }
}

pub fn with_id(
    req_id: impl Into<String>,
    target_id: impl Into<String>,
    input: impl Into<String>,
) -> Self {
    Self::with_id_and_input(req_id, target_id, AgentInput::text(input.into()))
}

pub fn with_id_and_input(
    req_id: impl Into<String>,
    target_id: impl Into<String>,
    input: AgentInput,
) -> Self {
    Self {
        req_id: req_id.into(),
        target_id: target_id.into(),
        sender_id: None,
        input,
        metadata: HashMap::new(),
    }
}
```

- [ ] **Step 4: Update protocol submit input type**

Modify `crates/vol-llm-agent-channel/src/protocol.rs` imports:

```rust
use vol_llm_agent::AgentInput;
```

Change `Message::Submit`:

```rust
input: AgentInput,
```

Serde compatibility is handled by `AgentInput`'s custom `Deserialize` from Task 1.

- [ ] **Step 5: Update dispatcher to call run_input**

In `crates/vol-llm-agent-channel/src/dispatcher.rs`, replace:

```rust
let result = agent.run(&pending.request.input).await;
```

with:

```rust
let result = agent.run_input(pending.request.input.clone()).await;
```

Update assertions that compare request input as a string. For example replace:

```rust
assert_eq!(first.unwrap().request.input, "hello");
```

with:

```rust
assert_eq!(first.unwrap().request.input, vol_llm_agent::AgentInput::text("hello"));
```

- [ ] **Step 6: Run channel tests**

Run:

```bash
cargo test -p vol-llm-agent-channel --features test-utils
```

Expected: PASS for channel unit and integration tests.

- [ ] **Step 7: Commit**

Only commit if approved:

```bash
git add crates/vol-llm-agent-channel/src/request.rs crates/vol-llm-agent-channel/src/protocol.rs crates/vol-llm-agent-channel/src/dispatcher.rs
git commit -m "feat: accept structured agent channel input"
```

---

### Task 5: Full verification and wiki ingest

**Files:**
- Potentially modify docs/wiki files through `wiki-ingest` skill after implementation is complete.

- [ ] **Step 1: Run focused crate tests**

Run:

```bash
cargo test -p vol-llm-agent --features test-utils
```

Expected: PASS.

Run:

```bash
cargo test -p vol-llm-provider --features test-utils
```

Expected: PASS.

Run:

```bash
cargo test -p vol-llm-agent-channel --features test-utils
```

Expected: PASS.

- [ ] **Step 2: Run formatting**

Run:

```bash
cargo fmt --all --check
```

Expected: PASS. If it fails, run `cargo fmt --all`, then rerun `cargo fmt --all --check`.

- [ ] **Step 3: Run workspace check**

Run:

```bash
cargo check --workspace --all-targets --features test-utils
```

Expected: PASS.

- [ ] **Step 4: Ingest implementation into project wiki**

Invoke the `wiki-ingest` skill with a summary of changed source files and the behavior added. The repository instruction requires this after finishing a development task.

- [ ] **Step 5: Run verification-before-completion**

Invoke `superpowers:verification-before-completion` before claiming completion. Report the exact commands run and their results.

- [ ] **Step 6: Final commit if approved**

Only commit if the user asked for commits. Include source changes, tests, and wiki updates:

```bash
git add crates/vol-llm-agent crates/vol-llm-provider crates/vol-llm-agent-channel docs/wiki
git commit -m "feat: support multimodal agent run input"
```

---

## Self-Review

**Spec coverage:**
- `AgentInput` envelope with `run_id`, `parts`, and `metadata`: Task 1.
- `run(&str)` preserved and `run_input(AgentInput)` added: Task 2.
- Text and image URL/data URL parts: Tasks 1 and 3.
- Provided run ID used consistently: Task 2.
- Empty parts rejected before LLM call: Task 2.
- Anthropic multipart conversion: Task 3.
- Channel old string and new structured input compatibility: Task 4.
- Wiki update after development: Task 5.

**Placeholder scan:** No unresolved TBD/TODO placeholders are intentionally left in the plan. The only conditional instruction is choosing an existing `LLMError` variant if the exact variant does not exist; this is bounded to a single file and includes the fallback implementation.

**Type consistency:** The plan consistently uses `AgentInput`, `InputPart`, `run_input`, `MessageContent::MultiPart`, and `AgentRequest::with_input`/`with_id_and_input` across tasks.
