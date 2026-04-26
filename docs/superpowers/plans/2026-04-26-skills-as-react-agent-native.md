# Skills as Native ReActAgent Capability — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move skill initialization from CodingAgent into ReActAgent as native helpers, so any agent gets skill support automatically.

**Architecture:** Add a `SkillsConfig` struct in the react module that holds a shared `SkillLoader` and provides helper methods to register the tool and enhance the context builder. CodingAgent replaces its direct skill imports with these helpers.

**Tech Stack:** Rust, vol-llm-agent, vol-llm-skill, vol-llm-context

---

### Task 1: Add `SkillsConfig` helper to react module

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs` — add SkillsConfig struct and methods
- Modify: `crates/vol-llm-agent/src/react/mod.rs` — re-export SkillsConfig
- Modify: `crates/vol-llm-agent/Cargo.toml` — add vol-llm-skill dependency
- Test: `crates/vol-llm-agent/src/react/agent.rs` (inline tests)

- [ ] **Step 1: Add vol-llm-skill dependency to Cargo.toml**

Run: `cat crates/vol-llm-agent/Cargo.toml` to see current deps.

Add to `[dependencies]`:
```toml
vol-llm-skill = { path = "../vol-llm-skill" }
```

- [ ] **Step 2: Add `SkillsConfig` struct and methods to `agent.rs`**

Add after the existing imports in `crates/vol-llm-agent/src/react/agent.rs`:

```rust
use std::path::Path;
use std::sync::Arc;
use vol_llm_skill::{SkillLoader, SkillInjector, SkillTool};
use vol_llm_tool::ToolRegistry;

/// Holds a shared SkillLoader and provides helpers to register skills
/// into the tool registry and context builder.
///
/// Create this during agent construction and use the helper methods
/// to wire skills into tools and context.
pub struct SkillsConfig {
    loader: Arc<SkillLoader>,
}

impl SkillsConfig {
    /// Create from a working directory. The SkillLoader discovers skills
    /// lazily on first access (no I/O during construction).
    pub fn from_workdir(working_dir: &Path) -> Self {
        Self {
            loader: Arc::new(SkillLoader::new(Some(working_dir.to_path_buf()))),
        }
    }

    /// Register the SkillTool into the tool registry.
    /// Call this on a mutable reference before wrapping the registry in Arc.
    pub fn register_tool(&self, registry: &mut ToolRegistry) {
        registry.register(SkillTool::new(self.loader.clone()));
    }

    /// Build a new ContextBuilder from an existing one, adding the SkillInjector.
    /// Returns a new ContextBuilder with the injector appended.
    pub fn enhance_context_builder(
        &self,
        existing: &vol_llm_context::ContextBuilder,
    ) -> vol_llm_context::ContextBuilder {
        let injector = SkillInjector::new(self.loader.clone());
        vol_llm_context::ContextBuilderBuilder::new(existing.token_budget().total)
            .add_contributors_from(existing)
            .add_contributor(Box::new(injector))
            .build()
    }
}
```

- [ ] **Step 3: Add `AgentConfig` helper method `with_skills()`**

Add to the `impl AgentConfig` block (create one if it doesn't exist):

```rust
impl AgentConfig {
    /// Enhance this config with skill injection in the context builder.
    /// This creates a new config with an updated context_builder that
    /// includes the SkillInjector for the given working directory.
    pub fn with_skills(self, working_dir: &Path) -> Self {
        let skills = SkillsConfig::from_workdir(working_dir);
        let new_context = skills.enhance_context_builder(&self.context_builder);
        AgentConfig {
            context_builder: new_context,
            ..self
        }
    }
}
```

- [ ] **Step 4: Re-export from `mod.rs`**

Add to `crates/vol-llm-agent/src/react/mod.rs`:

```rust
pub use agent::SkillsConfig;
```

- [ ] **Step 5: Add inline tests**

Add to the existing `#[cfg(test)] mod tests` block in `agent.rs`:

```rust
#[test]
fn test_skills_config_from_workdir() {
    let skills = SkillsConfig::from_workdir(Path::new("/tmp/test-project"));
    // SkillsConfig created successfully (no IO at creation time)
}

#[test]
fn test_skills_config_register_tool() {
    let skills = SkillsConfig::from_workdir(Path::new("/tmp/test-project"));
    let mut registry = ToolRegistry::new();
    skills.register_tool(&mut registry);
    // SkillTool should be registered — verify by checking definitions
    let defs = registry.definitions();
    assert!(defs.iter().any(|d| d.name == "skill"));
}

#[test]
fn test_skills_config_enhance_context_builder() {
    let skills = SkillsConfig::from_workdir(Path::new("/tmp/test-project"));
    let existing = vol_llm_context::ContextBuilderBuilder::new(128_000).build();
    let enhanced = skills.enhance_context_builder(&existing);
    // Enhanced builder should have more contributors than the original
    // (We can verify by building and counting messages)
    // For now, just verify it builds without panicking
    drop(enhanced);
}
```

- [ ] **Step 6: Verify and commit**

Run:
```bash
cargo test -p vol-llm-agent -- --test-threads=1
```
Expected: All tests pass.

```bash
git add crates/vol-llm-agent/
git commit -m "feat(vol-llm-agent): add SkillsConfig as native ReActAgent helper

SkillsConfig holds a shared SkillLoader and provides methods to register
SkillTool and enhance the context builder with SkillInjector. This makes
skills a native capability of ReActAgent."
```

### Task 2: Remove skill initialization from CodingAgent

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/agent.rs` — remove skill imports, use SkillsConfig helpers
- Test: `crates/vol-llm-agents/src/coding/tests.rs` — update tests that reference skill internals

- [ ] **Step 1: Update `build_tools_and_context()` in `coding/agent.rs`**

Replace the skill-related code in `build_tools_and_context`:

Current code (REMOVE these lines):
```rust
use vol_llm_skill::{SkillLoader, SkillInjector, SkillTool};
...
let skill_loader = Arc::new(SkillLoader::new(Some(config.working_dir.clone())));
tool_registry.register(SkillTool::new(skill_loader.clone()));
let skill_injector = SkillInjector::new(skill_loader);
```

Replace with:
```rust
use vol_llm_agent::react::SkillsConfig;
...
let skills = SkillsConfig::from_workdir(&config.working_dir);
skills.register_tool(&mut tool_registry);
```

And change the context builder construction from:
```rust
let context_builder = vol_llm_context::ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
        "You are an expert coding assistant...".to_string(),
    )))
    .add_contributor(Box::new(skill_injector))
    .build();
```

To:
```rust
let base_context = vol_llm_context::ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
        "You are an expert coding assistant. Help users understand, modify, and improve their codebase.".to_string(),
    )))
    .build();
let context_builder = skills.enhance_context_builder(&base_context);
```

The function return type stays the same: `Result<(Arc<ToolRegistry>, ContextBuilder), CodingAgentError>`.

- [ ] **Step 2: Remove skill imports from CodingAgent**

Remove the `use vol_llm_skill::{SkillLoader, SkillInjector, SkillTool}` import line from `crates/vol-llm-agents/src/coding/agent.rs`.

- [ ] **Step 3: Verify and commit**

Run:
```bash
cargo test -p vol-llm-agents -- --test-threads=1
```
Expected: All tests pass.

```bash
cargo check --workspace
```
Expected: No errors.

```bash
git add crates/vol-llm-agents/src/coding/
git commit -m "refactor(vol-llm-agents): use SkillsConfig instead of direct skill init

Remove SkillLoader/SkillInjector/SkillTool imports from CodingAgent.
Use SkillsConfig helpers from vol-llm-agent to register skills,
making skills a native ReActAgent capability."
```

### Task 3: Final verification

- [ ] **Step 1: Full workspace test**

Run:
```bash
cargo test --workspace -- --test-threads=1
```
Expected: All tests pass.

- [ ] **Step 2: Commit if all green**

Verify git status is clean, no uncommitted changes.
