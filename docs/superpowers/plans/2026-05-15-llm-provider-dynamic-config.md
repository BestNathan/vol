# LLM Provider Dynamic Configuration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace config.toml's `[[llm_providers]]` with file-based `.agents/providers/*.toml` configuration, supporting per-provider body defaults and custom headers.

**Architecture:** New `ProviderLoader` scans `.agents/providers/*.toml` files (project + user level, project overrides). Each file maps to a `ProviderFileConfig` with `body` (HashMap) and `headers` (HashMap). Body defaults merge with ConversationRequest at runtime. Headers attach to HTTP requests.

**Tech Stack:** Rust, serde (TOML), vol-llm-provider, vol-llm-core, vol-config

---

## File Map

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-provider/src/config.rs` | Modify | Add `ProviderFileConfig` struct, add `body`/`headers` to `LLMConfig` |
| `crates/vol-llm-provider/src/loader.rs` | Create | Dual-layer TOML file loader with merge |
| `crates/vol-llm-provider/src/anthropic.rs` | Modify | Use body defaults + headers from config |
| `crates/vol-llm-provider/src/registry.rs` | Modify | Add `from_loader()` factory |
| `crates/vol-llm-provider/src/lib.rs` | Modify | Export new types |
| `crates/vol-llm-provider/src/factory.rs` | Modify | Accept `ProviderFileConfig` |
| `crates/vol-config/src/lib.rs` | Modify | Remove `llm_providers` field, update `AgentAdviceConfig` |
| `crates/vol-monitor/src/main.rs` | Modify | Use `ProviderLoader` instead of `config.llm_providers` |
| `.agents/providers/anthropic-dashscope.toml` | Create | Example provider config |
| `config/llm.example.toml` | Delete | Replaced by `.agents/providers/` |
| `config.agent-test.toml` | Modify | Remove `[[llm_providers]]` section |
| `config.feishu-test.toml` | Modify | Remove `[[llm_providers]]` section |
| All test/example files using `LLMProviderConfig` | Modify | Update to new API |

---

### Task 1: Add `ProviderFileConfig` and extend `LLMConfig`

**Files:**
- Modify: `crates/vol-llm-provider/src/config.rs`
- Modify: `crates/vol-llm-provider/src/lib.rs`

- [ ] **Step 1: Add `body` and `headers` to `LLMConfig`**

In `crates/vol-llm-provider/src/config.rs`, add two new optional fields to `LLMConfig`:

```rust
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
    /// Default body parameters (provider-specific), merged at runtime
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<HashMap<String, serde_json::Value>>,
    /// Custom HTTP headers, attached to every request
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HashMap<String, String>>,
}
```

Add the import at the top of config.rs:

```rust
use std::collections::HashMap;
```

- [ ] **Step 2: Update `LLMConfig::new` constructor**

Update the constructor to accept body and headers:

```rust
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
            body: None,
            headers: None,
        }
    }

    /// Set default body parameters
    pub fn with_body(mut self, body: HashMap<String, serde_json::Value>) -> Self {
        self.body = Some(body);
        self
    }

    /// Set custom headers
    pub fn with_headers(mut self, headers: HashMap<String, String>) -> Self {
        self.headers = Some(headers);
        self
    }
```

- [ ] **Step 3: Add `ProviderFileConfig` struct**

Append to `crates/vol-llm-provider/src/config.rs`:

```rust
/// File-level provider configuration, parsed from a single TOML file.
/// Filename (without .toml extension) is the provider ID.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ProviderFileConfig {
    pub provider: LLMProvider,
    pub model: String,
    pub api_key: Secret,
    pub base_url: String,
    #[serde(default)]
    pub body: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub headers: Option<HashMap<String, String>>,
}

