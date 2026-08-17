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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display() {
        assert_eq!(LLMProvider::Anthropic.to_string(), "anthropic");
        assert_eq!(LLMProvider::OpenAI.to_string(), "openai");
    }

    #[test]
    fn test_serde_roundtrip_lowercase() {
        assert_eq!(
            serde_json::to_string(&LLMProvider::Anthropic).unwrap(),
            r#""anthropic""#
        );
        assert_eq!(
            serde_json::to_string(&LLMProvider::OpenAI).unwrap(),
            r#""openai""#
        );
        let parsed: LLMProvider = serde_json::from_str(r#""openai""#).unwrap();
        assert_eq!(parsed, LLMProvider::OpenAI);
        let parsed: LLMProvider = serde_json::from_str(r#""anthropic""#).unwrap();
        assert_eq!(parsed, LLMProvider::Anthropic);
    }

    #[test]
    fn test_partial_eq_copy() {
        assert_eq!(LLMProvider::Anthropic, LLMProvider::Anthropic);
        assert_ne!(LLMProvider::Anthropic, LLMProvider::OpenAI);
    }
}
