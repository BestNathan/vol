# ReAct Agent Config Unification Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Consolidate all agent configuration (AgentDef, llm, tools, plugins, session, sandbox) into a single `AgentConfig` with `AgentConfig::builder()` and runtime tool filtering from `AgentDef`.

**Architecture:** `AgentConfig` holds all components. `ReActAgent::new(config)` takes a single parameter. `run()` reads `AgentDef` constraints to filter tools and set max_iterations/max_history. `AgentBuilder` is deleted, replaced by `AgentConfig::builder()`.

**Tech Stack:** Rust, tokio, vol-llm-agent, vol-llm-tool, vol-session

---

### Task 1: Extend AgentDef with max_history_messages

**Files:**
- Modify: `crates/vol-llm-agent/src/agent_def.rs:35-107`

The `AgentDef` struct needs `max_history_messages` field (currently only has `max_iterations`). Also update `AgentFrontmatter` and `new()` to support it.

```rust
// In AgentDef struct (line ~37), add:
pub max_history_messages: Option<usize>,

// In AgentDef::new() (line ~63), add:
max_history_messages: None,

// In AgentDef builder, add:
pub fn with_max_history_messages(mut self, max: usize) -> Self {
    self.max_history_messages = Some(max);
    self
}
```

Also update `AgentFrontmatter` to add the field:

```rust
// In AgentFrontmatter struct, add:
#[serde(default)]
pub max_history_messages: Option<usize>,
```

And update `AgentDef::new()` to accept the new field with `None` default. Also update `agent_loader.rs` line 88-99 to set `max_history_messages` from frontmatter when building `AgentDef`.

- [ ] **Step 1: Update AgentDef struct** — add `max_history_messages: Option<usize>` field
- [ ] **Step 2: Update AgentDef::new()** — add `max_history_messages: None`
- [ ] **Step 3: Update AgentDef builder** — add `with_max_history_messages` method
- [ ] **Step 4: Update AgentFrontmatter** — add `max_history_messages: Option<usize>` field
- [ ] **Step 5: Update agent_loader.rs** — set `max_history_messages` from frontmatter in the AgentDef construction at line 88
- [ ] **Step 6: Update tests** — add `max_history_messages` field to any AgentDef constructions in tests
- [ ] **Step 7: Run tests** — `cargo test -p vol-llm-agent --lib` — all must pass
- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent/src/agent_def.rs crates/vol-llm-agent/src/agent_loader.rs
git commit -m "feat: add max_history_messages to AgentDef and AgentFrontmatter"
```

---

### Task 2: Add ToolRegistry::filter() method

**Files:**
- Modify: `crates/vol-llm-tool/src/registry.rs`

Add a `filter()` method to `ToolRegistry` that returns a new `Arc<ToolRegistry>` with only the allowed tools (minus disallowed).

```rust
// Add to impl ToolRegistry block:

/// Create a filtered registry containing only the allowed tools,
/// minus any disallowed tools.
///
/// - `allowed=None` → all tools included
/// - `allowed=Some(...)` → only named tools included
/// - `disallowed=Some(...)` → excluded from result
pub fn filter(
    &self,
    allowed: Option<&[&str]>,
    disallowed: Option<&[&str]>,
) -> Arc<Self> {
    let disallowed_set: std::collections::HashSet<&str> = disallowed
        .map(|d| d.iter().copied().collect())
        .unwrap_or_default();

    let tools = match allowed {
        None => self.tools
            .iter()
            .filter(|(name, _)| !disallowed_set.contains(name.as_str()))
            .map(|(name, tool)| (name.clone(), tool.clone()))
            .collect(),
        Some(allow) => {
            let allowed_set: std::collections::HashSet<&str> = allow.iter().copied().collect();
            self.tools
                .iter()
                .filter(|(name, _)| allowed_set.contains(name.as_str()) && !disallowed_set.contains(name.as_str()))
                .map(|(name, tool)| (name.clone(), tool.clone()))
                .collect()
        }
    };

    Arc::new(Self { tools })
}
```

- [ ] **Step 1: Write the failing test**

```rust
// Add to crates/vol-llm-tool/src/registry.rs in #[cfg(test)] mod tests:

#[test]
fn test_filter_no_filters_keeps_all() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("tool_a"));
    registry.register(DummyTool::new("tool_b"));
    let filtered = registry.filter(None, None);
    assert_eq!(filtered.tool_names().len(), 2);
}

#[test]
fn test_filter_allowed_keeps_only_allowed() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("tool_a"));
    registry.register(DummyTool::new("tool_b"));
    let filtered = registry.filter(Some(&["tool_a"]), None);
    assert_eq!(filtered.tool_names().len(), 1);
    assert!(filtered.tool_names().contains(&"tool_a"));
}

#[test]
fn test_filter_disallowed_removes() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("tool_a"));
    registry.register(DummyTool::new("tool_b"));
    let filtered = registry.filter(None, Some(&["tool_a"]));
    assert_eq!(filtered.tool_names().len(), 1);
    assert!(filtered.tool_names().contains(&"tool_b"));
}

#[test]
fn test_filter_allowed_and_disallowed() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("tool_a"));
    registry.register(DummyTool::new("tool_b"));
    registry.register(DummyTool::new("tool_c"));
    let filtered = registry.filter(Some(&["tool_a", "tool_b"]), Some(&["tool_b"]));
    assert_eq!(filtered.tool_names().len(), 1);
    assert!(filtered.tool_names().contains(&"tool_a"));
}
```

Create a simple `DummyTool` for testing:

```rust
// In #[cfg(test)] mod tests block:

use async_trait::async_trait;
use crate::tool::{ExecutableTool, ToolContext, ToolResultType, ToolResult, ToolSensitivity};

struct DummyTool {
    name: String,
}
impl DummyTool {
    fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}
#[async_trait]
impl ExecutableTool for DummyTool {
    fn name(&self) -> &'static str { &self.name }
    fn description(&self) -> &'static str { "dummy" }
    fn parameters(&self) -> serde_json::Value { serde_json::json!({}) }
    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity { ToolSensitivity::Safe }
    async fn execute(&self, _args: &serde_json::Value, _context: &ToolContext) -> ToolResultType<ToolResult> {
        Ok(ToolResult::success("ok"))
    }
}
```

- [ ] **Step 2: Run tests to verify they fail** — `cargo test -p vol-llm-tool` — should fail with "method filter not found"
- [ ] **Step 3: Implement ToolRegistry::filter()** — add the method above
- [ ] **Step 4: Run tests to verify they pass** — `cargo test -p vol-llm-tool`
- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tool/src/registry.rs
git commit -m "feat: add ToolRegistry::filter() for runtime tool filtering"
```

---

### Task 3: Rewrite AgentConfig and add AgentConfig::builder()

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:20-57` (replaces `AgentConfig`, `Default`, `SkillsConfig`, `with_skills`)
- Create: `crates/vol-llm-agent/src/react/config_builder.rs` (new file for `AgentConfigBuilder`)
- Modify: `crates/vol-llm-agent/src/react/mod.rs:33-45` (add config_builder module, remove AgentBuilder re-export)
- Modify: `crates/vol-llm-agent/src/lib.rs:20-23` (remove `AgentBuilder` re-export)

New `AgentConfig` structure:

```rust
use crate::agent_def::AgentDef;
use vol_llm_core::SandboxRef;
use vol_session::Session;

/// Agent configuration — single source of truth for ReActAgent.
#[derive(Clone)]
pub struct AgentConfig {
    // === Declarative definition (optional) ===
    pub def: Option<AgentDef>,

    // === Runtime components ===
    pub llm: Arc<dyn vol_llm_core::LLMClient>,
    pub tools: Arc<vol_llm_tool::ToolRegistry>,
    pub session: Arc<vol_session::Session>,
    pub sandbox: Option<SandboxRef>,

    // === Context and plugins ===
    pub context_builder: vol_llm_context::ContextBuilder,
    pub plugin_registry: PluginRegistry,
}
```

Builder:

```rust
// In config_builder.rs:

use super::agent::AgentConfig;
use crate::agent_def::AgentDef;
use vol_llm_core::SandboxRef;
use vol_llm_tool::{ExecutableTool, ToolRegistry};
use vol_session::{InMemoryEntryStore, Session};
use vol_llm_context::{ContextBuilderBuilder, ContextContributor};

pub struct AgentConfigBuilder {
    def: Option<AgentDef>,
    llm: Option<Arc<dyn vol_llm_core::LLMClient>>,
    tools: Vec<Box<dyn ExecutableTool>>,
    tool_registry: Option<Arc<ToolRegistry>>,
    session: Option<Arc<Session>>,
    sandbox: Option<SandboxRef>,
    context_builder: Option<vol_llm_context::ContextBuilder>,
    plugin_registry: PluginRegistry,
    contributors: Vec<Box<dyn ContextContributor>>,
}

impl AgentConfigBuilder {
    pub fn new() -> Self {
        Self {
            def: None,
            llm: None,
            tools: Vec::new(),
            tool_registry: None,
            session: None,
            sandbox: None,
            context_builder: None,
            plugin_registry: PluginRegistry::new(),
            contributors: Vec::new(),
        }
    }

    pub fn with_def(mut self, def: AgentDef) -> Self {
        self.def = Some(def);
        self
    }

    pub fn with_llm(mut self, llm: Arc<dyn vol_llm_core::LLMClient>) -> Self {
        self.llm = Some(llm);
        self
    }

    pub fn with_tool<T: ExecutableTool + 'static>(mut self, tool: T) -> Self {
        self.tools.push(Box::new(tool));
        self
    }

    pub fn with_tools(mut self, tools: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(tools);
        self
    }

    pub fn with_session(mut self, session: Arc<Session>) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    pub fn with_context_builder(mut self, cb: vol_llm_context::ContextBuilder) -> Self {
        self.context_builder = Some(cb);
        self
    }

    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        use vol_llm_context::builtin::SimpleContributor;
        self.contributors.push(Box::new(SimpleContributor::system(prompt)));
        self
    }

    pub fn with_plugin<P: super::AgentPlugin + 'static>(mut self, plugin: P) -> Self {
        self.plugin_registry.register(plugin);
        self
    }

    pub fn with_plugin_registry(mut self, registry: PluginRegistry) -> Self {
        self.plugin_registry = registry;
        self
    }

    pub fn build(mut self) -> Result<AgentConfig, AgentConfigBuildError> {
        let llm = self.llm.ok_or(AgentConfigBuildError::MissingLlm)?;

        // Build tool registry: if tool_registry not set, build from individual tools
        let tools = match self.tool_registry {
            Some(registry) => registry,
            None => {
                let mut registry = ToolRegistry::new();
                for tool in self.tools {
                    registry.register_boxed(tool);
                }
                Arc::new(registry)
            }
        };

        // Create session if not provided
        let session = self.session.unwrap_or_else(|| {
            Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())))
        });

        // Build context builder
        let context_builder = match self.context_builder {
            Some(cb) => {
                if self.contributors.is_empty() {
                    cb
                } else {
                    let budget = cb.token_budget();
                    let mut b = ContextBuilderBuilder::new(budget.total)
                        .head_size(budget.head_size)
                        .tail_size(budget.tail_size);
                    for c in self.contributors {
                        b = b.add_contributor(c);
                    }
                    b.build()
                }
            }
            None => {
                let mut b = ContextBuilderBuilder::new(128_000);
                for c in self.contributors {
                    b = b.add_contributor(c);
                }
                b.build()
            }
        };

        Ok(AgentConfig {
            def: self.def,
            llm,
            tools,
            session,
            sandbox: self.sandbox,
            context_builder,
            plugin_registry: self.plugin_registry,
        })
    }
}

impl Default for AgentConfigBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentConfigBuildError {
    #[error("LLM client is required")]
    MissingLlm,
}
```

Add to `AgentConfig`:

```rust
impl AgentConfig {
    /// Create a new builder for AgentConfig.
    pub fn builder() -> super::config_builder::AgentConfigBuilder {
        super::config_builder::AgentConfigBuilder::new()
    }

