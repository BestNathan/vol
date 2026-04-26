# Session Compressor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement two-layer session compression — rule-based tool call summarization + LLM-driven conversation summarization — to reduce session message history while preserving semantic meaning.

**Architecture:** Three components in `vol-llm-agents/src/coding/compressor/`: `ToolCallCompressor` (rule-based, truncates tool results), `ConversationCompressor` (LLM-driven, summarizes dialogue into single user message), and `SessionCompressor` (orchestrator that splits messages, delegates to sub-compressors, merges results).

**Tech Stack:** async-trait, tokio, vol-llm-core (LLMClient, Message), vol-session (SessionMessage)

---

## Context

**Problem:** Session message history grows without bound. When sessions reach token budget limits, the only compression mechanism is dropping messages — losing information rather than summarizing it.

**Solution:** LLM-driven compression that preserves semantic meaning. Two layers:
1. `ToolCallCompressor`: Rule-based, iterates tool messages, extracts name/args/result (truncated), produces summary lines
2. `ConversationCompressor`: LLM-driven, sends user/assistant messages to LLM with summary prompt, returns single user message

Output: `[tool_summary_msg (system), conv_summary_msg (user)] + last 5 original messages`

See [docs/superpowers/specs/2026-04-24-session-compressor-design.md](docs/superpowers/specs/2026-04-24-session-compressor-design.md) for full design spec.

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-llm-agents/src/coding/compressor/tool_call.rs` | Create | ToolCallCompressor |
| `crates/vol-llm-agents/src/coding/compressor/conversation.rs` | Create | ConversationCompressor |
| `crates/vol-llm-agents/src/coding/compressor/mod.rs` | Create | SessionCompressor + re-exports |
| `crates/vol-llm-agents/src/coding/compressor/tests.rs` | Create | Integration tests |
| `crates/vol-llm-agents/src/coding/mod.rs` | Modify | Add `mod compressor;` + re-export |
| `crates/vol-llm-agents/Cargo.toml` | Modify | Add vol-session dependency |

---

### Task 1: Implement ToolCallCompressor

**Files:**
- Create: `crates/vol-llm-agents/src/coding/compressor/tool_call.rs`
- Create: `crates/vol-llm-agents/src/coding/compressor/mod.rs` (skeleton)
- Modify: `crates/vol-llm-agents/src/coding/mod.rs`
- Modify: `crates/vol-llm-agents/Cargo.toml`

- [ ] **Step 1: Add vol-session dependency to Cargo.toml**

Add to `crates/vol-llm-agents/Cargo.toml` under `[dependencies]`:
```toml
vol-session = { path = "../vol-session" }
```

- [ ] **Step 2: Write tool_call.rs**

```rust
//! Rule-based tool call result compressor.
//!
//! Iterates over tool messages, extracts tool name, arguments (truncated),
//! and result (truncated), producing summary lines.

use vol_session::SessionMessage;

/// Max chars for tool args in summary
const TOOL_ARGS_MAX: usize = 200;
/// Max chars for tool result in summary
const TOOL_RESULT_MAX: usize = 500;

/// Rule-based compressor for tool call messages.
pub struct ToolCallCompressor;

impl ToolCallCompressor {
    /// Compress a batch of tool messages into a single system message summary.
    ///
    /// Returns a `Message::system` with one summary line per tool call.
    /// If input is empty, returns None.
    pub fn compress(&self, messages: &[SessionMessage]) -> Option<SessionMessage> {
        if messages.is_empty() {
            return None;
        }

        let summary_lines: Vec<String> = messages
            .iter()
            .filter_map(|sm| self.compress_one(sm))
            .collect();

        if summary_lines.is_empty() {
            return None;
        }

        let summary = summary_lines.join("\n");
        // Create a system message with the summary
        // We need to construct a SessionMessage — use the first message's session_id
        let session_id = messages.first().map(|m| m.session_id.clone()).unwrap_or_default();
        let system_msg = vol_llm_core::Message::system(summary);
        Some(SessionMessage::new(session_id, system_msg))
    }

