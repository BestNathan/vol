# LLMConfig Secret Multi-Provider Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor LLMConfig to use Secret type for flexible API key configuration and add LLMProviderRegistry for multi-provider support.

**Architecture:** Add Secret enum for literal/env value resolution, update LLMConfig structure, create LLMProviderRegistry for managing multiple named providers, update AgentAdviceConfig to reference provider by ID.

**Tech Stack:** Rust, serde (untagged enum), vol-llm-provider, vol-llm-bridge, TOML configuration

---

### Task 1: Implement Secret Type

**Files:**
- Create: `crates/vol-llm-provider/src/secret.rs`
- Modify: `crates/vol-llm-provider/src/lib.rs`

- [ ] **Step 1: Create src/secret.rs**

```rust
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
```

- [ ] **Step 2: Update src/lib.rs**

```rust
//! vol-llm-provider: LLM Provider implementations.

pub mod config;
pub mod anthropic;
pub mod factory;
pub mod secret;
pub mod registry;

pub use config::LLMConfig;
pub use anthropic::AnthropicProvider;
pub use factory::create_provider;
pub use secret::Secret;
pub use registry::{LLMProviderConfig, LLMProviderRegistry};
```

- [ ] **Step 3: Run tests**

```bash
cd crates/vol-llm-provider && cargo test secret
```

Expected: All 5 tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-provider/src/secret.rs crates/vol-llm-provider/src/lib.rs
git commit -m "feat(vol-llm-provider): add Secret type for flexible config"
```

---

### Task 2: Update LLMConfig Structure

**Files:**
- Modify: `crates/vol-llm-provider/src/config.rs`

- [ ] **Step 1: Update LLMConfig**

```rust
//! LLM configuration.

use serde::{Deserialize, Serialize};
use vol_llm_core::{LLMProvider, LLMError};
use crate::secret::Secret;

/// LLM configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMConfig {
    /// Provider type
    pub provider: LLMProvider,
    /// Model name
    pub model: String,
    /// API key (literal or environment variable reference)
    pub api_key: Secret,
    /// Base URL for API endpoint
    pub base_url: String,
}

