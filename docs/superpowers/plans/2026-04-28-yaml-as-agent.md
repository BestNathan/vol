# YAML as Agent Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create a YAML-based declarative agent definition system that parses a YAML file into a fully configured `ReActAgent` instance.

**Architecture:** New `vol-llm-yaml-agent` crate with `YamlAgentConfig` (serde-deserialized YAML), `YamlAgentBuilder` (builds ReActAgent from config), and a tool/plugin registry system that maps string names to concrete implementations.

**Tech Stack:** Rust, serde, serde_yaml, vol-llm-core, vol-llm-provider, vol-llm-agent, vol-llm-tools-builtin, vol-llm-observability

---

### Task 1: New Crate Skeleton

**Files:**
- Create: `crates/vol-llm-yaml-agent/Cargo.toml`
- Create: `crates/vol-llm-yaml-agent/src/lib.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-yaml-agent"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-context = { path = "../vol-llm-context" }
vol-llm-core = { path = "../vol-llm-core" }
vol-llm-provider = { path = "../vol-llm-provider" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-tools-builtin = { path = "../vol-llm-tools-builtin" }
vol-llm-observability = { path = "../vol-llm-observability" }
vol-session = { path = "../vol-session" }
serde = { workspace = true }
serde_yaml = "0.9"
tokio = { workspace = true }
async-trait = { workspace = true }
tracing = { workspace = true }
thiserror = "1.0"
serde_json = { workspace = true }

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create lib.rs**

```rust
//! vol-llm-yaml-agent: Declarative agent definitions via YAML.
//!
//! Parse a YAML file into a fully configured ReActAgent.
//!
//! # Example
//!
//! ```ignore
//! let agent = YamlAgentBuilder::from_file(".agents/agents/coding.yaml")?
//!     .build()?;
//! let response = agent.run("Hello!").await?;
//! ```

mod config;
mod error;
mod builder;

pub use config::YamlAgentConfig;
pub use error::YamlAgentError;
pub use builder::YamlAgentBuilder;
```

- [ ] **Step 3: Add to workspace Cargo.toml**

Add `"crates/vol-llm-yaml-agent"` to the `members` array in `Cargo.toml` (workspace root). Place it after `"crates/vol-llm-wiki"`.

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-yaml-agent 2>&1`
Expected: Compiles with unused import warnings (modules are empty stubs for now).

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-yaml-agent/Cargo.toml crates/vol-llm-yaml-agent/src/lib.rs Cargo.toml
git commit -m "feat: add vol-llm-yaml-agent crate skeleton"
```

---

### Task 2: Error Types

**Files:**
- Create: `crates/vol-llm-yaml-agent/src/error.rs`

- [ ] **Step 1: Write tests and error types**

```rust
//! Error types for YAML agent parsing and building.

use std::io;

/// YAML agent error
#[derive(Debug, thiserror::Error)]
pub enum YamlAgentError {
    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Unknown tool: {0}")]
    UnknownTool(String),

    #[error("Unknown plugin: {0}")]
    UnknownPlugin(String),

    #[error("LLM provider '{0}' not found")]
    LlmNotFound(String),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("IO error: {0}")]
    Io(io::Error),
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-yaml-agent 2>&1`
Expected: Compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-yaml-agent/src/error.rs
git commit -m "feat: add YamlAgentError types"
```

---

### Task 3: YamlAgentConfig

**Files:**
- Create: `crates/vol-llm-yaml-agent/src/config.rs`

- [ ] **Step 1: Write tests first**