    fn compress_one(&self, msg: &SessionMessage) -> Option<String> {
        let tool_name = msg.message.name.as_deref().unwrap_or("unknown");
        let args = msg.message.content.as_ref().map(|c| c.as_str()).unwrap_or("");
        let result = msg.message.content.as_ref().map(|c| c.as_str()).unwrap_or("");

        let args_truncated = truncate(args, TOOL_ARGS_MAX);
        let result_truncated = truncate(result, TOOL_RESULT_MAX);

        Some(format!("[{}] {} → {}", tool_name, args_truncated, result_truncated))
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.min(s.len())])
    }
}
```

- [ ] **Step 3: Write compressor/mod.rs skeleton**

```rust
//! Session compression module.
//!
//! Two-layer compression: rule-based tool call summarization + LLM-driven conversation summarization.

mod tool_call;

pub use tool_call::ToolCallCompressor;
```

- [ ] **Step 4: Update coding/mod.rs**

Add to `crates/vol-llm-agents/src/coding/mod.rs`:
```rust
mod compressor;
```

And add to the `pub use` section:
```rust
pub use compressor::ToolCallCompressor;
```

- [ ] **Step 5: Write tool_call tests inline**

Add to `crates/vol-llm-agents/src/coding/compressor/tool_call.rs` (at end of file):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::Message;

    fn make_tool_msg(session_id: &str, name: &str, args: &str, result: &str) -> SessionMessage {
        let msg = Message::tool(result.to_string(), "call_1".to_string())
            .with_name(name.to_string());
        SessionMessage::new(session_id.to_string(), msg)
    }

    impl Message {
        fn with_name(mut self, name: String) -> Self {
            self.name = Some(name);
            self
        }
    }

    #[test]
    fn test_compress_empty() {
        let compressor = ToolCallCompressor;
        assert!(compressor.compress(&[]).is_none());
    }

    #[test]
    fn test_compress_single_tool() {
        let compressor = ToolCallCompressor;
        let msgs = vec![make_tool_msg("s1", "bash", "ls -la", "total 42")];
        let result = compressor.compress(&msgs).unwrap();
        assert_eq!(result.message.role, vol_llm_core::message::MessageRole::System);
        let content = result.message.content.unwrap().as_str().to_string();
        assert!(content.contains("[bash]"));
        assert!(content.contains("ls -la"));
        assert!(content.contains("total 42"));
    }

    #[test]
    fn test_compress_multiple_tools() {
        let compressor = ToolCallCompressor;
        let msgs = vec![
            make_tool_msg("s1", "read_file", "{\"path\": \"test.rs\"}", "fn main() {}"),
            make_tool_msg("s1", "bash", "cargo check", "Finished dev"),
        ];
        let result = compressor.compress(&msgs).unwrap();
        let content = result.message.content.unwrap().as_str().to_string();
        assert!(content.contains("[read_file]"));
        assert!(content.contains("[bash]"));
        assert!(content.contains("cargo check"));
    }

    #[test]
    fn test_truncate_long_content() {
        let long_args = "a".repeat(300);
        let result = super::truncate(&long_args, TOOL_ARGS_MAX);
        assert!(result.len() == TOOL_ARGS_MAX + 3); // "..." appended
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_short_content() {
        let result = super::truncate("short", 200);
        assert_eq!(result, "short");
    }
}
```

