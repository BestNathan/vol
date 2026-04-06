//! Multi-provider registry for managing multiple LLM configurations.

use std::collections::HashMap;
use std::sync::Arc;
use vol_llm_core::{LLMClient, LLMError};
use crate::config::LLMConfig;
use crate::factory::create_provider;

/// Named LLM provider configuration
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct LLMProviderConfig {
    /// Unique identifier for this provider
    pub id: String,
    /// Provider configuration
    #[serde(flatten)]
    pub config: LLMConfig,
}

/// Registry for managing multiple LLM providers
pub struct LLMProviderRegistry {
    providers: HashMap<String, Arc<dyn LLMClient>>,
}

impl LLMProviderRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }

    /// Create registry from a list of provider configs
    pub fn from_configs(configs: &[LLMProviderConfig]) -> Result<Self, LLMError> {
        let mut registry = Self::new();
        for config in configs {
            let provider = create_provider(&config.config)?;
            registry.providers.insert(config.id.clone(), Arc::from(provider));
        }
        Ok(registry)
    }

    /// Get a provider by ID
    pub fn get(&self, id: &str) -> Option<Arc<dyn LLMClient>> {
        self.providers.get(id).cloned()
    }

    /// Get all registered provider IDs
    pub fn ids(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider exists
    pub fn contains(&self, id: &str) -> bool {
        self.providers.contains_key(id)
    }

    /// Get the number of registered providers
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl Default for LLMProviderRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::LLMProvider;

    #[test]
    fn test_registry_from_configs() {
        std::env::set_var("TEST_API_KEY", "test-key");

        let configs = vec![
            LLMProviderConfig {
                id: "test-provider".to_string(),
                config: LLMConfig::with_env_key(
                    LLMProvider::Anthropic,
                    "claude-test",
                    "TEST_API_KEY",
                    "https://api.test.com",
                ),
            },
        ];

        let registry = LLMProviderRegistry::from_configs(&configs).unwrap();
        assert!(registry.contains("test-provider"));
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_registry_get() {
        std::env::set_var("TEST_API_KEY_2", "test-key-2");

        let configs = vec![
            LLMProviderConfig {
                id: "provider-a".to_string(),
                config: LLMConfig::with_env_key(
                    LLMProvider::Anthropic,
                    "claude-test",
                    "TEST_API_KEY_2",
                    "https://api.test.com",
                ),
            },
        ];

        let registry = LLMProviderRegistry::from_configs(&configs).unwrap();
        assert!(registry.get("provider-a").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_registry_ids() {
        let registry = LLMProviderRegistry::new();
        assert!(registry.ids().is_empty());
    }
}
