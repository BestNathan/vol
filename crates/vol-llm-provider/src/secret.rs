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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

impl<'de> Deserialize<'de> for Secret {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Accept either the serialized (tagged) form `{"Literal": ...}` /
        // `{"Env": {"env": ..., "default": ...}}`, or a plain string (with the
        // `${VAR}` / `${VAR:default}` env-reference pattern).
        deserializer.deserialize_any(SecretVisitor)
    }
}

/// Wire shape of the `Env` variant inside the tagged form (matches the
/// derived `Serialize` output of `Secret::Env`).
#[derive(Deserialize)]
struct EnvSecretWire {
    env: String,
    #[serde(default)]
    default: Option<String>,
}

/// Parse a plain wire string: `${VAR}` / `${VAR:default}` become `Env`,
/// anything else is a `Literal`.
fn secret_from_env_pattern(s: &str) -> Secret {
    // Check for environment variable reference pattern: ${VAR_NAME} or ${VAR_NAME:default}
    if s.starts_with("${") && s.ends_with('}') {
        let inner = &s[2..s.len() - 1]; // Remove ${ and }

        // Check for default value pattern: ${VAR_NAME:default}
        if let Some(colon_pos) = inner.find(':') {
            Secret::Env {
                env: inner[..colon_pos].to_string(),
                default: Some(inner[colon_pos + 1..].to_string()),
            }
        } else {
            // No default: ${VAR_NAME}
            Secret::Env {
                env: inner.to_string(),
                default: None,
            }
        }
    } else {
        // Plain literal value
        Secret::Literal(s.to_string())
    }
}

struct SecretVisitor;

impl<'de> serde::de::Visitor<'de> for SecretVisitor {
    type Value = Secret;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str(
            "a plain string or a tagged secret object ({\"Literal\": ...} / {\"Env\": {...}})",
        )
    }

    fn visit_str<E>(self, s: &str) -> Result<Secret, E> {
        Ok(secret_from_env_pattern(s))
    }

    fn visit_map<A>(self, mut map: A) -> Result<Secret, A::Error>
    where
        A: serde::de::MapAccess<'de>,
    {
        let mut secret: Option<Secret> = None;
        while let Some(key) = map.next_key::<String>()? {
            match key.as_str() {
                "Literal" => {
                    let value: String = map.next_value()?;
                    secret = Some(Secret::Literal(value));
                }
                "Env" => {
                    let wire: EnvSecretWire = map.next_value()?;
                    secret = Some(Secret::Env {
                        env: wire.env,
                        default: wire.default,
                    });
                }
                _ => {
                    // Ignore unknown keys but consume their value.
                    let _: serde::de::IgnoredAny = map.next_value()?;
                }
            }
        }
        secret.ok_or_else(|| serde::de::Error::custom("missing \"Literal\" or \"Env\" key"))
    }
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
            Secret::Env { env, default } => std::env::var(env).or_else(|_| {
                default
                    .clone()
                    .ok_or_else(|| LLMError::Auth(format!("Environment variable '{env}' not set")))
            }),
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

    #[test]
    fn test_deserialize_env_var_reference() {
        // Test ${VAR_NAME} pattern - need to parse as part of a TOML struct
        #[derive(Deserialize)]
        struct Wrapper {
            key: Secret,
        }

        let wrapper: Wrapper = toml::from_str(r#"key = "${TEST_VAR}""#).unwrap();
        match wrapper.key {
            Secret::Env { env, default } => {
                assert_eq!(env, "TEST_VAR");
                assert_eq!(default, None);
            }
            _ => panic!("Expected Secret::Env"),
        }
    }

    #[test]
    fn test_deserialize_env_var_with_default() {
        // Test ${VAR_NAME:default} pattern
        #[derive(Deserialize)]
        struct Wrapper {
            key: Secret,
        }

        let wrapper: Wrapper = toml::from_str(r#"key = "${TEST_VAR:default_value}""#).unwrap();
        match wrapper.key {
            Secret::Env { env, default } => {
                assert_eq!(env, "TEST_VAR");
                assert_eq!(default, Some("default_value".to_string()));
            }
            _ => panic!("Expected Secret::Env"),
        }
    }

    #[test]
    fn test_deserialize_literal_value() {
        // Test plain literal value
        #[derive(Deserialize)]
        struct Wrapper {
            key: Secret,
        }

        let wrapper: Wrapper = toml::from_str(r#"key = "my-literal-key""#).unwrap();
        match wrapper.key {
            Secret::Literal(val) => {
                assert_eq!(val, "my-literal-key");
            }
            _ => panic!("Expected Secret::Literal"),
        }
    }

    #[test]
    fn test_deserialize_env_and_resolve() {
        std::env::set_var("DESER_TEST", "resolved-value");

        #[derive(Deserialize)]
        struct Wrapper {
            key: Secret,
        }

        let wrapper: Wrapper = toml::from_str(r#"key = "${DESER_TEST}""#).unwrap();
        assert_eq!(wrapper.key.resolve().unwrap(), "resolved-value");
    }

    #[test]
    fn test_json_roundtrip_literal() {
        let secret = Secret::literal("sk-test");
        let json = serde_json::to_string(&secret).unwrap();
        assert_eq!(json, r#"{"Literal":"sk-test"}"#);
        let parsed: Secret = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, secret);
    }

    #[test]
    fn test_json_roundtrip_env() {
        let secret = Secret::env("API_KEY");
        let json = serde_json::to_string(&secret).unwrap();
        let parsed: Secret = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, secret);
    }

    #[test]
    fn test_json_roundtrip_env_with_default() {
        let secret = Secret::env_with_default("API_KEY", "fallback");
        let json = serde_json::to_string(&secret).unwrap();
        let parsed: Secret = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, secret);
    }

    #[test]
    fn test_json_plain_string_deserialization_still_works() {
        // Back-compat: a plain string (including the ${VAR} pattern) must
        // still deserialize into the corresponding secret kind.
        let literal: Secret = serde_json::from_str(r#""sk-test""#).unwrap();
        assert_eq!(literal, Secret::literal("sk-test"));
        let env: Secret = serde_json::from_str(r#""${API_KEY}""#).unwrap();
        assert_eq!(env, Secret::env("API_KEY"));
        let env_default: Secret = serde_json::from_str(r#""${API_KEY:fallback}""#).unwrap();
        assert_eq!(env_default, Secret::env_with_default("API_KEY", "fallback"));
    }

    #[test]
    fn test_toml_tagged_form_deserializes() {
        // The JSON round-trip shape must also work from TOML inline tables.
        #[derive(Deserialize)]
        struct Wrapper {
            key: Secret,
        }
        let wrapper: Wrapper = toml::from_str(r#"key = { Literal = "sk-test" }"#).unwrap();
        assert_eq!(wrapper.key, Secret::literal("sk-test"));
    }
}