Note: `Message::tool` creates a message with `name: None`. The test needs a way to set the name field. Since `Message` doesn't have a `with_name` builder, we need to construct it directly. Let me adjust the test helper:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{Message, MessageContent};

    fn make_tool_msg(session_id: &str, name: &str, args: &str, result: &str) -> SessionMessage {
        let mut msg = Message::tool(result.to_string(), "call_1".to_string());
        msg.name = Some(name.to_string());
        SessionMessage::new(session_id.to_string(), msg)
    }

    #[test]
    fn test_compress_empty() {
        let compressor = ToolCallCompressor;
        assert!(compressor.compress(&[]).is_none());
    }

    #[test]
    fn test_compress_single_tool() {
        let compressor = ToolCallCompressor;
        let msgs = vec![make_tool_msg("s1", "bash", "ls -la", "total 42")];
        let result = compressor.compress(&msgs).unwrap();
        assert_eq!(result.message.role, vol_llm_core::message::MessageRole::System);
        let content = result.message.content.as_ref().unwrap().as_str();
        assert!(content.contains("[bash]"));
        assert!(content.contains("ls -la"));
        assert!(content.contains("total 42"));
    }

    #[test]
    fn test_compress_multiple_tools() {
        let compressor = ToolCallCompressor;
        let msgs = vec![
            make_tool_msg("s1", "read_file", "{\"path\": \"test.rs\"}", "fn main() {}"),
            make_tool_msg("s1", "bash", "cargo check", "Finished dev"),
        ];
        let result = compressor.compress(&msgs).unwrap();
        let content = result.message.content.as_ref().unwrap().as_str();
        assert!(content.contains("[read_file]"));
        assert!(content.contains("[bash]"));
    }

    #[test]
    fn test_truncate_long_content() {
        let long_args = "a".repeat(300);
        let result = super::truncate(&long_args, TOOL_ARGS_MAX);
        assert_eq!(result.len(), TOOL_ARGS_MAX + 3);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_short_content() {
        let result = super::truncate("short", 200);
        assert_eq!(result, "short");
    }
}
```

- [ ] **Step 6: Verify compilation**

```bash
cargo check -p vol-llm-agents
```

Expected: Compiles successfully.

- [ ] **Step 7: Run tests**

```bash
cargo test -p vol-llm-agents coding::compressor::tool_call
```

Expected: 5 tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agents/src/coding/compressor/tool_call.rs crates/vol-llm-agents/src/coding/compressor/mod.rs crates/vol-llm-agents/src/coding/mod.rs crates/vol-llm-agents/Cargo.toml
git commit -m "feat: add ToolCallCompressor for rule-based tool result summarization"
```

---

### Task 2: Implement ConversationCompressor

**Files:**
- Create: `crates/vol-llm-agents/src/coding/compressor/conversation.rs`
- Modify: `crates/vol-llm-agents/src/coding/compressor/mod.rs`

- [ ] **Step 1: Write conversation.rs**

```rust
//! LLM-driven conversation compressor.
//!
//! Sends user/assistant messages to the LLM with a summary prompt,
//! returns a single user message with the condensed summary.

use std::sync::Arc;
use vol_session::SessionMessage;
use vol_llm_core::{LLMClient, ConversationRequest, Message};

/// LLM-driven compressor for user/assistant dialogue.
pub struct ConversationCompressor {
    llm: Arc<dyn LLMClient>,
}

impl ConversationCompressor {
    /// Create a new compressor with the given LLM client.
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// Compress a batch of user/assistant messages into a single user message.
    ///
    /// Sends the messages to the LLM with a system prompt requesting a structured
    /// summary of key decisions, code changes, and open issues.
    ///
    /// Returns a `Message::user` with the summary text, prefixed with `[Session Summary]: `.
    /// If input is empty, returns None.
    pub async fn compress(&self, messages: &[SessionMessage]) -> Option<SessionMessage> {
        if messages.is_empty() {
            return None;
        }

        let system_prompt = r#"You are a session compressor. Summarize the following conversation into a single paragraph. Focus on:
1. Key decisions made
2. Code changes proposed or implemented
3. Open issues or unresolved questions
Be concise. Output only the summary."#;

        // Serialize messages into a single prompt
        let serialized: Vec<String> = messages
            .iter()
            .filter_map(|sm| {
                let role = match sm.message.role {
                    vol_llm_core::message::MessageRole::User => "User",
                    vol_llm_core::message::MessageRole::Assistant => "Assistant",
                    _ => return None,
                };
                let content = sm.message.content.as_ref().map(|c| c.as_str()).unwrap_or("");
                Some(format!("{}: {}", role, content))
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let request = ConversationRequest::with_system(system_prompt, &serialized);

        match self.llm.converse(request).await {
            Ok(response) => {
                let summary_text = response.message.content
                    .as_ref()
                    .map(|c| c.as_str())
                    .unwrap_or("")
                    .to_string();

                if summary_text.is_empty() {
                    return None;
                }

                let prefixed = format!("[Session Summary]: {}", summary_text);
                let session_id = messages.first().map(|m| m.session_id.clone()).unwrap_or_default();
                let user_msg = Message::user(prefixed);
                Some(SessionMessage::new(session_id, user_msg))
            }
            Err(e) => {
                tracing::warn!(error = %e, "ConversationCompressor: LLM call failed, returning None (caller should handle fallback)");
                None
            }
        }
    }
}
```