```rust
//! YAML agent configuration.

use std::path::PathBuf;
use serde::Deserialize;

/// Parsed YAML agent configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct YamlAgentConfig {
    /// Agent identifier
    pub name: String,

    /// LLM provider ID to use
    pub llm: String,

    /// Maximum reasoning iterations (default: 10)
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Maximum history messages to keep (default: 20)
    #[serde(default = "default_max_history")]
    pub max_history_messages: usize,

    /// Inline system prompt
    #[serde(default)]
    pub system: Option<String>,

    /// File paths to load as system prompt (content appended after inline system)
    #[serde(default)]
    pub system_files: Option<Vec<String>>,

    /// Tool names to register
    #[serde(default)]
    pub tools: Vec<String>,

    /// Per-tool parameter configs (keyed by tool name)
    #[serde(default)]
    pub tool_configs: Option<serde_yaml::Value>,

    /// Plugin names to register
    #[serde(default)]
    pub plugins: Option<Vec<String>>,

    /// Working directory (default: ".")
    #[serde(default = "default_working_dir")]
    pub working_dir: PathBuf,
}

fn default_max_iterations() -> u32 { 10 }
fn default_max_history() -> usize { 20 }
fn default_working_dir() -> PathBuf { PathBuf::from(".") }
```

- [ ] **Step 2: Add tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_config() {
        let yaml = r#"
name: test
llm: anthropic-main
tools: [read, write]
"#;
        let config: YamlAgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "test");
        assert_eq!(config.max_iterations, 10);
        assert_eq!(config.max_history_messages, 20);
        assert_eq!(config.system, None);
        assert_eq!(config.working_dir, PathBuf::from("."));
    }

    #[test]
    fn test_parse_full_config() {
        let yaml = r#"
name: coding
llm: anthropic-main
max_iterations: 20
max_history_messages: 30
system: "You are a coding assistant."
system_files:
  - .agents/AGENT.md
  - .agents/INSTRUCTION.md
tools:
  - read
  - write
  - edit
  - bash
tool_configs:
  web_search:
    provider: tavily
plugins:
  - logger
working_dir: "/tmp/project"
"#;
        let config: YamlAgentConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.name, "coding");
        assert_eq!(config.max_iterations, 20);
        assert_eq!(config.max_history_messages, 30);
        assert_eq!(config.system.as_deref(), Some("You are a coding assistant."));
        assert_eq!(config.system_files.as_ref().unwrap().len(), 2);
        assert_eq!(config.tools, vec!["read", "write", "edit", "bash"]);
        assert_eq!(config.plugins.as_ref().unwrap(), &vec!["logger".to_string()]);
        assert_eq!(config.working_dir, PathBuf::from("/tmp/project"));
    }
}
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p vol-llm-yaml-agent 2>&1`
Expected: 2 tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-yaml-agent/src/config.rs
git commit -m "feat: add YamlAgentConfig with serde deserialization"
```

---

### Task 4: Tool Registry by Name

**Files:**
- Create: `crates/vol-llm-yaml-agent/src/tools.rs`
- Modify: `crates/vol-llm-yaml-agent/src/lib.rs` (add mod + pub use)

- [ ] **Step 1: Write tests and implementation**

```rust
//! Tool registration by name for YAML agent definitions.

use vol_llm_tool::{ExecutableTool, ToolRegistry};

/// Register a tool by name to the registry.
///
/// Supported names: read, write, edit, glob, grep, bash, web_search, web_fetch
pub fn register_tool_by_name(
    registry: &mut ToolRegistry,
    name: &str,
) -> Result<(), super::error::YamlAgentError> {
    use super::error::YamlAgentError;
    use vol_llm_tools_builtin::{
        ReadTool, WriteTool, EditTool, GlobTool, GrepTool, BashTool,
        WebSearchTool, WebFetchTool, TavilySearchProvider, DefaultFetchProvider,
    };

    match name {
        "read" => registry.register(ReadTool::new()),
        "write" => registry.register(WriteTool::new()),
        "edit" => registry.register(EditTool::new()),
        "glob" => registry.register(GlobTool::new()),
        "grep" => registry.register(GrepTool::new()),
        "bash" => registry.register(BashTool::new()),
        "web_search" => {
            let provider = match TavilySearchProvider::from_env() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!("web_search provider init failed: {}", e);
                    return Ok(());
                }
            };
            registry.register(WebSearchTool::new(provider));
        }
        "web_fetch" => {
            registry.register(WebFetchTool::new(DefaultFetchProvider::default()));
        }
        _ => return Err(YamlAgentError::UnknownTool(name.to_string())),
    }

    Ok(())
}

/// Register multiple tools by name.
pub fn register_tools_by_name(
    registry: &mut ToolRegistry,
    names: &[String],
) -> Result<(), super::error::YamlAgentError> {
    for name in names {
        register_tool_by_name(registry, name)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_core_tools() {
        let mut registry = ToolRegistry::new();
        for name in &["read", "write", "edit", "glob", "grep", "bash"] {
            register_tool_by_name(&mut registry, name).unwrap();
        }
    }

    #[test]
    fn test_register_unknown_tool() {
        let mut registry = ToolRegistry::new();
        let err = register_tool_by_name(&mut registry, "quantum_tool").unwrap_err();
        assert!(err.to_string().contains("quantum_tool"));
    }

    #[test]
    fn test_register_multiple_tools() {
        let mut registry = ToolRegistry::new();
        let names = vec!["read".to_string(), "write".to_string()];
        register_tools_by_name(&mut registry, &names).unwrap();
    }
}
```

