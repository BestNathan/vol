# CodingAgent Parameter Refactoring Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make CodingAgent a thin, pass-through service facade — no env var reads, LLM from caller via config, sandbox auto-init from working_dir.

**Architecture:** CodingAgentConfig gets `llm: Option<Arc<dyn LLMClient>>` field replacing `llm_provider_id`. CodingAgent::new() reads LLM from config and auto-inits a LocalSandbox from config.working_dir. No API signature change — config remains the only parameter.

**Tech Stack:** Rust, async (tokio), vol-llm-core (Sandbox, LLMClient), vol-llm-agent (ReActAgent)

---

### Task 1: Update CodingAgentConfig

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/config.rs:1-78`

- [ ] **Step 1: Add `llm` field to CodingAgentConfig struct**

Add after line 10 (`agent_id: String`):

```rust
use std::sync::Arc;

/// LLM client for generating responses.
/// Caller constructs this; CodingAgent does not read env vars.
pub llm: Option<Arc<dyn vol_llm_core::LLMClient>>,
```

- [ ] **Step 2: Remove `llm_provider_id` field from CodingAgentConfig**

Remove line 35:

```rust
pub llm_provider_id: String,
```

- [ ] **Step 3: Update CodingAgentConfig::Debug impl**

Remove line 55 from Debug impl:

```rust
.field("llm_provider_id", &self.llm_provider_id)
```

Add after line 47 (after agent_id field):

```rust
.field("llm", &"<LLMClient>")
```

- [ ] **Step 4: Update CodingAgentConfig::default()**

Replace lines 64-77 with:

```rust
impl Default for CodingAgentConfig {
    fn default() -> Self {
        Self {
            agent_id: "coding-agent".to_string(),
            max_iterations: 10,
            working_dir: PathBuf::from("."),
            log_base_path: PathBuf::from("logs"),
            hitl_enabled: true,
            unsafe_mode: false,
            verbose: false,
            html_report_path: None,
            llm: None,
            plugin_registry: PluginRegistry::new(),
            tool_config: ToolConfig::new(),
        }
    }
}
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-agents`
Expected: FAIL — CodingAgent::new() references removed `llm_provider_id` field

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agents/src/coding/config.rs
git commit -m "refactor: CodingAgentConfig carries LLMClient, removes llm_provider_id"
```

---

### Task 2: Rewrite CodingAgent::new() and add sandbox init

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs:1-282`

- [ ] **Step 1: Remove LLMProviderConfig/Registry imports and add Sandbox import**

Remove from imports (line 7):

```rust
use vol_llm_provider::{LLMProviderConfig, LLMProviderRegistry};
```

No new imports needed — SandboxRef is already imported via `vol_llm_core::SandboxRef`.

- [ ] **Step 2: Rewrite CodingAgent::new() — remove env var reads and LLM construction**

Replace lines 39-89 (the entire `new()` method body) with:

```rust
    /// Create a new CodingAgent from config.
    ///
    /// The caller must provide an LLMClient via `config.llm`.
    /// If `config.working_dir` is not ".", a LocalSandbox is automatically
    /// created and passed to the ReActAgent.
    pub async fn new(config: CodingAgentConfig) -> Result<Self, CodingAgentError> {
        // Get LLM from config — caller constructs this
        let llm = config.llm.clone()
            .ok_or_else(|| CodingAgentError::Config("llm not set: config.llm must be provided by caller".to_string()))?;

        // Create tool registry with coding tools
        let mut tool_registry = ToolRegistry::new();
        Self::register_coding_tools(&mut tool_registry, &config.tool_config);

        // Create agent config - use plugin_registry from config
        let agent_config = AgentConfig {
            max_iterations: config.max_iterations,
            max_history_messages: 20,
            prompt_context: vol_llm_agent::PromptContext::new(
                vol_llm_agent::PromptTemplate::new("coding", "You are an expert coding assistant. Help users understand, modify, and improve their codebase.")
            ),
            verbose: config.verbose,
            plugin_registry: config.plugin_registry.clone(),
            agent_id: if config.agent_id.is_empty() { generate_agent_id() } else { config.agent_id.clone() },
            log_base_path: config.log_base_path.clone(),
        };

        // Auto-init sandbox from working_dir if not current directory
        let sandbox: Option<vol_llm_core::SandboxRef> = if config.working_dir != PathBuf::from(".") {
            let sandbox = crate::coding::sandbox::LocalSandbox::new(Some(config.working_dir.clone()));
            sandbox.start().map_err(|e| CodingAgentError::Config(
                format!("Failed to start sandbox at {:?}: {}", config.working_dir, e)
            ))?;
            Some(Arc::new(sandbox))
        } else {
            None
        };

        Ok(Self {
            config,
            state: Some(CodingAgentState {
                llm,
                tool_registry: Arc::new(tool_registry),
                agent_config,
            }),
            observer: None,
            sandbox,
        })
    }