impl ProviderFileConfig {
    /// Convert to LLMConfig (for backward compatibility with existing factory)
    pub fn to_llm_config(&self) -> LLMConfig {
        LLMConfig {
            provider: self.provider,
            model: self.model.clone(),
            api_key: self.api_key.clone(),
            base_url: self.base_url.clone(),
            body: self.body.clone(),
            headers: self.headers.clone(),
        }
    }
}
```

- [ ] **Step 4: Export `ProviderFileConfig` from lib.rs**

In `crates/vol-llm-provider/src/lib.rs`, add the export:

```rust
pub use config::{LLMConfig, ProviderFileConfig};
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-provider/src/config.rs crates/vol-llm-provider/src/lib.rs
git commit -m "feat: add ProviderFileConfig and body/headers to LLMConfig"
```

---

### Task 2: Create `ProviderLoader` for dual-layer TOML loading

**Files:**
- Create: `crates/vol-llm-provider/src/loader.rs`
- Modify: `crates/vol-llm-provider/src/lib.rs`

- [ ] **Step 1: Write the loader module**

Create `crates/vol-llm-provider/src/loader.rs`:

```rust
//! Provider configuration loader.
//!
//! Scans `.agents/providers/*.toml` from project and user directories.
//! Project-level configs override user-level configs per-key (by filename).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

use crate::{LLMConfig, ProviderFileConfig, Secret};

const PROVIDERS_DIR: &str = ".agents/providers";

/// Provider configuration with resolved ID.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NamedProviderConfig {
    pub id: String,
    #[serde(flatten)]
    pub config: ProviderFileConfig,
}

/// Loaded provider registry.
#[derive(Debug, Clone)]
pub struct ProviderLoader {
    providers: HashMap<String, ProviderFileConfig>,
}

impl ProviderLoader {
    /// Load configuration from project-level and user-level sources.
    ///
    /// Priority: `.agents/providers/` (project root) > `~/.agents/providers/` (user home).
    /// Per-key merge: if both files define the same provider ID, the project-level wins.
    pub fn load(working_dir: Option<&Path>) -> Self {
        let project_map = load_dir(working_dir);
        let user_map = load_user_dir();

        // Merge: user first (lower priority), then project (higher priority)
        let mut providers = user_map;
        providers.extend(project_map);

        Self { providers }
    }

    /// Get a provider by ID
    pub fn get(&self, id: &str) -> Option<&ProviderFileConfig> {
        self.providers.get(id)
    }

    /// Get all provider IDs
    pub fn ids(&self) -> Vec<&str> {
        self.providers.keys().map(|s| s.as_str()).collect()
    }

    /// Check if a provider exists
    pub fn contains(&self, id: &str) -> bool {
        self.providers.contains_key(id)
    }

    /// Number of loaded providers
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }

    /// Convert to legacy LLMProviderConfig list (for migration compatibility)
    pub fn to_provider_configs(&self) -> Vec<NamedProviderConfig> {
        self.providers
            .iter()
            .map(|(id, config)| NamedProviderConfig {
                id: id.clone(),
                config: config.clone(),
            })
            .collect()
    }
}

impl Default for ProviderLoader {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
        }
    }
}

/// Load all TOML files from a directory, keyed by filename (without extension).
fn load_dir(dir: Option<&Path>) -> HashMap<String, ProviderFileConfig> {
    let mut map = HashMap::new();
    let Some(dir) = dir else { return map };

    let providers_dir = dir.join(PROVIDERS_DIR);
    if !providers_dir.is_dir() {
        return map;
    }

    if let Ok(entries) = std::fs::read_dir(&providers_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "toml") {
                let id = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.to_string());
                let Some(id) = id else { continue };

                match std::fs::read_to_string(&path) {
                    Ok(content) => match toml::from_str::<ProviderFileConfig>(&content) {
                        Ok(config) => {
                            map.insert(id, config);
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse provider config '{}': {}", path.display(), e);
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Failed to read provider config '{}': {}", path.display(), e);
                    }
                }
            }
        }
    }

    map
}