    /// Convenience constructor for direct struct creation (used by downstream code).
    pub fn new(
        llm: Arc<dyn vol_llm_core::LLMClient>,
        tools: Arc<vol_llm_tool::ToolRegistry>,
        session: Arc<vol_session::Session>,
    ) -> Self {
        Self {
            def: None,
            llm,
            tools,
            session,
            sandbox: None,
            context_builder: ContextBuilderBuilder::new(128_000).build(),
            plugin_registry: PluginRegistry::new(),
        }
    }
}
```

- [ ] **Step 1: Create config_builder.rs** — copy the full builder code above
- [ ] **Step 2: Rewrite AgentConfig** in agent.rs — replace old struct with new structure and add `builder()` + `new()` convenience methods
- [ ] **Step 3: Update mod.rs** — add `pub mod config_builder;`, keep `pub use agent::{AgentConfig, ...}`, remove `pub use builder::AgentBuilder`
- [ ] **Step 4: Update lib.rs** — remove `AgentBuilder` from re-exports (line 21)
- [ ] **Step 5: Write tests** in config_builder.rs:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_def::AgentDef;
    use vol_llm_core::{ConversationRequest, ConversationResponse, LLMClient, LLMProvider, StreamEvent, StreamEventData, StreamReceiver, SupportedParam};

    struct MockLlm;
    #[async_trait::async_trait]
    impl LLMClient for MockLlm {
        fn provider(&self) -> LLMProvider { LLMProvider::Anthropic }
        fn model(&self) -> &str { "mock" }
        fn supported_params(&self) -> &[SupportedParam] { &[] }
        async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<ConversationResponse> { unimplemented!() }
        async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<StreamReceiver> {
            let (tx, rx) = tokio::sync::mpsc::channel(10);
            Ok(StreamReceiver::new(rx))
        }
    }

    #[tokio::test]
    async fn test_builder_minimal() {
        let result = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .build();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_builder_missing_llm() {
        let result = AgentConfigBuilder::new().build();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_builder_with_def() {
        let def = AgentDef::new("test", "prompt");
        let config = AgentConfigBuilder::new()
            .with_llm(Arc::new(MockLlm))
            .with_def(def.clone())
            .build()
            .unwrap();
        assert!(config.def.is_some());
        assert_eq!(config.def.as_ref().unwrap().name, "test");
    }
}
```

- [ ] **Step 6: Run tests** — `cargo test -p vol-llm-agent --lib` — all must pass
- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/config_builder.rs crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/mod.rs crates/vol-llm-agent/src/lib.rs
git commit -m "feat: unify AgentConfig with builder, remove AgentBuilder"
```

---

### Task 4: Update ReActAgent to use single-parameter constructor and runtime tool filtering

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:108-556`

Update `ReActAgent` to use the new `AgentConfig`:

```rust
pub struct ReActAgent {
    config: AgentConfig,
}

impl ReActAgent {
    /// Create a new ReActAgent from config.
    pub fn new(config: AgentConfig) -> Self {
        Self { config }
    }

    /// Run the ReAct loop.
    pub async fn run(&self, user_input: &str) -> Result<AgentResponse, crate::AgentError> {
        // ... existing Phase 1 code ...

        // === Tool filtering from AgentDef ===
        let effective_tools = if let Some(def) = &self.config.def {
            let allowed: Option<Vec<&str>> = def.tools.as_ref()
                .map(|t| t.iter().map(|s| s.as_str()).collect());
            let disallowed: Option<Vec<&str>> = def.disallowed_tools.as_ref()
                .map(|t| t.iter().map(|s| s.as_str()).collect());
            self.config.tools.filter(
                allowed.as_deref(),
                disallowed.as_deref(),
            )
        } else {
            self.config.tools.clone()
        };

        // Read max_iterations and max_history from def if set
        let max_iterations = self.config.def.as_ref()
            .and_then(|d| d.max_iterations)
            .unwrap_or(5); // default
        let max_history_messages = self.config.def.as_ref()
            .and_then(|d| d.max_history_messages)
            .unwrap_or(20); // default

        let run_id = uuid::Uuid::new_v4().simple().to_string();
        let config = AgentConfig {
            max_iterations,
            max_history_messages,
            ..self.config.clone()
        };
        let session = self.config.session.clone();

        let (run_ctx, plugin_rx) = RunContext::new(
            run_id.clone(),
            user_input.to_string(),
            self.config.session.id.clone(),
            session.clone(),
            effective_tools.clone(),
            config.clone(),
        );
        // ... rest of existing run() body, but replace:
        // - self.config → config (the cloned copy)
        // - self.tools → effective_tools
        // - self.llm → self.config.llm
        // - self.config.plugin_registry → config.plugin_registry
        // - self.sandbox → self.config.sandbox
    }
}
```