```

- [ ] **Step 3: Add LocalSandbox import to agent.rs**

Add at the top of agent.rs (line 8, after existing imports):

```rust
use crate::coding::sandbox::LocalSandbox;
```

- [ ] **Step 4: Update with_sandbox() to allow overriding auto-init**

The existing `with_sandbox()` method (line 123) stays as-is — it allows callers to override the auto-init sandbox:

```rust
    /// Set the sandbox for tool execution (overrides auto-init from working_dir)
    pub fn with_sandbox(mut self, sandbox: vol_llm_core::SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }
```

No change needed — method already exists and sets `self.sandbox`.

- [ ] **Step 5: Add llm() builder method**

Add after line 218 (after `new()` method on CodingAgentBuilder):

```rust
    /// Set the LLM client for this agent.
    /// The caller constructs the LLM; CodingAgent does not read env vars.
    pub fn llm(mut self, llm: Arc<dyn vol_llm_core::LLMClient>) -> Self {
        self.config.llm = Some(llm);
        self
    }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-agents`
Expected: Compiles successfully

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "refactor: CodingAgent::new() reads LLM from config, auto-inits sandbox from working_dir"
```

---

### Task 3: Update coding_agent_basic.rs example

**Files:**
- Modify: `crates/vol-llm-agents/examples/coding_agent_basic.rs:1-155`

- [ ] **Step 1: Add LLM provider imports**

Add at the top after line 8:

```rust
use vol_llm_provider::{LLMProviderConfig, LLMProviderRegistry};
```

- [ ] **Step 2: Construct LLM externally and pass via config**

Replace lines 43-56 (the config creation and agent construction) with:

```rust
    // Construct LLM externally — CodingAgent does not read env vars
    let api_key = std::env::var("ANTHROPIC_AUTH_TOKEN")
        .expect("ANTHROPIC_AUTH_TOKEN must be set for this example");

    let llm_config = LLMProviderConfig {
        id: "anthropic-main".to_string(),
        config: vol_llm_provider::LLMConfig {
            provider: vol_llm_core::LLMProvider::Anthropic,
            model: "qwen3.5-plus".to_string(),
            api_key: vol_llm_provider::Secret::literal(api_key),
            base_url: "https://coding.dashscope.aliyuncs.com/apps/anthropic".to_string(),
        },
    };

    let registry = LLMProviderRegistry::from_configs(&[llm_config])
        .expect("Failed to create LLM provider registry");

    let llm = registry.get("anthropic-main")
        .expect("LLM provider 'anthropic-main' not found")
        .clone();

    let config = CodingAgentConfig {
        max_iterations: 30,
        working_dir: PathBuf::from("/tmp/deribit-ws-client"),
        hitl_enabled: false,
        unsafe_mode: true,
        verbose: true,
        agent_id: agent_id.clone(),
        log_base_path: log_base_path.clone(),
        tool_config,
        llm: Some(llm),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
        ..Default::default()
    };

    let agent = CodingAgent::new(config).await?;
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agents --example coding_agent_basic`
Expected: Compiles successfully

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/examples/coding_agent_basic.rs
git commit -m "refactor: example constructs LLM externally, passes via config.llm"
```

---

### Task 4: Run workspace-wide tests and verify

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: Compiles successfully with 0 errors, 0 warnings

- [ ] **Step 2: Run vol-llm-agents tests**

Run: `cargo test -p vol-llm-agents --lib`
Expected: All tests pass

- [ ] **Step 3: Commit (if tests pass)**

```bash
git commit -m "test: verify workspace compilation and tests after refactor"
```