/// Load user-level provider configs from ~/.agents/providers/
fn load_user_dir() -> HashMap<String, ProviderFileConfig> {
    let home = dirs::home_dir();
    load_dir(home.as_deref())
}
```

- [ ] **Step 2: Add `dirs` dependency to Cargo.toml**

In `crates/vol-llm-provider/Cargo.toml`, add:

```toml
dirs = "5"
```

- [ ] **Step 3: Export `ProviderLoader` and `NamedProviderConfig` from lib.rs**

In `crates/vol-llm-provider/src/lib.rs`, add:

```rust
pub mod loader;
pub use loader::{NamedProviderConfig, ProviderLoader};
```

- [ ] **Step 4: Write tests for ProviderLoader**

Append to `crates/vol-llm-provider/src/loader.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_test_file(dir: &Path, name: &str, content: &str) {
        std::fs::create_dir_all(dir.join(PROVIDERS_DIR)).unwrap();
        let mut file = std::fs::File::create(dir.join(PROVIDERS_DIR).join(name)).unwrap();
        file.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_single_provider() {
        let dir = tempfile::tempdir().unwrap();
        write_test_file(
            dir.path(),
            "anthropic-test.toml",
            r#"
provider = "anthropic"
model = "claude-test"
api_key = "${TEST_KEY}"
base_url = "https://api.test.com"
"#,
        );

        let loader = ProviderLoader::load(Some(dir.path()));
        assert_eq!(loader.len(), 1);
        assert!(loader.contains("anthropic-test"));
        let config = loader.get("anthropic-test").unwrap();
        assert_eq!(config.model, "claude-test");
    }

    #[test]
    fn test_load_with_body_and_headers() {
        let dir = tempfile::tempdir().unwrap();
        write_test_file(
            dir.path(),
            "anthropic-full.toml",
            r#"
provider = "anthropic"
model = "claude-test"
api_key = "sk-test"
base_url = "https://api.test.com"

[body]
max_tokens = 4096
temperature = 0.5

[headers]
"anthropic-version" = "2023-06-01"
"#,
        );

        let loader = ProviderLoader::load(Some(dir.path()));
        let config = loader.get("anthropic-full").unwrap();
        assert!(config.body.is_some());
        let body = config.body.as_ref().unwrap();
        assert_eq!(body["max_tokens"], 4096);
        assert!(config.headers.is_some());
        let headers = config.headers.as_ref().unwrap();
        assert_eq!(headers["anthropic-version"], "2023-06-01");
    }

    #[test]
    fn test_project_overrides_user() {
        let user_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();

        // User config
        write_test_file(
            user_dir.path(),
            "anthropic-test.toml",
            r#"
provider = "anthropic"
model = "claude-user"
api_key = "sk-user"
base_url = "https://user.api.com"
"#,
        );

        // Project config (overrides user)
        write_test_file(
            project_dir.path(),
            "anthropic-test.toml",
            r#"
provider = "anthropic"
model = "claude-project"
api_key = "sk-project"
base_url = "https://project.api.com"
"#,
        );

        // Set HOME so user dir is found
        std::env::set_var("HOME", user_dir.path());

        let loader = ProviderLoader::load(Some(project_dir.path()));
        assert_eq!(loader.len(), 1);
        let config = loader.get("anthropic-test").unwrap();
        // Project-level should win
        assert_eq!(config.model, "claude-project");

        std::env::remove_var("HOME");
    }

    #[test]
    fn test_load_empty_dir() {
        let dir = tempfile::tempdir().unwrap();
        let loader = ProviderLoader::load(Some(dir.path()));
        assert!(loader.is_empty());
    }

    #[test]
    fn test_load_nonexistent_dir() {
        let loader = ProviderLoader::load(None);
        assert!(loader.is_empty());
    }
}
```

- [ ] **Step 5: Add `tempfile` as dev dependency**

In `crates/vol-llm-provider/Cargo.toml`, add:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-provider/src/loader.rs crates/vol-llm-provider/Cargo.toml crates/vol-llm-provider/src/lib.rs
git commit -m "feat: add ProviderLoader with dual-layer TOML loading"
```

---

### Task 3: Update `AnthropicProvider` to use body defaults and headers

**Files:**
- Modify: `crates/vol-llm-provider/src/anthropic.rs`
- Modify: `crates/vol-llm-provider/src/factory.rs`

- [ ] **Step 1: Update `AnthropicProvider` struct and constructor**