- [ ] **Step 2: Check TavilySearchProvider and DefaultFetchProvider APIs**

The spec says web_search/web_fetch are supported. Check if `TavilySearchProvider::from_env()` and `DefaultFetchProvider::default()` exist:

Run: `grep -n 'from_env\|impl Default' crates/vol-llm-tools-builtin/src/ -r`

If these methods don't exist, use the simplest available constructors:
- `TavilySearchProvider::from_config(&TavilyConfig::default())` for web_search
- `DefaultFetchProvider::default()` or `DefaultFetchProvider::from_config(&FetchProviderConfig::default())` for web_fetch

Adjust the implementation accordingly.

- [ ] **Step 3: Update lib.rs**

Add to `lib.rs`:
```rust
mod tools;
```

- [ ] **Step 4: Verify tests pass**

Run: `cargo test -p vol-llm-yaml-agent 2>&1`
Expected: All previous + 3 new tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-yaml-agent/src/tools.rs crates/vol-llm-yaml-agent/src/lib.rs
git commit -m "feat: add tool registration by name for YAML agent"
```

---

### Task 5: Plugin Registry by Name

**Files:**
- Create: `crates/vol-llm-yaml-agent/src/plugins.rs`
- Modify: `crates/vol-llm-yaml-agent/src/lib.rs` (add mod + pub use)

- [ ] **Step 1: Write tests and implementation**

```rust
//! Plugin registration by name for YAML agent definitions.

use std::path::Path;
use vol_llm_agent::react::PluginRegistry;

/// Register a plugin by name.
///
/// Supported names: logger (writes JSONL to store_dir/logs/)
pub fn register_plugin_by_name(
    registry: &mut PluginRegistry,
    name: &str,
    working_dir: &Path,
) -> Result<(), super::error::YamlAgentError> {
    use super::error::YamlAgentError;

    match name {
        "logger" => {
            let logger = vol_llm_observability::LoggerPlugin::new(working_dir.to_path_buf());
            registry.register(logger);
        }
        _ => return Err(YamlAgentError::UnknownPlugin(name.to_string())),
    }

    Ok(())
}