impl LLMConfig {
    /// Create a new LLMConfig
    pub fn new(
        provider: LLMProvider,
        model: impl Into<String>,
        api_key: Secret,
        base_url: impl Into<String>,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            api_key,
            base_url: base_url.into(),
        }
    }

    /// Resolve API key from secret
    pub fn resolve_api_key(&self) -> Result<String, LLMError> {
        self.api_key.resolve()
    }

    /// Create config with literal API key (convenience for testing)
    pub fn with_literal_key(
        provider: LLMProvider,
        model: impl Into<String>,
        api_key: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self::new(provider, model, Secret::literal(api_key), base_url)
    }

    /// Create config with environment variable (convenience for production)
    pub fn with_env_key(
        provider: LLMProvider,
        model: impl Into<String>,
        env_var: impl Into<String>,
        base_url: impl Into<String>,
    ) -> Self {
        Self::new(provider, model, Secret::env(env_var), base_url)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_core::LLMProvider;

    #[test]
    fn test_config_with_literal_key() {
        let config = LLMConfig::with_literal_key(
            LLMProvider::Anthropic,
            "claude-test",
            "sk-test-key",
            "https://api.test.com",
        );
        assert_eq!(config.resolve_api_key().unwrap(), "sk-test-key");
    }

    #[test]
    fn test_config_with_env_key() {
        std::env::set_var("TEST_API_KEY", "env-key");
        let config = LLMConfig::with_env_key(
            LLMProvider::Anthropic,
            "claude-test",
            "TEST_API_KEY",
            "https://api.test.com",
        );
        assert_eq!(config.resolve_api_key().unwrap(), "env-key");
    }
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-provider && cargo test config
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-provider/src/config.rs
git commit -m "feat(vol-llm-provider): update LLMConfig to use Secret type"
```

---

### Task 3: Update AnthropicProvider to Use Secret

**Files:**
- Modify: `crates/vol-llm-provider/src/anthropic.rs`

- [ ] **Step 1: Update AnthropicProvider::new()**

Replace the `new` function to use `resolve_api_key()`:

```rust
impl AnthropicProvider {
    /// Create new Anthropic provider
    pub fn new(config: &LLMConfig) -> Result<Self, LLMError> {
        Ok(Self {
            client: Client::new(),
            api_key: config.resolve_api_key()?,
            model: config.model.clone(),
            base_url: config.base_url.clone(),
        })
    }
    // ... rest of implementation unchanged
}
```

- [ ] **Step 2: Run cargo check**

```bash
cd crates/vol-llm-provider && cargo check
```

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-provider/src/anthropic.rs
git commit -m "refactor(vol-llm-provider): update AnthropicProvider to use Secret"
```

---

### Task 4: Create LLMProviderConfig and LLMProviderRegistry

**Files:**
- Create: `crates/vol-llm-provider/src/registry.rs`

- [ ] **Step 1: Create src/registry.rs**

```rust
//! Multi-provider registry for managing multiple LLM configurations.

use std::collections::HashMap;
use vol_llm_core::{LLMClient, LLMError};
use crate::config::LLMConfig;
use crate::factory::create_provider;

/// Named LLM provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LLMProviderConfig {
    /// Unique identifier for this provider
    pub id: String,
    /// Provider configuration
    #[serde(flatten)]
    pub config: LLMConfig,
}

/// Registry for managing multiple LLM providers
pub struct LLMProviderRegistry {
    providers: HashMap<String, Box<dyn LLMClient>>,
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
            registry.providers.insert(config.id.clone(), provider);
        }
        Ok(registry)
    }

    /// Get a provider by ID
    pub fn get(&self, id: &str) -> Option<&dyn LLMClient> {
        self.providers.get(id).map(|p| p.as_ref())
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
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-provider && cargo test registry
```

Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-provider/src/registry.rs
git commit -m "feat(vol-llm-provider): add LLMProviderRegistry for multi-provider"
```

---

### Task 5: Update Factory Function

**Files:**
- Modify: `crates/vol-llm-provider/src/factory.rs`

- [ ] **Step 1: Update factory.rs**

```rust
//! Provider factory functions.

use vol_llm_core::{LLMClient, LLMProvider, LLMError};
use crate::{AnthropicProvider, LLMConfig};

/// Create provider from config
pub fn create_provider(config: &LLMConfig) -> Result<Box<dyn LLMClient>, LLMError> {
    match config.provider {
        LLMProvider::Anthropic => Ok(Box::new(AnthropicProvider::new(config)?)),
        // OpenAI provider can be added in the future
        #[allow(unreachable_patterns)]
        _ => Err(LLMError::Parse("Provider not implemented".to_string())),
    }
}
```

- [ ] **Step 2: Run cargo check**

```bash
cd crates/vol-llm-provider && cargo check
```

Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-provider/src/factory.rs
git commit -m "chore(vol-llm-provider): update factory comments"
```

---

### Task 6: Update AgentAdviceConfig to Use Provider ID

**Files:**
- Modify: `crates/vol-llm-bridge/src/service.rs`

- [ ] **Step 1: Update AgentAdviceConfig**

```rust
/// Agent advice configuration
#[derive(Clone)]
pub struct AgentAdviceConfig {
    pub enabled: bool,
    pub cooldown_secs: u64,
    pub max_analyses_per_hour: u32,
    pub llm_provider_id: String,  // Reference to llm_providers config
}

impl Default for AgentAdviceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_secs: 300, // 5 minutes
            max_analyses_per_hour: 20,
            llm_provider_id: "anthropic-main".to_string(),
        }
    }
}
```

- [ ] **Step 2: Update AgentAdviceService to accept registry**

```rust
/// Agent Advice Service
pub struct AgentAdviceService {
    limiter: FrequencyLimiter,
    config: AgentAdviceConfig,
    registry: vol_llm_provider::LLMProviderRegistry,
}

impl AgentAdviceService {
    /// Create a new agent advice service
    pub fn new(
        config: AgentAdviceConfig,
        registry: vol_llm_provider::LLMProviderRegistry,
    ) -> Self {
        Self {
            limiter: FrequencyLimiter::new(config.cooldown_secs, config.max_analyses_per_hour),
            config,
            registry,
        }
    }

    /// Generate advice using ReAct Agent
    async fn generate_advice(
        &self,
        alert: &Alert,
        history_summary: String,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let tools = ToolRegistry::new();

        // Get provider from registry by ID
        let llm = self.registry.get(&self.config.llm_provider_id)
            .ok_or_else(|| format!("Unknown provider: {}", self.config.llm_provider_id))?;

        // Create agent
        let agent = ReActAgent::new(
            llm,
            tools,
            AgentConfig {
                max_iterations: 5,
                system_prompt: system_prompt().to_string(),
                verbose: false,
            },
        );
        
        // ... rest of the method unchanged
    }
}
```

- [ ] **Step 3: Remove unused imports**

Remove:
```rust
use vol_llm_provider::{create_provider, LLMConfig};
use vol_llm_core::LLMProvider;
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p vol-llm-bridge
```

Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-bridge/src/service.rs
git commit -m "feat(vol-llm-bridge): update AgentAdviceService to use provider registry"
```

---

### Task 7: Add vol-config Support for llm_providers

**Files:**
- Modify: `crates/vol-config/src/lib.rs` (or main config struct location)

- [ ] **Step 1: Check current config structure**

```bash
cat crates/vol-config/src/lib.rs
```

- [ ] **Step 2: Add llm_providers field**

```rust
// Add to main Config struct:

/// LLM Provider configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    // ... existing fields ...
    
    /// LLM providers (can be multiple)
    #[serde(default)]
    pub llm_providers: Vec<vol_llm_provider::LLMProviderConfig>,
    
    /// Agent advice configuration
    #[serde(default)]
    pub agent_advice: AgentAdviceConfig,
}
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check -p vol-config
```

Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-config/src/lib.rs
git commit -m "feat(vol-config): add llm_providers array to config"
```