In `crates/vol-llm-provider/src/anthropic.rs`, update the struct:

```rust
/// Anthropic Provider
pub struct AnthropicProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    body_defaults: HashMap<String, serde_json::Value>,
    headers: HashMap<String, String>,
}
```

Add `use std::collections::HashMap;` at the top.

Update constructor:

```rust
impl AnthropicProvider {
    /// Create new Anthropic provider
    pub fn new(config: &LLMConfig) -> Result<Self> {
        let client = Self::build_client()?;
        Ok(Self {
            client,
            api_key: config.resolve_api_key()?,
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            body_defaults: config.body.clone().unwrap_or_default(),
            headers: config.headers.clone().unwrap_or_default(),
        })
    }
```

- [ ] **Step 2: Update `converse()` to merge body defaults and attach headers**

In `converse()`, change the body building section. Replace the current body construction:

```rust
    async fn converse(&self, request: ConversationRequest) -> Result<ConversationResponse> {
        // max_tokens is required for Anthropic
        let max_tokens = request.model_config.max_tokens
            .or_else(|| self.body_defaults.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as u32))
            .unwrap_or(8192);

        // Convert messages
        let anthropic_messages = self.convert_messages(&request.messages)?;

        // Build request body with defaults as base
        let mut body = json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": anthropic_messages,
        });

        // Apply body defaults (excluding max_tokens which is already set)
        for (key, value) in &self.body_defaults {
            if key == "max_tokens" {
                continue; // Already handled above
            }
            // Check if request overrides this key
            let overridden = match key.as_str() {
                "temperature" => request.model_config.temperature.is_some(),
                "top_p" => request.model_config.top_p.is_some(),
                "top_k" => request.model_config.top_k.is_some(),
                _ => false,
            };
            if !overridden {
                body[key] = value.clone();
            }
        }
```

- [ ] **Step 3: Attach custom headers in the HTTP request**

In `converse()`, update the HTTP request builder to include custom headers. Replace the request sending section:

```rust
        // Send request
        let url = format!("{}/v1/messages", self.base_url);

        let mut req = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("User-Agent", "claude-code/1.0.0")
            .json(&body);

        // Attach custom headers
        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req
            .send()
            .await
            .map_err(LLMError::Network)?;
```

- [ ] **Step 4: Update `converse_stream()` similarly**

Apply the same changes to `converse_stream()`: body defaults merge and header attachment.

Replace the body building section:

```rust
    async fn converse_stream(&self, request: ConversationRequest) -> Result<StreamReceiver> {
        // max_tokens is required for Anthropic
        let max_tokens = request.model_config.max_tokens
            .or_else(|| self.body_defaults.get("max_tokens").and_then(|v| v.as_u64()).map(|v| v as u32))
            .unwrap_or(8192);

        // Convert messages
        let anthropic_messages = self.convert_messages(&request.messages)?;

        // Build request body with defaults as base
        let mut body = json!({
            "model": self.model,
            "max_tokens": max_tokens,
            "messages": anthropic_messages,
            "stream": true,
        });

        // Apply body defaults (excluding max_tokens which is already set)
        for (key, value) in &self.body_defaults {
            if key == "max_tokens" {
                continue;
            }
            let overridden = match key.as_str() {
                "temperature" => request.model_config.temperature.is_some(),
                "top_p" => request.model_config.top_p.is_some(),
                "top_k" => request.model_config.top_k.is_some(),
                _ => false,
            };
            if !overridden {
                body[key] = value.clone();
            }
        }
```

Replace the HTTP request section:

```rust
        // Send request
        let url = format!("{}/v1/messages", self.base_url);

        let mut req = self
            .client
            .post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("Content-Type", "application/json")
            .header("User-Agent", "claude-code/1.0.0")
            .json(&body);

        // Attach custom headers
        for (key, value) in &self.headers {
            req = req.header(key, value);
        }

        let response = req
            .send()
            .await
            .map_err(LLMError::Network)?;
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-provider/src/anthropic.rs crates/vol-llm-provider/src/factory.rs
git commit -m "feat: AnthropicProvider uses body defaults and custom headers"
```