The `run()` method body is large. The key changes are:
1. Replace `self.tools` with `effective_tools` (filtered from def)
2. Replace `self.config.max_iterations` with the def-derived value
3. Replace `self.llm` with `self.config.llm`
4. Replace `self.sandbox` with `self.config.sandbox`
5. Replace `self.config.plugin_registry` with `config.plugin_registry`

**Specifically, in the existing `run()` body, these substitutions are needed:**

```
Line 171: let run_id = uuid::...  → keep as-is
Line 173: let tools = self.tools.clone() → let tools = effective_tools.clone()
Line 176: tools (→ effective_tools.clone())
Line 205: self.config.plugin_registry.plugins() → config.plugin_registry.plugins()
Line 213: self.config.plugin_registry.plugins() → config.plugin_registry.plugins()
Line 226: let llm = self.llm.clone() → let llm = self.config.llm.clone()
Line 227: let tools = self.tools.clone() → let tools = effective_tools.clone()
Line 230: let sandbox = self.sandbox.clone() → let sandbox = self.config.sandbox.clone()
Line 263: config.max_iterations → max_iterations (from def)
Line 276: config.max_iterations → max_iterations
Line 286: tools.definitions() → effective_tools.definitions()
Line 289: run_ctx.get_context() → keep as-is (uses config.max_history_messages via context_builder)
Line 390: tools.execute() → effective_tools.execute()
```

Note: Since `AgentConfig` no longer has `max_iterations`/`max_history_messages`/`working_dir`/`agent_id` fields, the `RunContext::new()` call needs adjustment. These runtime values should be stored in a local struct or computed from `def`. Let me reconsider — actually it's cleaner to keep `max_iterations`/`max_history_messages` as fields on `AgentConfig` too, set from `def` during builder construction. This avoids recomputing in `run()`.

Actually, the spec says: `AgentConfig` holds the components. `run()` reads from `def` for constraints. So `max_iterations` and `max_history_messages` should be computed in `run()` from `def` and passed to `RunContext` via a modified config or local values.

The cleanest approach: `RunContext` already receives `config: AgentConfig` — but now `AgentConfig` doesn't have `max_iterations`. So I need to either add them back, or change `RunContext` to accept them separately.

Given `RunContext` uses `config.max_history_messages` and the `run()` loop uses `config.max_iterations`, the simplest approach is to **keep these as computed fields on `AgentConfig`** — they are set during `run()` by cloning and setting them from `def`.

Actually, let me simplify: keep `max_iterations` and `max_history_messages` on `AgentConfig` as well. They can be set by the builder, but if `def` provides values, `run()` uses those. This avoids changing `RunContext`.

**Revised `AgentConfig`:**

```rust
pub struct AgentConfig {
    pub def: Option<AgentDef>,
    pub llm: Arc<dyn LLMClient>,
    pub tools: Arc<ToolRegistry>,
    pub session: Arc<Session>,
    pub sandbox: Option<SandboxRef>,
    pub context_builder: ContextBuilder,
    pub plugin_registry: PluginRegistry,
    pub max_iterations: u32,
    pub max_history_messages: usize,
}
```

With defaults in builder: `max_iterations: 5`, `max_history_messages: 20`. In `run()`, override from `def` if set:

```rust
let max_iterations = self.config.def.as_ref()
    .and_then(|d| d.max_iterations)
    .unwrap_or(self.config.max_iterations);
let max_history_messages = self.config.def.as_ref()
    .and_then(|d| d.max_history_messages)
    .unwrap_or(self.config.max_history_messages);
```