- [ ] **Step 2: Update mod.rs**

Update `crates/vol-llm-agents/src/coding/compressor/mod.rs`:

```rust
//! Session compression module.
//!
//! Two-layer compression: rule-based tool call summarization + LLM-driven conversation summarization.

mod conversation;
mod tool_call;

pub use conversation::ConversationCompressor;
pub use tool_call::ToolCallCompressor;
```

- [ ] **Step 3: Add conversation compressor tests**

Append to `crates/vol-llm-agents/src/coding/compressor/conversation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{ConversationResponse, FinishReason, MessageContent, TokenUsage, message::MessageRole};

    // Dummy LLM that returns a fixed summary
    struct MockLlm {
        response: String,
    }
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
            Ok(ConversationResponse {
                message: Message::user(self.response.clone()),
                model: "mock".to_string(),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }
        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
            unimplemented!()
        }
    }

    fn make_conv_msg(session_id: &str, role: MessageRole, content: &str) -> SessionMessage {
        let msg = match role {
            MessageRole::User => Message::user(content.to_string()),
            MessageRole::Assistant => Message::assistant(content.to_string()),
            _ => panic!("only user/assistant roles supported"),
        };
        SessionMessage::new(session_id.to_string(), msg)
    }

    #[tokio::test]
    async fn test_compress_empty() {
        let llm = Arc::new(MockLlm { response: "ignored".to_string() });
        let compressor = ConversationCompressor::new(llm);
        assert!(compressor.compress(&[]).await.is_none());
    }

    #[tokio::test]
    async fn test_compress_returns_summary() {
        let llm = Arc::new(MockLlm { response: "Decided to use Rust".to_string() });
        let compressor = ConversationCompressor::new(llm);
        let msgs = vec![
            make_conv_msg("s1", MessageRole::User, "What language?"),
            make_conv_msg("s1", MessageRole::Assistant, "Let's use Rust"),
        ];
        let result = compressor.compress(&msgs).await.unwrap();
        assert_eq!(result.message.role, MessageRole::User);
        let content = result.message.content.as_ref().unwrap().as_str();
        assert!(content.contains("[Session Summary]"));
        assert!(content.contains("Decided to use Rust"));
    }

    #[tokio::test]
    async fn test_compress_empty_llm_response() {
        let llm = Arc::new(MockLlm { response: "".to_string() });
        let compressor = ConversationCompressor::new(llm);
        let msgs = vec![make_conv_msg("s1", MessageRole::User, "Hello")];
        assert!(compressor.compress(&msgs).await.is_none());
    }
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-agents
```

Expected: Compiles successfully.

- [ ] **Step 5: Run tests**

```bash
cargo test -p vol-llm-agents coding::compressor::conversation
```