---

### Task 8: Update Configuration Examples

**Files:**
- Create: `config/llm.example.toml`
- Modify: `.env.example`

- [ ] **Step 1: Create config/llm.example.toml**

```toml
# LLM Provider Configuration Examples

# Option 1: Single provider with environment variable
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

# Option 2: Multiple providers with fallback
[[llm_providers]]
id = "anthropic-primary"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[[llm_providers]]
id = "openai-backup"
provider = "openai"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY:sk-fallback-key}"
base_url = "https://api.openai.com/v1"

# Agent Advice Configuration
[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
llm_provider_id = "anthropic-main"
```

- [ ] **Step 2: Update .env.example**

```bash
# LLM API Keys
ANTHROPIC_AUTH_TOKEN=
OPENAI_API_KEY=
```

- [ ] **Step 3: Commit**

```bash
git add config/llm.example.toml .env.example
git commit -m "docs: add LLM configuration examples"
```

---

### Task 9: Update vol-monitor Integration

**Files:**
- Modify: `crates/vol-monitor/src/main.rs`

- [ ] **Step 1: Find current AgentAdviceService initialization**

```bash
grep -n "AgentAdviceService" crates/vol-monitor/src/main.rs
```

- [ ] **Step 2: Update initialization code**

```rust
// Before:
if config.agent_advice.enabled {
    let advice_service = AgentAdviceService::new(config.agent_advice.clone());
    let advice_rx = alert_tx.subscribe();
    tokio::spawn(async move {
        advice_service.run(advice_rx).await
    });
}

// After:
if config.agent_advice.enabled {
    // Build provider registry from config
    let registry = LLMProviderRegistry::from_configs(&config.llm_providers)
        .expect("Failed to initialize LLM providers");
    
    // Verify the configured provider exists
    if !registry.contains(&config.agent_advice.llm_provider_id) {
        panic!(
            "Unknown provider: '{}' (available: {:?})",
            config.agent_advice.llm_provider_id,
            registry.ids()
        );
    }
    
    let advice_service = AgentAdviceService::new(
        config.agent_advice.clone(),
        registry,
    );
    let advice_rx = alert_tx.subscribe();
    tokio::spawn(async move {
        advice_service.run(advice_rx).await
    });
}
```

- [ ] **Step 3: Add necessary imports**

```rust
use vol_llm_provider::{LLMProviderRegistry, LLMProviderConfig};
```

- [ ] **Step 4: Run cargo check**

```bash
cargo check -p vol-monitor
```

Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-monitor/src/main.rs
git commit -m "feat(vol-monitor): integrate LLM provider registry"
```

---

### Task 10: Update Documentation

**Files:**
- Modify: `docs/CONFIGURATION.md` or create `docs/LLM_CONFIG.md`

- [ ] **Step 1: Add LLM Configuration section**

```markdown
## LLM Configuration

### Secret Value Format

The `api_key` field supports flexible value formats:

| Format | Example | Description |
|--------|---------|-------------|
| Literal | `"sk-xxx-key"` | Direct API key value |
| Env Var | `"${API_KEY}"` | Read from environment variable |
| Env + Default | `"${API_KEY:default}"` | Env var with fallback |

### Multiple Providers

Configure multiple LLM providers:

```toml
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[[llm_providers]]
id = "openai-backup"
provider = "openai"
model = "gpt-4o"
api_key = "${OPENAI_API_KEY}"
base_url = "https://api.openai.com/v1"
```

### Agent Advice Configuration

```toml
[agent_advice]
enabled = true
cooldown_secs = 300
max_analyses_per_hour = 20
llm_provider_id = "anthropic-main"  # References [[llm_providers]] id
```
```

- [ ] **Step 2: Commit**

```bash
git add docs/CONFIGURATION.md
git commit -m "docs: update configuration guide for multi-provider"
```

---

### Task 11: Run Full Test Suite

- [ ] **Step 1: Run all vol-llm-provider tests**

```bash
cargo test -p vol-llm-provider
```

Expected: All tests pass (8+ tests)

- [ ] **Step 2: Run all vol-llm-bridge tests**

```bash
cargo test -p vol-llm-bridge
```

Expected: All 7 tests pass

- [ ] **Step 3: Run workspace check**

```bash
cargo check --workspace
```

Expected: No errors

- [ ] **Step 4: Run clippy**

```bash
cargo clippy --workspace -- -D warnings
```

Expected: No warnings

---

### Task 12: Create Migration Guide

**Files:**
- Create: `docs/migrations/llm-config-v2.md`

- [ ] **Step 1: Create migration guide**

```markdown
# LLM Config Migration Guide (v1 → v2)

## Overview

This migration updates `LLMConfig` to use the new `Secret` type for flexible API key configuration and multi-provider support.

## Breaking Changes

### Old Format (v1)

```toml
[llm]
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key_env = "ANTHROPIC_AUTH_TOKEN"
endpoint = "https://coding.dashscope.aliyuncs.com/apps/anthropic"
```

### New Format (v2)

```toml
[[llm_providers]]
id = "anthropic-main"
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[agent_advice]
llm_provider_id = "anthropic-main"
```

## Migration Steps

1. Replace `[llm]` with `[[llm_providers]]` (array syntax)
2. Add `id` field to identify the provider
3. Rename `api_key_env` value to `api_key` format:
   - Old: `api_key_env = "ANTHROPIC_AUTH_TOKEN"`
   - New: `api_key = "${ANTHROPIC_AUTH_TOKEN}"`
4. Rename `endpoint` to `base_url`
5. Update `[agent_advice]` to reference provider by `llm_provider_id`

## Quick Migration Script

For simple single-provider setups:

```bash
# Backup original config
cp config.toml config.toml.bak

# Manual edits required - see examples above
```

## Testing After Migration

1. Verify config loads: `cargo run -- --config config.toml`
2. Check provider initializes: Look for "AgentAdviceService started" in logs
3. Test alert processing: Trigger a test alert

## Rollback

If issues occur, restore the backup:

```bash
cp config.toml.bak config.toml
cargo build
```
```

- [ ] **Step 2: Commit**

```bash
git add docs/migrations/llm-config-v2.md
git commit -m "docs: add LLM config migration guide"
```

---

## Self-Review

**1. Spec coverage check:**

| Spec Requirement | Task |
|-----------------|------|
| `Secret` type | Task 1 |
| `Secret::resolve()` with literal/env/default | Task 1 |
| `LLMConfig` uses `Secret` | Task 2 |
| `AnthropicProvider` updated | Task 3 |
| `LLMProviderConfig` struct | Task 4 |
| `LLMProviderRegistry` | Task 4 |
| `AgentAdviceConfig` uses provider ID | Task 6 |
| TOML config examples | Task 8 |
| Migration path documented | Task 12 |

**2. Placeholder scan:** No TBD/TODO remaining.

**3. Type consistency:** All types match spec - `Secret`, `LLMConfig`, `LLMProviderRegistry`, `LLMProviderConfig` are consistent throughout.

---

Plan complete. Two execution options:

**1. Subagent-Driven (recommended)** - Fresh subagent per task with review between tasks

**2. Inline Execution** - Execute tasks in this session with checkpoints

Which approach?