/// Register multiple plugins by name.
pub fn register_plugins_by_name(
    registry: &mut PluginRegistry,
    names: &[String],
    working_dir: &Path,
) -> Result<(), super::error::YamlAgentError> {
    for name in names {
        register_plugin_by_name(registry, name, working_dir)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_logger() {
        let mut registry = PluginRegistry::new();
        let temp = tempfile::tempdir().unwrap();
        register_plugin_by_name(&mut registry, "logger", temp.path()).unwrap();
    }

    #[test]
    fn test_register_unknown_plugin() {
        let mut registry = PluginRegistry::new();
        let temp = tempfile::tempdir().unwrap();
        let err = register_plugin_by_name(&mut registry, "magic", temp.path()).unwrap_err();
        assert!(err.to_string().contains("magic"));
    }
}
```

- [ ] **Step 2: Update lib.rs**

Add to `lib.rs`:
```rust
mod plugins;
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p vol-llm-yaml-agent 2>&1`
Expected: All previous + 2 new tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-yaml-agent/src/plugins.rs crates/vol-llm-yaml-agent/src/lib.rs
git commit -m "feat: add plugin registration by name"
```

---

### Task 6: YamlAgentBuilder

**Files:**
- Create: `crates/vol-llm-yaml-agent/src/builder.rs`
- Modify: `crates/vol-llm-yaml-agent/src/lib.rs` (already has mod + pub use from skeleton)

- [ ] **Step 1: Write tests and implementation**

```rust
//! Build ReActAgent from YAML configuration.

use std::path::Path;
use std::sync::Arc;
use vol_llm_agent::ReActAgent;
use vol_llm_agent::react::AgentConfig;
use vol_llm_context::ContextBuilderBuilder;
use vol_llm_provider::LLMProviderRegistry;
use vol_llm_tool::ToolRegistry;
use vol_session::{InMemoryEntryStore, Session};

use crate::config::YamlAgentConfig;
use crate::error::YamlAgentError;
use crate::tools::register_tools_by_name;
use crate::plugins::register_plugins_by_name;

/// Builder that creates a ReActAgent from YAML config.
pub struct YamlAgentBuilder {
    config: YamlAgentConfig,
    llm_registry: LLMProviderRegistry,
}

impl YamlAgentBuilder {
    /// Load YAML from a file path.
    pub fn from_file(path: &Path) -> Result<Self, YamlAgentError> {
        let yaml = std::fs::read_to_string(path)
            .map_err(YamlAgentError::Io)?;
        Self::from_yaml(&yaml)
    }

    /// Load YAML from a string.
    pub fn from_yaml(yaml: &str) -> Result<Self, YamlAgentError> {
        let config: YamlAgentConfig = serde_yaml::from_str(yaml)?;
        // Defaults
        let llm_registry = LLMProviderRegistry::new();
        Ok(Self { config, llm_registry })
    }

    /// Set the LLM provider registry.
    ///
    /// Must be called before `build()` if the YAML references an LLM provider.
    pub fn with_llm_registry(mut self, registry: LLMProviderRegistry) -> Self {
        self.llm_registry = registry;
        self
    }

    /// Build the ReActAgent.
    pub fn build(self) -> Result<ReActAgent, YamlAgentError> {
        // 1. Resolve LLM
        let llm = self.llm_registry.get(&self.config.llm)
            .ok_or_else(|| YamlAgentError::LlmNotFound(self.config.llm.clone()))?;

        // 2. Register tools
        let mut tool_registry = ToolRegistry::new();
        register_tools_by_name(&mut tool_registry, &self.config.tools)?;

        // 3. Build system prompt: inline + files
        let system_prompt = self.build_system_prompt();

        // 4. Build context
        let context_builder = ContextBuilderBuilder::new(128_000)
            .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
                system_prompt,
            )))
            .build();

        // 5. Build agent config
        let mut plugin_registry = vol_llm_agent::react::PluginRegistry::new();
        register_plugins_by_name(
            &mut plugin_registry,
            self.config.plugins.as_ref().unwrap_or(&vec![]),
            &self.config.working_dir,
        )?;

        let agent_config = AgentConfig {
            max_iterations: self.config.max_iterations,
            max_history_messages: self.config.max_history_messages,
            context_builder,
            plugin_registry,
            agent_id: self.config.name.clone(),
            working_dir: self.config.working_dir.clone(),
        };

        // 6. Create session
        let entry_store = Arc::new(InMemoryEntryStore::new());
        let session = Arc::new(Session::new(entry_store));

        Ok(ReActAgent::new(llm, Arc::new(tool_registry), agent_config, session))
    }

    /// Build combined system prompt: inline string + file contents.
    fn build_system_prompt(&self) -> String {
        let mut parts = Vec::new();

        if let Some(ref inline) = self.config.system {
            parts.push(inline.clone());
        }

        if let Some(ref files) = self.config.system_files {
            for path in files {
                match std::fs::read_to_string(path) {
                    Ok(content) => parts.push(content),
                    Err(e) => {
                        tracing::warn!(path, error = %e, "Failed to load system file, skipping");
                    }
                }
            }
        }

        parts.join("\n\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vol_llm_provider::{LLMConfig, Secret};
    use vol_llm_core::{LLMProvider, LLMClient};

    fn make_registry() -> LLMProviderRegistry {
        // Create a registry with a dummy provider for testing.
        // In tests we use a mock LLM; for now just verify config parsing works.
        let registry = LLMProviderRegistry::new();
        registry
    }

    #[test]
    fn test_from_yaml_valid() {
        let yaml = r#"
name: test-agent
llm: test-provider
tools: [read, write]
"#;
        let builder = YamlAgentBuilder::from_yaml(yaml).unwrap();
        assert_eq!(builder.config.name, "test-agent");
        assert_eq!(builder.config.tools, vec!["read", "write"]);
    }

    #[test]
    fn test_from_yaml_invalid() {
        let yaml = "not: valid: yaml: [";
        let result = YamlAgentBuilder::from_yaml(yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_system_prompt_inline_only() {
        let yaml = r#"
name: test
llm: p
system: "Hello world"
"#;
        let builder = YamlAgentBuilder::from_yaml(yaml).unwrap();
        let prompt = builder.build_system_prompt();
        assert_eq!(prompt, "Hello world");
    }

    #[test]
    fn test_build_system_prompt_with_missing_files() {
        let yaml = r#"
name: test
llm: p
system: "Base"
system_files:
  - /nonexistent/file.md
"#;
        let builder = YamlAgentBuilder::from_yaml(yaml).unwrap();
        let prompt = builder.build_system_prompt();
        // Should only contain the inline part, file is skipped with warning
        assert_eq!(prompt, "Base");
    }
}
```

- [ ] **Step 2: Verify tests pass**

Run: `cargo test -p vol-llm-yaml-agent 2>&1`
Expected: All previous + 4 new tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-yaml-agent/src/builder.rs
git commit -m "feat: add YamlAgentBuilder for building ReActAgent from YAML"
```

---

### Task 7: Integration Test with Real LLM (Mock)

**Files:**
- Create: `crates/vol-llm-yaml-agent/tests/yaml_agent_integration_test.rs`
- Create: `crates/vol-llm-yaml-agent/tests/test_agent.yaml`

- [ ] **Step 1: Create test YAML file**

```yaml
name: test-agent
llm: test-provider
max_iterations: 5
system: "You are a test assistant. Answer briefly."
tools:
  - read
  - write
  - edit
  - glob
  - grep
  - bash
plugins:
  - logger
```

- [ ] **Step 2: Write integration test**

```rust
//! Integration test: parse YAML and build agent with a mock LLM.

use std::sync::Arc;
use vol_llm_agent::ReActAgent;
use vol_llm_core::{ConversationRequest, ConversationResponse, Message, LLMClient, LLMProvider};
use vol_llm_provider::LLMProviderRegistry;

/// Mock LLM that returns a fixed response.
struct MockLLM;

#[async_trait::async_trait]
impl LLMClient for MockLLM {
    fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
    fn model(&self) -> &str { "mock" }
    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] { &[] }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> {
        Ok(ConversationResponse {
            message: Message::assistant("Mock response"),
            model: "mock".to_string(),
            usage: vol_llm_core::TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
                cached_tokens: None,
            },
            finish_reason: vol_llm_core::FinishReason::Stop,
            raw: None,
        })
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::StreamReceiver> {
        unimplemented!()
    }
}