Expected: 3 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agents/src/coding/compressor/conversation.rs crates/vol-llm-agents/src/coding/compressor/mod.rs
git commit -m "feat: add ConversationCompressor for LLM-driven dialogue summarization"
```

---

### Task 3: Implement SessionCompressor Orchestrator

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/compressor/mod.rs`

- [ ] **Step 1: Write SessionCompressor in mod.rs**

Update `crates/vol-llm-agents/src/coding/compressor/mod.rs` to full content:

```rust
//! Session compression module.
//!
//! Two-layer compression: rule-based tool call summarization + LLM-driven conversation summarization.
//!
//! # Architecture
//!
//! ```text
//! SessionCompressor
//! ├── split(messages, keep_last: 5)
//! │   ├── history → separate tool vs conversation messages
//! │   └── recent  → passthrough (last 5 messages)
//! ├── ToolCallCompressor.compress(tool_messages)
//! │   └── Returns: 1x system message summarizing all tool calls
//! ├── ConversationCompressor.compress(user_assistant_messages, llm)
//! │   └── Returns: 1x user message summarizing key decisions/code/issues
//! └── Merge: [tool_summary?, conv_summary?] + recent
//! ```

mod conversation;
mod tool_call;

pub use conversation::ConversationCompressor;
pub use tool_call::ToolCallCompressor;

use std::sync::Arc;
use vol_session::SessionMessage;
use vol_llm_core::{LLMClient, Message, message::MessageRole};

/// Number of recent messages to preserve untouched
const KEEP_LAST: usize = 5;

/// Orchestrator for session-wide compression.
///
/// Splits messages into history (compressible) and recent (passthrough),
/// delegates to sub-compressors, and merges results.
pub struct SessionCompressor {
    tool_compressor: ToolCallCompressor,
    conv_compressor: ConversationCompressor,
    keep_last: usize,
}

