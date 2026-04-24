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
use vol_llm_core::{LLMClient, message::MessageRole};

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
    /// If LLM compression fails, the conversation summary is skipped and
    /// the original conversation messages are included uncompressed (graceful degradation).
    pub async fn compress(&self, messages: Vec<SessionMessage>) -> Vec<SessionMessage> {
        if messages.is_empty() {
            return vec![];
        }

        if messages.len() <= self.keep_last {
            return messages;
        }

        let split_at = messages.len() - self.keep_last;
        let (history_slice, recent_slice) = messages.split_at(split_at);
        let recent: Vec<SessionMessage> = recent_slice.to_vec();

        // Partition history into tool vs conversation messages (collect references into Vec)
        let tool_msgs: Vec<&SessionMessage> = history_slice.iter().filter(|sm| {
            matches!(sm.message.role, MessageRole::Tool)
        }).collect();
        let conv_msgs: Vec<&SessionMessage> = history_slice.iter().filter(|sm| {
            !matches!(sm.message.role, MessageRole::Tool)
        }).collect();

        let mut result = Vec::new();

        // Compress tool messages
        if !tool_msgs.is_empty() {
            let owned: Vec<SessionMessage> = tool_msgs.iter().map(|m| (*m).clone()).collect();
            if let Some(tool_summary) = self.tool_compressor.compress(&owned) {
                result.push(tool_summary);
            }
        }

        // Compress conversation messages
        if !conv_msgs.is_empty() {
            let owned: Vec<SessionMessage> = conv_msgs.iter().map(|m| (*m).clone()).collect();
            if let Some(conv_summary) = self.conv_compressor.compress(&owned).await {
                result.push(conv_summary);
            } else {
                // LLM failed — include full uncompressed history as fallback
                for msg in &conv_msgs {
                    result.push((*msg).clone());
                }
            }
        }

        // Append recent messages (passthrough)
        result.extend(recent);

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{ConversationResponse, FinishReason, Message, TokenUsage};

    struct MockLlm {
        response: String,
        fail: bool,
    }

    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider { vol_llm_core::LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }
        async fn converse(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
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
        async fn converse_stream(&self, _request: vol_llm_core::ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
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

        // Build 9 messages total
        let mut msgs = Vec::new();
        msgs.push(make_msg("s1", MessageRole::User, "Let's build a tool", None));          // 0: history, conv
        msgs.push(make_msg("s1", MessageRole::Assistant, "Good idea", None));               // 1: history, conv
        msgs.push(make_msg("s1", MessageRole::Tool, "result1", Some("call_1")));            // 2: history, tool
        msgs.push(make_msg("s1", MessageRole::Tool, "result2", Some("call_2")));            // 3: history, tool
        msgs.push(make_msg("s1", MessageRole::User, "What about compression?", None));      // 4: recent
        msgs.push(make_msg("s1", MessageRole::Assistant, "Yes let's", None));               // 5: recent
        msgs.push(make_msg("s1", MessageRole::Tool, "result3", Some("call_3")));            // 6: recent
        msgs.push(make_msg("s1", MessageRole::User, "Recent msg 1", None));                 // 7: recent
        msgs.push(make_msg("s1", MessageRole::Assistant, "Recent msg 2", None));            // 8: recent

        // 9 total, keep_last=5, split_at=4
        // History (0..4): User, Assistant, Tool, Tool → conv_msgs=2, tool_msgs=2
        // Recent (4..9): 5 messages passthrough
        // Result: [tool_summary (system), conv_summary (user)] + 5 recent = 7
        let result = compressor.compress(msgs).await;

        assert_eq!(result.len(), 7);
        // First should be tool summary (system)
        assert_eq!(result[0].message.role, MessageRole::System);
        // Second should be conv summary (user)
        assert_eq!(result[1].message.role, MessageRole::User);
        let content = result[1].message.content.as_ref().unwrap().as_str();
        assert!(content.contains("[Session Summary]"));
        assert!(content.contains("They discussed Rust"));
        // Message at index 2 should be first recent message
        assert_eq!(result[2].message.content.as_ref().unwrap().as_str(), "What about compression?");
    }

    #[tokio::test]
    async fn test_compress_llm_fallback() {
        let llm = Arc::new(MockLlm { response: "".to_string(), fail: true });
        let compressor = SessionCompressor::new(llm);

        // 6 messages: keep_last=5, split_at=1
        let mut msgs = Vec::new();
        msgs.push(make_msg("s1", MessageRole::User, "Hello", None));         // 0: history, conv
        msgs.push(make_msg("s1", MessageRole::Assistant, "Hi", None));       // 1: recent
        msgs.push(make_msg("s1", MessageRole::User, "How are you?", None));  // 2: recent
        msgs.push(make_msg("s1", MessageRole::Assistant, "Fine", None));     // 3: recent
        msgs.push(make_msg("s1", MessageRole::User, "Recent", None));        // 4: recent
        msgs.push(make_msg("s1", MessageRole::Assistant, "Recent2", None));  // 5: recent

        // History = msgs[0..1] = [User "Hello"] → conv_msgs = [User "Hello"]
        // LLM fails → conv_msgs included uncompressed (1 message)
        // Result: [conv_msgs uncompressed (1)] + recent (5) = 6
        let result = compressor.compress(msgs).await;
        assert_eq!(result.len(), 6);
        // First message should be the uncompressed conv message from history
        assert_eq!(result[0].message.role, MessageRole::User);
        assert_eq!(result[0].message.content.as_ref().unwrap().as_str(), "Hello");
    }
}