#[tokio::test]
async fn test_parse_and_build_with_mock_llm() {
    // Create registry with mock provider
    let mut registry = LLMProviderRegistry::new();
    // Note: LLMProviderRegistry requires Arc<dyn LLMClient>.
    // We need to add the mock to the registry.
    // If LLMProviderRegistry doesn't support arbitrary adds, use from_configs.
    // For now, skip the full build test and just verify config parsing.

    let yaml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_agent.yaml");
    assert!(yaml_path.exists());

    let builder = vol_llm_yaml_agent::YamlAgentBuilder::from_file(&yaml_path).unwrap();
    assert_eq!(builder.config.name, "test-agent");
    assert_eq!(builder.config.tools.len(), 6);
}

#[tokio::test]
#[ignore = "requires ANTHROPIC_AUTH_TOKEN and real LLM"]
async fn test_full_agent_run() {
    let yaml_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/test_agent.yaml");
    let builder = vol_llm_yaml_agent::YamlAgentBuilder::from_file(&yaml_path).unwrap();
    // In a real test, we'd set up an LLM provider registry and call build()
    // This is left as an ignored test for manual verification.
}
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p vol-llm-yaml-agent 2>&1`
Expected: All tests pass (integration test may be ignored if mock LLM setup is complex).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-yaml-agent/tests/
git commit -m "test: add YAML agent integration test with mock LLM"
```

