//! Provider factory functions.

use crate::{AnthropicProvider, LLMConfig};
use vol_llm_core::{LLMClient, LLMError, LLMProvider};

/// Create provider from config
pub fn create_provider(config: &LLMConfig) -> Result<Box<dyn LLMClient>, LLMError> {
    match config.provider {
        LLMProvider::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        // OpenAI provider can be added in the future
        #[allow(unreachable_patterns)]
        _ => Err(LLMError::Parse("Provider not implemented".to_string())),
    }
}
