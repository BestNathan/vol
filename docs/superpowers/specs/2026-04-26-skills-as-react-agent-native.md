# Skills as Native ReActAgent Capability

## Context

Skills (SkillLoader + SkillTool + SkillInjector) are currently initialized in `CodingAgent::build_tools_and_context()`. Skills should be a native capability of ReActAgent — provided as simple helpers that any agent can use during construction.

## Changes

### 1. `crates/vol-llm-agent/src/react/agent.rs`

Add two public helpers:

```rust
/// Register SkillTool into the tool registry.
/// Call this during agent construction, before wrapping the registry in Arc.
pub fn register_skills(tools: &mut vol_llm_tool::ToolRegistry, working_dir: &std::path::Path) {
    let loader = Arc::new(vol_llm_skill::SkillLoader::new(Some(working_dir.to_path_buf())));
    tools.register(vol_llm_skill::SkillTool::new(loader.clone()));
}

/// Enhance AgentConfig with SkillInjector in the context builder.
/// Call this during agent construction.
pub fn agent_config_with_skills(
    config: AgentConfig,
    working_dir: &std::path::Path,
) -> AgentConfig {
    let loader = Arc::new(vol_llm_skill::SkillLoader::new(Some(working_dir.to_path_buf())));
    let injector = vol_llm_skill::SkillInjector::new(loader);
    // Build new context from existing contributors + skill injector
    let mut builder = vol_llm_context::ContextBuilderBuilder::new(config.context_builder.token_budget().total)
        .add_contributors_from(&config.context_builder)
        .add_contributor(Box::new(injector));
    AgentConfig {
        context_builder: builder.build(),
        ..config
    }
}
```

Re-export from `lib.rs`:
```rust
pub use react::agent::{register_skills, agent_config_with_skills};
```

### 2. `crates/vol-llm-agents/src/coding/agent.rs`

In `build_tools_and_context()`, replace skill-related code:

```rust
// Before (remove these lines):
// let skill_loader = Arc::new(SkillLoader::new(Some(config.working_dir.clone())));
// tool_registry.register(SkillTool::new(skill_loader.clone()));
// let skill_injector = SkillInjector::new(skill_loader);

// After (replace with):
use vol_llm_agent::react::{register_skills, agent_config_with_skills};
// ... after register_coding_tools:
register_skills(&mut tool_registry, &config.working_dir);

// And for context builder, use agent_config_with_skills on the AgentConfig
// when it's built (in build_agent_config).
```

Remove `use vol_llm_skill::{SkillLoader, SkillInjector, SkillTool}` import.

### 3. `crates/vol-llm-agent/Cargo.toml`

Add: `vol-llm-skill = { path = "../vol-llm-skill" }`

### 4. `crates/vol-llm-agent/src/react/mod.rs`

Re-export the new functions.

## Verification

```bash
cargo test -p vol-llm-agent -- --test-threads=1
cargo test -p vol-llm-agents -- --test-threads=1
cargo check --workspace
```