impl SessionCompressor {
    /// Create a new SessionCompressor with the given LLM client.
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self {
            tool_compressor: ToolCallCompressor,
            conv_compressor: ConversationCompressor::new(llm),
            keep_last: KEEP_LAST,
        }
    }

    /// Compress a batch of session messages.
    ///
    /// If `messages.len() <= keep_last`, returns as-is (nothing to compress).
    /// Otherwise:
    /// 1. Split: history = messages[0..len-keep_last], recent = messages[len-keep_last..]
    /// 2. From history: partition into tool_msgs and conv_msgs
    /// 3. Compress tool messages → system message (if any)
    /// 4. Compress conversation messages → user message via LLM (if any)
    /// 5. Merge: [tool_summary?, conv_summary?] + recent
    ///
    /// If LLM compression fails, the conversation summary is skipped (graceful degradation).
    pub async fn compress(&self, messages: Vec<SessionMessage>) -> Vec<SessionMessage> {
        if messages.is_empty() {
            return vec![];
        }

        if messages.len() <= self.keep_last {
            return messages;
        }

        let split_at = messages.len() - self.keep_last;
        let history = &messages[..split_at];
        let recent = messages[split_at..].to_vec();

        // Partition history into tool vs conversation messages
        let (tool_msgs, conv_msgs): (Vec<_>, Vec<_>) = history.iter().partition(|sm| {
            matches!(sm.message.role, MessageRole::Tool)
        });

        let mut result = Vec::new();

        // Compress tool messages
        if !tool_msgs.is_empty() {
            if let Some(tool_summary) = self.tool_compressor.compress(&tool_msgs) {
                result.push(tool_summary);
            }
        }

        // Compress conversation messages
        if !conv_msgs.is_empty() {
            if let Some(conv_summary) = self.conv_compressor.compress(&conv_msgs).await {
                result.push(conv_summary);
            } else {
                // LLM failed — include full uncompressed history as fallback
                result.extend(conv_msgs.into_iter().cloned());
            }
        }

        // Append recent messages (passthrough)
        result.extend(recent);

        result
    }
}
```

- [ ] **Step 2: Add SessionCompressor tests to mod.rs**

Append to `crates/vol-llm-agents/src/coding/compressor/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{ConversationResponse, FinishReason, MessageContent, TokenUsage};

    // Mock LLM for testing
    struct MockLlm {
        response: String,
        fail: bool,
    }
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
            if self.fail {
                Err(vol_llm_core::LLMError::Api { status: 500, message: "LLM failed".to_string() })
            } else {
                Ok(ConversationResponse {
                    message: Message::user(self.response.clone()),
                    model: "mock".to_string(),
                    usage: TokenUsage::default(),
                    finish_reason: FinishReason::Stop,
                    raw: None,
                })
            }
        }
        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
            unimplemented!()
        }
    }

    fn make_msg(session_id: &str, role: MessageRole, content: &str, call_id: Option<&str>) -> SessionMessage {
        let msg = match role {
            MessageRole::System => Message::system(content.to_string()),
            MessageRole::User => Message::user(content.to_string()),
            MessageRole::Assistant => Message::assistant(content.to_string()),
            MessageRole::Tool => {
                let mut m = Message::tool(content.to_string(), call_id.unwrap_or("call_1").to_string());
                m.name = Some("test_tool".to_string());
                m
            }
        };
        SessionMessage::new(session_id.to_string(), msg)
    }

    #[tokio::test]
    async fn test_compress_empty() {
        let llm = Arc::new(MockLlm { response: "ok".to_string(), fail: false });
        let compressor = SessionCompressor::new(llm);
        assert!(compressor.compress(vec![]).await.is_empty());
    }

    #[tokio::test]
    async fn test_compress_under_threshold() {
        let llm = Arc::new(MockLlm { response: "ok".to_string(), fail: false });
        let compressor = SessionCompressor::new(llm);
        let msgs = vec![
            make_msg("s1", MessageRole::User, "hello", None),
            make_msg("s1", MessageRole::Assistant, "hi", None),
        ];
        let result = compressor.compress(msgs.clone()).await;
        assert_eq!(result.len(), 2); // returned as-is
    }

    #[tokio::test]
    async fn test_compress_mixed_history() {
        let llm = Arc::new(MockLlm { response: "They discussed Rust".to_string(), fail: false });
        let compressor = SessionCompressor::new(llm);

        // Build 8 messages: 3 tool + 3 conversation + 2 recent
        let mut msgs = Vec::new();
        msgs.push(make_msg("s1", MessageRole::User, "Let's build a tool", None));
        msgs.push(make_msg("s1", MessageRole::Assistant, "Good idea", None));
        msgs.push(make_msg("s1", MessageRole::Tool, "result1", Some("call_1")));
        msgs.push(make_msg("s1", MessageRole::Tool, "result2", Some("call_2")));
        msgs.push(make_msg("s1", MessageRole::User, "What about compression?", None));
        msgs.push(make_msg("s1", MessageRole::Assistant, "Yes let's", None));
        msgs.push(make_msg("s1", MessageRole::Tool, "result3", Some("call_3")));
        msgs.push(make_msg("s1", MessageRole::User, "Recent msg 1", None)); // recent
        msgs.push(make_msg("s1", MessageRole::Assistant, "Recent msg 2", None)); // recent

        let result = compressor.compress(msgs).await;

        // Expected: [tool_summary (system), conv_summary (user)] + 2 recent = 4
        // The keep_last is 5, so history = msgs[0..4], recent = msgs[4..9] = 5 messages
        // Wait — 9 total, keep_last=5, split_at=4. history = msgs[0..4], recent = msgs[4..9]
        // History msgs[0..4]: User, Assistant, Tool, Tool
        // Recent msgs[4..9]: User(compression?), Assistant(yes), Tool(result3), User(recent1), Assistant(recent2) = 5
        // tool_msgs in history: 2 (result1, result2) → 1 summary
        // conv_msgs in history: 2 (User "Let's build", Assistant "Good idea") → 1 summary
        // Result: [tool_summary, conv_summary] + 5 recent = 7
        assert_eq!(result.len(), 7);
        // First should be tool summary (system)
        assert_eq!(result[0].message.role, MessageRole::System);
        // Second should be conv summary (user)
        assert_eq!(result[1].message.role, MessageRole::User);
        // Last 5 should be the recent passthrough
        assert_eq!(result[2].message.content.as_ref().unwrap().as_str(), "What about compression?");
    }

    #[tokio::test]
    async fn test_compress_llm_fallback() {
        let llm = Arc::new(MockLlm { response: "".to_string(), fail: true });
        let compressor = SessionCompressor::new(llm);

        let mut msgs = Vec::new();
        msgs.push(make_msg("s1", MessageRole::User, "Hello", None));
        msgs.push(make_msg("s1", MessageRole::Assistant, "Hi", None));
        msgs.push(make_msg("s1", MessageRole::User, "How are you?", None));
        msgs.push(make_msg("s1", MessageRole::Assistant, "Fine", None));
        msgs.push(make_msg("s1", MessageRole::User, "Recent", None));
        msgs.push(make_msg("s1", MessageRole::Assistant, "Recent2", None));

        let result = compressor.compress(msgs).await;

        // LLM fails → conv_msgs included uncompressed
        // history = msgs[0..1] (6 - 5 = 1), recent = msgs[1..6] (5 messages)
        // Wait: len=6, keep_last=5, split_at=1. history = msgs[0..1] = [User "Hello"]
        // conv_msgs = [User "Hello"] (1 message), tool_msgs = []
        // LLM fails → conv_msgs included as-is (1 message)
        // Result: [conv_msgs uncompressed (1)] + recent (5) = 6
        assert_eq!(result.len(), 6);
    }
}
```

- [ ] **Step 3: Update coding/mod.rs exports**

Update `crates/vol-llm-agents/src/coding/mod.rs` to also export `SessionCompressor` and `ConversationCompressor`:

```rust
pub use compressor::{ToolCallCompressor, ConversationCompressor, SessionCompressor};
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-agents
```

Expected: Compiles successfully.

- [ ] **Step 5: Run tests**

```bash
cargo test -p vol-llm-agents coding::compressor
```

Expected: All compressor tests pass (5 tool_call + 3 conversation + 4 session = 12 total).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agents/src/coding/compressor/mod.rs crates/vol-llm-agents/src/coding/mod.rs
git commit -m "feat: add SessionCompressor orchestrator with graceful LLM fallback"
```

