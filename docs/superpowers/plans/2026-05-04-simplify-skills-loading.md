# Simplify Skills Loading Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `SkillsConfig` wrapper; make skills auto-load during `AgentConfigBuilder::build()`, using `AgentDef.working_dir` to decide scope (project vs global).

**Architecture:** `AgentConfigBuilder` gains a `working_dir` field. `build()` constructs `SkillLoader::new(working_dir)` which auto-discovers skills from both `~/.agents/skills` (always) and `{working_dir}/.agents/skills` (if set). Skills are registered into the tool registry and context builder before the config is finalized.

**Tech Stack:** Rust, vol-llm-skill (SkillLoader, SkillTool, SkillInjector), vol-llm-agent

---

### Task 1: Add `working_dir` to `AgentDef` and `AgentFrontmatter`

**Files:**
- Modify: `crates/vol-llm-agent/src/agent_def.rs`

- [ ] **Step 1: Add `working_dir: Option<PathBuf>` field to `AgentDef` struct**

In `crates/vol-llm-agent/src/agent_def.rs`, add `use std::path::PathBuf` at the top if not present.

Add the field to `AgentDef`:

```rust
pub struct AgentDef {
    // ... existing fields ...
    /// Working directory for skill/agent discovery scope.
    pub working_dir: Option<PathBuf>,
}
```

Update `AgentDef::new()` to initialize it as `None`:

```rust
pub fn new(name: &str, content: impl Into<String>) -> Self {
    let content_str = content.into();
    Self {
        id: format!("code:{}", name),
        name: name.to_string(),
        r#type: name.to_string(),
        description: String::new(),
        scope: AgentScope::Repo,
        tools: None,
        disallowed_tools: None,
        model: None,
        max_iterations: None,
        max_history_messages: None,
        prompt: content_str,
        working_dir: None,
    }
}
```

Add a builder-style setter after `with_max_history_messages`:

```rust
/// Set the working directory for skill discovery scope.
pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
    self.working_dir = Some(dir);
    self
}
```

- [ ] **Step 2: Add `working_dir` to `AgentFrontmatter`**

Add an optional `working_dir` field to `AgentFrontmatter` for YAML/Markdown frontmatter parsing:

```rust
pub struct AgentFrontmatter {
    // ... existing fields ...
    /// Optional. Working directory root for skill/agent discovery.
    #[serde(default)]
    pub working_dir: Option<String>,
}
```

- [ ] **Step 3: Update `agent_loader.rs` to pass `working_dir` from frontmatter**

In `crates/vol-llm-agent/src/agent_loader.rs`, where `AgentDef` is constructed from `AgentFrontmatter`, add:

```rust
working_dir: fm.working_dir.as_ref().map(PathBuf::from),
```

- [ ] **Step 4: Add inline test for `with_working_dir`**

```rust
#[test]
fn test_agent_def_with_working_dir() {
    let def = AgentDef::new("test", "prompt")
        .with_working_dir(PathBuf::from("/tmp/project"));
    assert_eq!(def.working_dir, Some(PathBuf::from("/tmp/project")));
}
```

- [ ] **Step 5: Verify and commit**

```bash
cargo check -p vol-llm-agent
cargo test -p vol-llm-agent agent_def -- --nocapture
git add crates/vol-llm-agent/src/agent_def.rs crates/vol-llm-agent/src/agent_loader.rs
git commit -m "feat(agent-def): add working_dir field to AgentDef and AgentFrontmatter"
```

---

### Task 2: Update `AgentConfigBuilder` to accept `working_dir` and auto-load skills in `build()`