---

### Task 4: Update `LLMProviderRegistry` to support `ProviderLoader`

**Files:**
- Modify: `crates/vol-llm-provider/src/registry.rs`

- [ ] **Step 1: Add `from_loader()` method**

In `crates/vol-llm-provider/src/registry.rs`, add the import at top:

```rust
use crate::loader::ProviderLoader;
```

Add new factory method:

```rust
    /// Create registry from a ProviderLoader
    pub fn from_loader(loader: &ProviderLoader) -> Result<Self, LLMError> {
        let mut registry = Self::new();
        for id in loader.ids() {
            let file_config = loader.get(id).unwrap();
            let llm_config = file_config.to_llm_config();
            let provider = create_provider(&llm_config)?;
            registry.providers.insert(id.to_string(), Arc::from(provider));
        }
        Ok(registry)
    }
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-provider/src/registry.rs
git commit -m "feat: add LLMProviderRegistry::from_loader()"
```

---

### Task 5: Remove `llm_providers` from `vol-config` and update `main.rs`

**Files:**
- Modify: `crates/vol-config/src/lib.rs`
- Modify: `crates/vol-monitor/src/main.rs`

- [ ] **Step 1: Remove `llm_providers` field from `Config`**

In `crates/vol-config/src/lib.rs`, remove the field:

```rust
    // REMOVE this line:
    // pub llm_providers: Vec<vol_llm_provider::LLMProviderConfig>,
```

Remove the entire line `pub llm_providers: Vec<vol_llm_provider::LLMProviderConfig>,` from the `Config` struct.

- [ ] **Step 2: Update `AgentAdviceConfig` to not require a specific provider ID format**

Keep `llm_provider_id` as-is — it now refers to a filename (without `.toml`) in `.agents/providers/`.

- [ ] **Step 3: Update `main.rs` to use `ProviderLoader`**

In `crates/vol-monitor/src/main.rs`, update the imports at the top. Add:

```rust
use vol_llm_provider::ProviderLoader;
```

Replace the LLM initialization block (lines ~89-123) with:

```rust
    // Initialize LLM provider registry from .agents/providers/
    let loader = ProviderLoader::load(None);
    let llm_registry: Option<LLMProviderRegistry> = if !loader.is_empty() {
        info!(
            "Initializing LLM providers: {} loaded from .agents/providers/",
            loader.len()
        );
        match LLMProviderRegistry::from_loader(&loader) {
            Ok(registry) => {
                info!("Available LLM providers: {:?}", registry.ids());

                // Verify agent_advice provider if configured
                if config.agent_advice.enabled {
                    if registry.contains(&config.agent_advice.llm_provider_id) {
                        info!(
                            "AgentAdvice will use provider: {}",
                            config.agent_advice.llm_provider_id
                        );
                    } else {
                        warn!(
                            "AgentAdvice provider '{}' not found in configured providers",
                            config.agent_advice.llm_provider_id
                        );
                    }
                }

                Some(registry)
            }
            Err(e) => {
                warn!("Failed to initialize LLM providers: {}", e);
                None
            }
        }
    } else {
        info!("No LLM providers configured in .agents/providers/");
        None
    };
```

- [ ] **Step 4: Update the Config::default() call in tests**

In `crates/vol-monitor/src/main.rs`, find the test config builder (around line 389) and remove the `llm_providers: vec![]` line.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-config/src/lib.rs crates/vol-monitor/src/main.rs
git commit -m "refactor: remove llm_providers from vol-config, use ProviderLoader"
```

---

### Task 6: Create example provider configs and cleanup legacy files

**Files:**
- Create: `.agents/providers/anthropic-dashscope.toml`
- Delete: `config/llm.example.toml`
- Modify: `config.agent-test.toml`
- Modify: `config.feishu-test.toml`

- [ ] **Step 1: Create example provider config**

Create `.agents/providers/anthropic-dashscope.toml`:

```toml
# Anthropic provider via DashScope (China-accessible endpoint)
provider = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_AUTH_TOKEN}"
base_url = "https://coding.dashscope.aliyuncs.com/apps/anthropic"

