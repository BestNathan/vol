//! Secret value that supports literal strings and environment variable references.

use serde::{Deserialize, Serialize};
use vol_llm_core::LLMError;

/// A secret value that can be either a literal string or an environment variable reference.
///
/// # Examples
///
/// Literal value:
/// ```toml
/// api_key = "sk-xxx-actual-key"
/// ```
///
/// Environment variable:
/// ```toml
/// api_key = "${API_KEY}"
/// ```
///
/// Environment variable with default:
/// ```toml
/// api_key = "${API_KEY:sk-fallback-key}"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Secret {
    /// Direct literal value
    Literal(String),
    /// Environment variable reference with optional default
    Env {
        env: String,
        #[serde(default)]
        default: Option<String>,
    },
}

impl Secret {
    /// Create a literal secret
    pub fn literal(value: impl Into<String>) -> Self {
        Secret::Literal(value.into())
    }

    /// Create an env-based secret without default
    pub fn env(var_name: impl Into<String>) -> Self {
        Secret::Env {
            env: var_name.into(),
            default: None,
        }
    }

    /// Create an env-based secret with default
    pub fn env_with_default(var_name: impl Into<String>, default: impl Into<String>) -> Self {
        Secret::Env {
            env: var_name.into(),
            default: Some(default.into()),
        }
    }

    /// Resolve the secret to a concrete value
    ///
    /// - For Literal: returns the value directly
    /// - For Env: reads from environment, falls back to default if set
    pub fn resolve(&self) -> Result<String, LLMError> {
        match self {
            Secret::Literal(s) => Ok(s.clone()),
            Secret::Env { env, default } => {
                std::env::var(env).or_else(|_| {
                    default.clone().ok_or_else(|| {
                        LLMError::Auth(format!("Environment variable '{}' not set", env))
                    })
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_resolve() {
        let secret = Secret::literal("my-secret-key");
        assert_eq!(secret.resolve().unwrap(), "my-secret-key");
    }

    #[test]
    fn test_env_resolve() {
        // Set a test env var
        std::env::set_var("TEST_SECRET_KEY", "env-value");
        let secret = Secret::env("TEST_SECRET_KEY");
        assert_eq!(secret.resolve().unwrap(), "env-value");
    }

    #[test]
    fn test_env_with_default_resolves_to_env() {
        std::env::set_var("TEST_WITH_DEFAULT", "env-value");
        let secret = Secret::env_with_default("TEST_WITH_DEFAULT", "default-value");
        assert_eq!(secret.resolve().unwrap(), "env-value");
    }

    #[test]
    fn test_env_with_default_resolves_to_default() {
        // Ensure env var does not exist
        std::env::remove_var("TEST_NONEXISTENT");
        let secret = Secret::env_with_default("TEST_NONEXISTENT", "default-value");
        assert_eq!(secret.resolve().unwrap(), "default-value");
    }

    #[test]
    fn test_env_without_default_fails() {
        std::env::remove_var("TEST_MUST_FAIL");
        let secret = Secret::env("TEST_MUST_FAIL");
        assert!(secret.resolve().is_err());
    }
}
