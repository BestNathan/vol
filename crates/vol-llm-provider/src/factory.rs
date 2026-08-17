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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Secret;

    #[test]
    fn create_provider_returns_anthropic_provider() {
        let config = LLMConfig::with_literal_key(
            LLMProvider::Anthropic,
            "claude-test",
            "sk-test",
            "https://api.test.com",
        );
        let provider = create_provider(&config).unwrap();
        assert_eq!(provider.provider(), LLMProvider::Anthropic);
    }

    #[test]
    fn create_provider_returns_openai_provider() {
        let config = LLMConfig::with_literal_key(
            LLMProvider::OpenAI,
            "gpt-4o",
            "sk-test",
            "https://api.test.com",
        );
        let provider = create_provider(&config).unwrap();
        assert_eq!(provider.provider(), LLMProvider::OpenAI);
        assert_eq!(provider.model(), "gpt-4o");
    }

    #[test]
    fn create_provider_propagates_auth_error() {
        std::env::remove_var("FACTORY_MISSING_KEY");
        let config = LLMConfig::with_env_key(
            LLMProvider::OpenAI,
            "gpt-4o",
            "FACTORY_MISSING_KEY",
            "https://api.test.com",
        );
        let err = create_provider(&config).err().unwrap();
        assert!(matches!(err, LLMError::Auth(_)));
    }

    #[test]
    fn create_provider_preserves_secret() {
        let config = LLMConfig::new(
            LLMProvider::Anthropic,
            "claude-test",
            Secret::env_with_default("FACTORY_FALLBACK_KEY", "fallback-value"),
            "https://api.test.com",
        );
        let provider = create_provider(&config).unwrap();
        assert_eq!(provider.provider(), LLMProvider::Anthropic);
    }
}