[body]
max_tokens = 8192
temperature = 0.7

[headers]
"anthropic-version" = "2023-06-01"
```

- [ ] **Step 2: Delete `config/llm.example.toml`**

```bash
rm config/llm.example.toml
```

- [ ] **Step 3: Remove `[[llm_providers]]` from `config.agent-test.toml`**

Remove lines 81-86 (the `[[llm_providers]]` section) from `config.agent-test.toml`.

- [ ] **Step 4: Remove `[[llm_providers]]` from `config.feishu-test.toml`**

Remove lines 80-85 (the `[[llm_providers]]` section) from `config.feishu-test.toml`.

- [ ] **Step 5: Commit**

```bash
git add .agents/providers/anthropic-dashscope.toml config/llm.example.toml config.agent-test.toml config.feishu-test.toml
git commit -m "chore: migrate to file-based provider config, remove legacy llm_providers"
```

---

### Task 7: Update test and example files in `vol-llm-agents`

**Files:**
- Modify: `crates/vol-llm-agents/tests/advice_agent_integration.rs`
- Modify: `crates/vol-llm-agents/tests/observer_integration.rs`
- Modify: `crates/vol-llm-agents/tests/e2e_log_counter_cli.rs`
- Modify: `crates/vol-llm-agents/tests/coding_web_tools_integration.rs`
- Modify: `crates/vol-llm-agents/tests/coding_e2e_test.rs`
- Modify: `crates/vol-llm-agents/examples/coding_agent_basic.rs`
- Modify: `crates/vol-llm-agents/examples/coding_agent_wordcount.rs`
- Modify: `crates/vol-llm-agents/src/ppt/agent.rs`
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`
- Modify: `crates/vol-llm-wiki/src/agent.rs`
- Modify: `crates/vol-llm-provider/src/registry.rs` (tests)

- [ ] **Step 1: Update `vol-llm-agents/tests/advice_agent_integration.rs`**

Replace `LLMProviderConfig` usage with `ProviderFileConfig`:

Change imports:
```rust
use vol_llm_provider::{ProviderFileConfig, LLMProviderRegistry, create_provider, Secret};
use vol_llm_core::LLMProvider;
use std::sync::Arc;
```

Replace the `LLMProviderConfig` construction with `ProviderFileConfig`:

```rust
    let file_config = ProviderFileConfig {
        provider: LLMProvider::Anthropic,
        model: "qwen3.5-plus".to_string(),
        api_key: Secret::literal("sk-test-key"),
        base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
        body: None,
        headers: None,
    };

    let llm_config = file_config.to_llm_config();
    let provider = create_provider(&llm_config).unwrap();
    let mut registry = LLMProviderRegistry::new();
    registry.providers.insert("anthropic-main".to_string(), Arc::from(provider));
```

- [ ] **Step 2: Apply the same pattern to all other test/example files**

Pattern for each file:
1. Change imports: remove `LLMProviderConfig`, add `ProviderFileConfig`, `create_provider`
2. Replace `LLMProviderConfig { id, config: LLMConfig { ... } }` with `ProviderFileConfig { ... }` → `to_llm_config()` → `create_provider()` → insert into registry
3. Add `body: None, headers: None` to `ProviderFileConfig`
4. Add `use std::sync::Arc` if not present

Files to update:
- `crates/vol-llm-agents/tests/observer_integration.rs`
- `crates/vol-llm-agents/tests/e2e_log_counter_cli.rs`
- `crates/vol-llm-agents/tests/coding_web_tools_integration.rs`
- `crates/vol-llm-agents/tests/coding_e2e_test.rs`
- `crates/vol-llm-agents/examples/coding_agent_basic.rs`
- `crates/vol-llm-agents/examples/coding_agent_wordcount.rs`
- `crates/vol-llm-agents/src/ppt/agent.rs`
- `crates/vol-llm-agents/src/coding/agent.rs`
- `crates/vol-llm-wiki/src/agent.rs`

- [ ] **Step 3: Export `create_provider` from lib.rs**

