//! LLM Client trait.

use crate::{ConversationRequest, ConversationResponse, LLMProvider, Result, StreamReceiver};
use async_trait::async_trait;

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
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse>;

    /// Send streaming conversation request
    async fn converse_stream(&self, request: ConversationRequest) -> Result<StreamReceiver>;
}