---

### Task 4: Full Workspace Verification

**Files:** No new files — verification only.

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

Expected: No errors.

- [ ] **Step 2: Run all vol-llm-agents tests**

```bash
cargo test -p vol-llm-agents
```

Expected: All existing tests pass + new compressor tests (12 total).

- [ ] **Step 3: Run all workspace tests**

```bash
cargo test --workspace
```

Expected: All tests pass, no regressions.

- [ ] **Step 4: Commit if any fixes needed**

```bash
git add -A
git commit -m "fix: resolve compilation/test issues from session compressor"
```

---

## Summary of Changes

| Crate | Files Changed | Purpose |
|-------|---------------|---------|
| `vol-llm-agents` | **new**: `src/coding/compressor/tool_call.rs`, `src/coding/compressor/conversation.rs` | Compressor implementations |
| `vol-llm-agents` | **modify**: `src/coding/compressor/mod.rs`, `src/coding/mod.rs`, `Cargo.toml` | Module wiring + dependency |

### Compressor Module Structure

| File | Purpose |
|------|---------|
| `compressor/tool_call.rs` | ToolCallCompressor — rule-based tool result summarization |
| `compressor/conversation.rs` | ConversationCompressor — LLM-driven dialogue summarization |
| `compressor/mod.rs` | SessionCompressor orchestrator + re-exports + integration tests |
