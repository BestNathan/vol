//! Provider factory functions.

use crate::{AnthropicProvider, LLMConfig, OpenaiProvider};
use vol_llm_core::{LLMClient, LLMError, LLMProvider};

/// Create provider from config
pub fn create_provider(config: &LLMConfig) -> Result<Box<dyn LLMClient>, LLMError> {
    match config.provider {
        LLMProvider::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        LLMProvider::OpenAI => Ok(Box::new(OpenaiProvider::new(config)?)),
    }
}
