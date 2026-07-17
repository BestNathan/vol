//! LLM-driven conversation compressor.

use std::sync::Arc;
use vol_llm_core::{ConversationRequest, LLMClient, Message};
use vol_session::SessionMessage;

pub struct ConversationCompressor {
    llm: Arc<dyn LLMClient>,
}

impl ConversationCompressor {
    pub fn new(llm: Arc<dyn LLMClient>) -> Self {
        Self { llm }
    }

    /// Compress user/assistant messages into a single user message summary.
    ///
    /// If the LLM call fails, returns None (caller should handle fallback).
    pub async fn compress(&self, messages: &[SessionMessage]) -> Option<SessionMessage> {
        if messages.is_empty() {
            return None;
        }

        let system_prompt = r#"You are a session compressor. Summarize the following conversation into a single paragraph. Focus on:
1. Key decisions made
2. Code changes proposed or implemented
3. Open issues or unresolved questions
Be concise. Output only the summary."#;

        let serialized: String = messages
            .iter()
            .filter_map(|sm| {
                let role = match sm.message.role {
                    vol_llm_core::message::MessageRole::User => "User",
                    vol_llm_core::message::MessageRole::Assistant => "Assistant",
                    _ => return None,
                };
                let content = sm
                    .message
                    .content
                    .as_ref()
                    .map(vol_llm_core::MessageContent::as_str)
                    .unwrap_or("");
                Some(format!("{role}: {content}"))
            })
            .collect::<Vec<_>>()
            .join("\n\n");

        let request = ConversationRequest::with_system(system_prompt, &serialized);

        match self.llm.converse(request).await {
            Ok(response) => {
                let summary_text = response
                    .message
                    .content
                    .as_ref()
                    .map(vol_llm_core::MessageContent::as_str)
                    .unwrap_or("")
                    .to_string();

                if summary_text.is_empty() {
                    return None;
                }

                let prefixed = format!("[Session Summary]: {summary_text}");
                let session_id = messages
                    .first()
                    .map(|m| m.session_id.clone())
                    .unwrap_or_default();
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

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::{message::MessageRole, ConversationResponse, FinishReason, TokenUsage};

    struct MockLlm {
        response: String,
    }

    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> vol_llm_core::LLMProvider {
            vol_llm_core::LLMProvider::Anthropic
        }
        fn model(&self) -> &str {
            "mock"
        }
        fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
            &[]
        }
        async fn converse(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<ConversationResponse> {
            Ok(ConversationResponse {
                message: Message::user(self.response.clone()),
                model: "mock".to_string(),
                usage: TokenUsage::default(),
                finish_reason: FinishReason::Stop,
                raw: None,
            })
        }
        async fn converse_stream(
            &self,
            _request: ConversationRequest,
        ) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
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
        let llm = Arc::new(MockLlm {
            response: "ignored".to_string(),
        });
        let compressor = ConversationCompressor::new(llm);
        assert!(compressor.compress(&[]).await.is_none());
    }

    #[tokio::test]
    async fn test_compress_returns_summary() {
        let llm = Arc::new(MockLlm {
            response: "Decided to use Rust".to_string(),
        });
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
        let llm = Arc::new(MockLlm {
            response: "".to_string(),
        });
        let compressor = ConversationCompressor::new(llm);
        let msgs = vec![make_conv_msg("s1", MessageRole::User, "Hello")];
        assert!(compressor.compress(&msgs).await.is_none());
    }
}