This means `RunContext` stays unchanged (it reads `config.max_history_messages` which is set before `run()` creates the context).

- [ ] **Step 1: Update AgentConfig** — add back `max_iterations: u32` and `max_history_messages: usize` fields with defaults
- [ ] **Step 2: Update AgentConfigBuilder** — add `with_max_iterations()` and `with_max_history_messages()` methods, default values in `build()`
- [ ] **Step 3: Update ReActAgent::new()** — `pub fn new(config: AgentConfig) -> Self { Self { config } }`
- [ ] **Step 4: Update run()** — compute `max_iterations`/`max_history_messages` from def, create `effective_tools` via filter, replace all `self.tools`/`self.llm`/`self.sandbox` references with `self.config.*`
- [ ] **Step 5: Run tests** — `cargo test -p vol-llm-agent --lib`
- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/config_builder.rs
git commit -m "feat: ReActAgent uses single-param constructor with runtime def-based filtering"
```

---

### Task 5: Update ReActAgent internal tests

**Files:**
- Modify: `crates/vol-llm-agent/src/react/tests.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_flow_test.rs`
- Modify: `crates/vol-llm-agent/tests/react_mock_test.rs`
- Modify: `crates/vol-llm-agent/tests/session_history_test.rs`
- Modify: `crates/vol-llm-agent/tests/debug_agent_output.rs`
- Modify: `crates/vol-llm-agent/tests/react_agent_integration.rs`
- Modify: `crates/vol-llm-agent/tests/agent_run_tests.rs`
- Modify: `crates/vol-llm-agent/tests/agent_alert_scenario.rs`
- Modify: `crates/vol-llm-agent/tests/agent_message_history_test.rs`
- Modify: `crates/vol-llm-agent/tests/agent_llm_integration.rs`
- Modify: `crates/vol-llm-agent/tests/code_agent_simulation.rs`
- Modify: `crates/vol-llm-agent/examples/agent_cli_approval.rs`
- Modify: `crates/vol-llm-agent/examples/session_example.rs`
- Modify: `crates/vol-llm-agent/examples/agent_observability_test.rs`

Replace all `AgentBuilder::new()...build()` and `ReActAgent::builder()...build()` with `AgentConfig::builder()...build()` then `ReActAgent::new(config)`.

Example migration pattern:

```rust
// Before:
let agent = AgentBuilder::new()
    .with_llm(mock_llm)
    .with_tool(my_tool)
    .with_max_iterations(5)
    .with_system_prompt("prompt".to_string())
    .build()?;

// After:
let config = AgentConfig::builder()
    .with_llm(mock_llm)
    .with_tool(my_tool)
    .with_max_iterations(5)
    .with_system_prompt("prompt".to_string())
    .build()?;
let agent = ReActAgent::new(config);
```

And for `ReActAgent::builder()`:

```rust
// Before:
let agent = ReActAgent::builder()
    .with_llm(mock_llm)
    .with_tool(my_tool)
    .build()?;

// After:
let config = AgentConfig::builder()
    .with_llm(mock_llm)
    .with_tool(my_tool)
    .build()?;
let agent = ReActAgent::new(config);
```

- [ ] **Step 1: Update react/tests.rs** — replace AgentBuilder usage
- [ ] **Step 2: Update each test file** — one at a time, run `cargo test -p vol-llm-agent` after each
- [ ] **Step 3: Run all vol-llm-agent tests** — `cargo test -p vol-llm-agent`
- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/tests.rs crates/vol-llm-agent/tests/ crates/vol-llm-agent/examples/
git commit -m "test: update all agent tests to use AgentConfig::builder()"
```

---

### Task 6: Update agent_tool.rs

**Files:**
- Modify: `crates/vol-llm-agent/src/agent_tool.rs:170-195`

The `AgentTool::execute()` method creates an `AgentConfig` and `ReActAgent`. Update to use the new builder pattern and pass the `AgentDef` for tool filtering.