In `crates/vol-llm-provider/src/lib.rs`, add:

```rust
pub use factory::create_provider;
```

This is needed by downstream tests to build providers from `LLMConfig`.

- [ ] **Step 4: Update `vol-llm-provider` registry tests**

In `crates/vol-llm-provider/src/registry.rs` tests, update struct literals to include the new `body` and `headers` fields.

Replace both test `LLMConfig` constructions (in `test_registry_from_configs` and `test_registry_get`) by adding `body: None, headers: None,` after the `base_url` field:

```rust
        config: LLMConfig::with_literal_key(
            LLMProvider::Anthropic,
            "claude-test",
            "TEST_API_KEY",
            "https://api.test.com",
        ),
```

Replace the full struct literal with the convenience constructor to avoid listing all fields.

- [ ] **Step 5: Update all downstream test/example files**

Pattern for each file: Replace `LLMProviderConfig { id, config: LLMConfig { ... } }` with:

```rust
let file_config = ProviderFileConfig {
    provider: LLMProvider::Anthropic,
    model: "...".to_string(),
    api_key: Secret::literal("sk-test-key"),
    base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
    body: None,
    headers: None,
};
let llm_config = file_config.to_llm_config();
let provider = create_provider(&llm_config).unwrap();
let mut registry = LLMProviderRegistry::new();
registry.providers.insert("anthropic-main".to_string(), Arc::from(provider));
```

Add these imports to each file:
```rust
use vol_llm_provider::{ProviderFileConfig, create_provider};
use std::sync::Arc;
```

**Files to update (exact replacements):**

`crates/vol-llm-agents/tests/advice_agent_integration.rs` — Replace lines ~48-62 (the LLMProviderConfig construction) with the pattern above.

`crates/vol-llm-agents/tests/observer_integration.rs` — Replace lines ~14-25.

`crates/vol-llm-agents/tests/e2e_log_counter_cli.rs` — Replace lines ~13-22.

`crates/vol-llm-agents/tests/coding_web_tools_integration.rs` — Replace lines ~34-45.

`crates/vol-llm-agents/tests/coding_e2e_test.rs` — Replace lines ~17-26.

`crates/vol-llm-agents/examples/coding_agent_basic.rs` — Replace lines ~48-58.

`crates/vol-llm-agents/examples/coding_agent_wordcount.rs` — Replace lines ~31-38.

`crates/vol-llm-agents/src/ppt/agent.rs` — Replace lines ~28-45. Remove `LLMProviderConfig` import. Use the simplified pattern:

```rust
        let file_config = ProviderFileConfig {
            provider: vol_llm_core::LLMProvider::Anthropic,
            model: "qwen3.5-plus".to_string(),
            api_key: vol_llm_provider::Secret::literal(api_key),
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
            body: None,
            headers: None,
        };

        let llm_config = file_config.to_llm_config();
        let provider = create_provider(&llm_config)
            .map_err(|e| PptAgentError::ConfigError(format!("Failed to initialize LLM: {}", e)))?;
        let mut registry = LLMProviderRegistry::new();
        registry.providers.insert(config.llm_provider_id.clone(), Arc::from(provider));
        let llm = registry.get(&config.llm_provider_id)
            .ok_or_else(|| PptAgentError::ConfigError(format!("LLM provider '{}' not found", config.llm_provider_id)))?;
```

`crates/vol-llm-agents/src/coding/agent.rs` — Replace lines ~67-80 (LLMProviderConfig construction). Use same pattern.

`crates/vol-llm-wiki/src/agent.rs` — Replace lines ~57-68 (LLMProviderConfig construction). Use same pattern.

---

### Task 8: Run tests and verify

- [ ] **Step 1: Run provider tests**

Run:
```bash
cargo test --package vol-llm-provider 2>&1 | tail -30
```

Expected: All tests pass.

- [ ] **Step 2: Run full compilation check**

Run:
```bash
cargo check 2>&1 | tail -30
```

Expected: No errors.

- [ ] **Step 3: Commit final fixes if needed**

```bash
git add -A && git commit -m "fix: address compilation/test failures"
```