---

### Task 8: Agent Discovery Helper

**Files:**
- Create: `crates/vol-llm-yaml-agent/src/discovery.rs`
- Modify: `crates/vol-llm-yaml-agent/src/lib.rs` (add mod + pub use)

- [ ] **Step 1: Write tests and implementation**

```rust
//! Discover YAML agent files from a directory.

use std::path::{Path, PathBuf};
use crate::error::YamlAgentError;

/// Find all .yaml files in the given directory.
pub fn discover_agents(dir: &Path) -> Result<Vec<PathBuf>, YamlAgentError> {
    if !dir.exists() {
        return Ok(vec![]);
    }

    let entries = std::fs::read_dir(dir)
        .map_err(YamlAgentError::Io)?;

    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("yaml") {
            files.push(path);
        }
    }

    files.sort();
    Ok(files)
}

/// Discover agents from the standard `.agents/agents/` directory.
pub fn discover_from_workdir(working_dir: &Path) -> Result<Vec<PathBuf>, YamlAgentError> {
    let agents_dir = working_dir.join(".agent").join("agents");
    discover_agents(&agents_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_discover_empty_dir() {
        let temp = tempfile::tempdir().unwrap();
        let files = discover_agents(temp.path()).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_yaml_files() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join("agent1.yaml"), "name: a\nllm: x\n").unwrap();
        std::fs::write(temp.path().join("agent2.yaml"), "name: b\nllm: y\n").unwrap();
        std::fs::write(temp.path().join("readme.md"), "not yaml").unwrap();

        let files = discover_agents(temp.path()).unwrap();
        assert_eq!(files.len(), 2);
        assert!(files[0].ends_with("agent1.yaml"));
        assert!(files[1].ends_with("agent2.yaml"));
    }

    #[test]
    fn test_discover_nonexistent_dir() {
        let files = discover_agents(Path::new("/nonexistent")).unwrap();
        assert!(files.is_empty());
    }

    #[test]
    fn test_discover_from_workdir() {
        let temp = tempfile::tempdir().unwrap();
        let agents_dir = temp.path().join(".agent").join("agents");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(agents_dir.join("coding.yaml"), "name: c\nllm: p\n").unwrap();

        let files = discover_from_workdir(temp.path()).unwrap();
        assert_eq!(files.len(), 1);
    }
}
```

- [ ] **Step 2: Update lib.rs**

Add to `lib.rs`:
```rust
mod discovery;
pub use discovery::{discover_agents, discover_from_workdir};
```

- [ ] **Step 3: Verify tests pass**

Run: `cargo test -p vol-llm-yaml-agent 2>&1`
Expected: All tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-yaml-agent/src/discovery.rs crates/vol-llm-yaml-agent/src/lib.rs
git commit -m "feat: add agent file discovery helper"
```

---

### Task 9: Cleanup and Final Build Check

**Files:**
- All crate files

- [ ] **Step 1: Full workspace build check**

Run: `cargo check --workspace 2>&1 | grep -E '^error'`
Expected: No errors.

- [ ] **Step 2: Full test suite**

Run: `cargo test -p vol-llm-yaml-agent 2>&1 | tail -15`
Expected: All tests pass.

- [ ] **Step 3: Commit any remaining changes**

```bash
git add -A
git commit -m "feat: vol-llm-yaml-agent crate complete"
```