```rust
// In agent_tool.rs execute(), replace lines 174-189:

let system_prompt = if def.content.trim().is_empty() {
    DEFAULT_AGENT_PROMPT.to_string()
} else {
    def.prompt.clone()  // renamed from content to prompt
};

let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));

// Use AgentConfig::builder() and pass def for tool filtering
let agent_config = AgentConfig::builder()
    .with_def((*def).clone())
    .with_llm(self.llm.clone())
    .with_tools(self.parent_tools.clone())
    .with_session(session)
    .with_max_iterations(def.max_iterations.unwrap_or(5))
    .with_system_prompt(system_prompt)
    .with_plugin_registry(PluginRegistry::new())
    .with_working_dir(self.working_dir.clone())
    .build()
    .map_err(|e| ToolError::ExecutionFailed(format!("Failed to build agent config: {}", e)))?;

let sub_agent = crate::react::ReActAgent::new(agent_config);
let response = sub_agent.run(&params.prompt).await.map_err(|e| {
    ToolError::ExecutionFailed(format!("Sub-agent failed: {}", e))
})?;
```

Wait — `with_working_dir` doesn't exist on the new builder. The `AgentConfig` doesn't have `working_dir` field. Actually, looking at the spec more carefully, `working_dir` should also be in `AgentConfig`. Let me reconsider.

The spec says: "重复的config都放在def里 比如 迭代次数 最大历史消息数 id workdir等". So `working_dir` and `agent_id` go into `AgentDef`. But for runtime convenience, they should also be on `AgentConfig` with defaults.

Add to `AgentConfig`:
- `pub agent_id: String`
- `pub working_dir: PathBuf`

With defaults: `agent_id = "agent_{timestamp}"`, `working_dir = PathBuf::from(".")`.

And add to `AgentConfigBuilder`:
- `with_agent_id(String)`
- `with_working_dir(PathBuf)`

- [ ] **Step 1: Add agent_id and working_dir to AgentConfig** with defaults
- [ ] **Step 2: Add builder methods** `with_agent_id()` and `with_working_dir()`
- [ ] **Step 3: Update agent_tool.rs** — use builder pattern, rename `content` → `prompt` in `AgentDef` usage
- [ ] **Step 4: Update agent_def.rs** — rename `content` to `prompt` everywhere
- [ ] **Step 5: Update agent_loader.rs** — use `prompt` instead of `content`
- [ ] **Step 6: Run tests** — `cargo test -p vol-llm-agent --lib`
- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/config_builder.rs crates/vol-llm-agent/src/agent_tool.rs crates/vol-llm-agent/src/agent_def.rs crates/vol-llm-agent/src/agent_loader.rs
git commit -m "feat: update AgentTool to use unified AgentConfig with AgentDef"
```

---

### Task 7: Update CodingAgent

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`

The `CodingAgent` currently has its own `CodingAgentConfig` and calls `ReActAgent::new(llm, tools, agent_config, session)`. Update to use `AgentConfig::builder()`.

The `CodingAgent` itself is a higher-level wrapper and should remain — only its internal `ReActAgent` construction changes.

```rust
// In CodingAgent::build_agent_config(), replace the existing method:

fn build_agent_config(&self) -> AgentConfig {
    AgentConfig::builder()
        .with_llm(self.llm.clone())
        .with_tools(self.tool_registry.clone())
        .with_session(/* will be set in run() */)
        .with_context_builder(self.context_builder.clone())
        .with_plugin_registry(self.config.plugin_registry.clone())
        .with_agent_id(self.config.agent_id.clone())
        .with_working_dir(self.config.working_dir.clone())
        .with_max_iterations(self.config.max_iterations)
        .with_max_history_messages(20)
        .with_sandbox(self.sandbox.clone())
        .build()
        .expect("AgentConfig builder should not fail")
}
```

And in `run()`:

```rust
let mut agent_config = self.build_agent_config();
// Override session per run
agent_config.session = session.clone();

let react_agent = ReActAgent::new(agent_config);
let response = react_agent.run(task).await
    .map_err(|e| CodingAgentError::Agent(e))?;
```