**Files:**
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs`

- [ ] **Step 1: Add imports and `working_dir` field**

Add imports at the top:

```rust
use std::path::PathBuf;
use vol_llm_skill::{SkillInjector, SkillLoader, SkillTool};
```

Add to the `AgentConfigBuilder` struct:

```rust
pub struct AgentConfigBuilder {
    // ... existing fields ...
    working_dir: Option<PathBuf>,
}
```

Initialize as `None` in `new()`. Add the builder method:

```rust
pub fn with_working_dir(mut self, dir: PathBuf) -> Self {
    self.working_dir = Some(dir);
    self
}
```

- [ ] **Step 2: Rewrite `build()` to auto-load skills**

Replace the entire existing `build()` method with this:

```rust
pub fn build(self) -> Result<AgentConfig, AgentConfigBuildError> {
    let llm = self
        .llm
        .ok_or(AgentConfigBuildError::MissingLlm)?;

    // Determine effective working_dir: explicit override > def > None
    let working_dir = self.working_dir
        .or_else(|| self.def.as_ref().and_then(|d| d.working_dir.clone()));

    // Build tool registry: if tool_registry not set, build from individual tools
    let mut tools = match self.tool_registry {
        Some(registry) => {
            // Need mutable access to register SkillTool
            Arc::try_unwrap(registry).unwrap_or_else(|arc| {
                let inner = (*arc).clone();
                // Clear the inner registry to get a fresh one
                inner
            })
        }
        None => {
            let mut registry = ToolRegistry::new();
            for tool in self.tools {
                registry.register_boxed(tool);
            }
            registry
        }
    };

    // Auto-load skills into the tool registry
    let skill_loader = Arc::new(SkillLoader::new(working_dir.clone()));
    tools.register(SkillTool::new(skill_loader.clone()));

    // Create session if not provided
    let session = self.session.unwrap_or_else(|| {
        Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())))
    });

    // Build context builder, adding SkillInjector
    let context_builder = match self.context_builder {
        Some(cb) => {
            let injector = SkillInjector::new(skill_loader);
            let budget = cb.token_budget();
            let mut b = vol_llm_context::ContextBuilderBuilder::new(budget.total)
                .head_size(budget.head_size)
                .tail_size(budget.tail_size)
                .add_contributors_from(&cb)
                .add_contributor(Box::new(injector));
            for c in self.contributors {
                b = b.add_contributor(c);
            }
            b.build()
        }
        None => {
            let injector = SkillInjector::new(skill_loader);
            let mut b = vol_llm_context::ContextBuilderBuilder::new(128_000)
                .add_contributor(Box::new(injector));
            for c in self.contributors {
                b = b.add_contributor(c);
            }
            b.build()
        }
    };

    Ok(AgentConfig {
        def: self.def,
        llm,
        tools: Arc::new(tools),
        session,
        sandbox: self.sandbox,
        context_builder,
        plugin_registry: self.plugin_registry,
    })
}
```

- [ ] **Step 3: Remove `mut` from `build()` signature**

```rust
pub fn build(self) -> Result<AgentConfig, AgentConfigBuildError> {
```

- [ ] **Step 4: Verify and commit**

```bash
cargo check -p vol-llm-agent
git add crates/vol-llm-agent/src/react/config_builder.rs
git commit -m "feat(agent-config): add working_dir to builder, auto-load skills in build()"
```

---

### Task 3: Delete `SkillsConfig` and `AgentConfig::with_skills()`

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`
- Modify: `crates/vol-llm-agent/src/react/mod.rs`

- [ ] **Step 1: Remove `SkillsConfig` struct and `impl` from agent.rs**

Delete lines 90-137 (the `SkillsConfig` struct, its `impl` block, and the `AgentConfig::with_skills` method).

- [ ] **Step 2: Remove unused imports**

After deleting `SkillsConfig`, check if `std::path::Path` is still used. If not, remove it from imports.

- [ ] **Step 3: Delete tests that used `SkillsConfig`**

Delete these two tests:
- `test_skills_config_register_tool`
- `test_skills_config_enhance_context_builder`

- [ ] **Step 4: Remove `SkillsConfig` re-export from mod.rs**

Change in `react/mod.rs`:

```rust
// Before:
pub use agent::{AgentConfig, ReActAgent, SkillsConfig};

// After:
pub use agent::{AgentConfig, ReActAgent};
```

- [ ] **Step 5: Verify and commit**

```bash
cargo check -p vol-llm-agent
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/mod.rs
git commit -m "refactor(agent): delete SkillsConfig and with_skills(), skills now auto-loaded"
```

---

### Task 4: Update `CodingAgent` to remove `SkillsConfig` usage

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs`

- [ ] **Step 1: Remove `SkillsConfig` import**

Remove this line from the imports at the top:

```rust
use vol_llm_agent::react::SkillsConfig;
```

- [ ] **Step 2: Simplify `build_tools_and_context()`**

Replace the entire method:

```rust
fn build_tools_and_context(config: &CodingAgentConfig) -> Result<(Arc<ToolRegistry>, ContextBuilder), CodingAgentError> {
    use vol_llm_skill::{SkillInjector, SkillLoader, SkillTool};

    let mut tool_registry = ToolRegistry::new();
    Self::register_coding_tools(&mut tool_registry, &config.tool_config);

    // Register skill tool directly
    let loader = Arc::new(SkillLoader::new(Some(config.working_dir.clone())));
    tool_registry.register(SkillTool::new(loader.clone()));

    // Build context with skill injector
    let injector = SkillInjector::new(loader);
    let context_builder = vol_llm_context::ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
            "You are an expert coding assistant. Help users understand, modify, and improve their codebase.".to_string(),
        )))
        .add_contributor(Box::new(injector))
        .build();

    Ok((Arc::new(tool_registry), context_builder))
}
```

- [ ] **Step 3: Verify and commit**

```bash
cargo check -p vol-llm-agents
git add crates/vol-llm-agents/src/coding/agent.rs
git commit -m "refactor(coding-agent): remove SkillsConfig, use SkillLoader/SkillInjector directly"
```

---

### Task 5: Full workspace build and test

- [ ] **Step 1: Build entire workspace**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run vol-llm-agent lib tests**

```bash
cargo test -p vol-llm-agent --lib
```

- [ ] **Step 3: Run vol-llm-agents lib tests**

```bash
cargo test -p vol-llm-agents --lib
```

- [ ] **Step 4: Commit if needed**

---

## Self-Review

**Spec coverage check:**

| Spec requirement | Task |
|---|---|
| `working_dir: Option<PathBuf>` on `AgentDef` | Task 1 |
| `working_dir` on `AgentFrontmatter` | Task 1 |
| `AgentDef::with_working_dir()` setter | Task 1 |
| `AgentLoader` passes `working_dir` from frontmatter | Task 1 |
| `working_dir` field on `AgentConfigBuilder` | Task 2 |
| `AgentConfigBuilder::with_working_dir()` method | Task 2 |
| Auto-load skills in `build()` via `SkillLoader::new()` | Task 2 |
| Register `SkillTool` in tool registry automatically | Task 2 |
| Add `SkillInjector` to context builder automatically | Task 2 |
| Delete `SkillsConfig` struct | Task 3 |
| Delete `AgentConfig::with_skills()` | Task 3 |
| Remove `SkillsConfig` from `mod.rs` re-exports | Task 3 |
| Remove `SkillsConfig` import/usage from `CodingAgent` | Task 4 |
| `cargo check --workspace` + `cargo test` | Task 5 |

**Placeholder scan:** No TBD/TODO in any step.

**Type consistency:** All types match existing definitions. `SkillLoader::new(Option<PathBuf>)` already handles global + repo roots. `SkillTool::new(Arc<SkillLoader>)` and `SkillInjector::new(Arc<SkillLoader>)` match current signatures.

**Backward compatibility:** `AgentConfig::builder()` without `with_working_dir()` still works — `SkillLoader::new(None)` loads only global skills. Callers that previously used `.with_skills(path)` must now use `.with_working_dir(path)` instead.