- [ ] **Step 1: Update build_agent_config()** — use AgentConfig::builder()
- [ ] **Step 2: Update run()** — set session on config, use ReActAgent::new(config)
- [ ] **Step 3: Run tests** — `cargo test -p vol-llm-agents`
- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "feat: update CodingAgent to use unified AgentConfig"
```

---

### Task 8: Update AdviceAgent, WikiAgent, YamlAgent

**Files:**
- Modify: `crates/vol-llm-agents/src/advice/service.rs:140-159`
- Modify: `crates/vol-llm-wiki/src/agent.rs:160-174`
- Modify: `crates/vol-llm-yaml-agent/src/builder.rs:52-95`

Same pattern — replace `AgentBuilder` / 4-param `ReActAgent::new` with `AgentConfig::builder()` + `ReActAgent::new(config)`.

**AdviceAgent:**

```rust
// Replace lines 150-159:
let config = AgentConfig::builder()
    .with_llm(llm.clone())
    .with_tool(IndexPriceTool::new(None))
    .with_tool(VolatilityIndexTool::new(None))
    .with_tool(OptionsTool::new(None))
    .with_tool(RvTool::new(None))
    .with_max_iterations(5)
    .with_system_prompt(system_prompt().to_string())
    .build()
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
let agent = ReActAgent::new(config);
let response = agent.run(&user_prompt).await
    .map_err(|e| Box::new(e) as Box<dyn std::error::Error + Send + Sync>)?;
```

**WikiAgent:**

```rust
// Replace lines 160-174:
let agent_config = AgentConfig::builder()
    .with_llm(self.llm.clone())
    .with_tools(self.tool_registry.clone())
    .with_session(session)
    .with_context_builder(self.context_builder.clone())
    .with_max_iterations(self.config.max_iterations)
    .with_agent_id(self.config.agent_id.clone())
    .with_working_dir(self.config.working_dir.clone())
    .with_system_prompt(/* system prompt */)
    .build()
    .map_err(WikiAgentError::Config)?;
let react_agent = ReActAgent::new(agent_config);
```

**YamlAgentBuilder:**

```rust
// Replace line 94:
Ok(ReActAgent::new(agent_config))
// where agent_config is built via AgentConfig::builder()
```

- [ ] **Step 1: Update AdviceAgent** — replace AgentBuilder usage
- [ ] **Step 2: Update WikiAgent** — replace 4-param ReActAgent::new
- [ ] **Step 3: Update YamlAgentBuilder** — replace 4-param ReActAgent::new
- [ ] **Step 4: Run all tests** — `cargo test --workspace`
- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/advice/service.rs crates/vol-llm-wiki/src/agent.rs crates/vol-llm-yaml-agent/src/builder.rs
git commit -m "feat: update AdviceAgent, WikiAgent, YamlAgent to use unified AgentConfig"
```

---

### Task 9: Delete old builder.rs and clean up

**Files:**
- Delete: `crates/vol-llm-agent/src/react/builder.rs`
- Modify: `crates/vol-llm-agent/src/react/mod.rs:34` (remove `pub mod builder;`)
- Modify: `crates/vol-llm-agent/src/react/mod.rs:45` (remove `pub use builder::AgentBuilder;`)
- Modify: `crates/vol-llm-agent/src/react/mod.rs:22-30` (update module doc comment)

- [ ] **Step 1: Remove builder module** from mod.rs
- [ ] **Step 2: Delete builder.rs**
- [ ] **Step 3: Update doc comments** in mod.rs
- [ ] **Step 4: Run full workspace tests** — `cargo test --workspace`
- [ ] **Step 5: Commit**

```bash
git rm crates/vol-llm-agent/src/react/builder.rs
git add crates/vol-llm-agent/src/react/mod.rs
git commit -m "refactor: delete deprecated AgentBuilder, replaced by AgentConfig::builder()"
```

---

### Task 10: Remove old AgentConfig fields from builder and verify

**Files:**
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs`

Ensure the builder has all required methods. Add `with_agent_id()` and `with_working_dir()` if not present. Run full workspace test.

- [ ] **Step 1: Verify builder completeness** — all fields have `with_*` methods
- [ ] **Step 2: Run full workspace tests** — `cargo test --workspace 2>&1 | tail -30`
- [ ] **Step 3: Run cargo check** — `cargo check --workspace`
- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/config_builder.rs
git commit -m "chore: finalize AgentConfig builder completeness"
```
