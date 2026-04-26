# Register Skill Tool into CodingAgent

## Context

CodingAgent already imports `SkillTool` but never registers it. The `SkillInjector` (context contributor) is used via `from_workdir()` which creates its own internal `SkillLoader`. The goal is to share a single `SkillLoader` between both `SkillInjector` and `SkillTool`, so they discover and serve the same skills.

## Design

In `CodingAgent::new()`, create an `Arc<SkillLoader>` explicitly, then pass it to both `SkillInjector::new()` and `SkillTool::new()`.

### Code Change

**File:** `crates/vol-llm-agents/src/coding/agent.rs`

Replace the current skill_injector creation (lines 76-81):
```rust
let skill_injector = vol_llm_skill::SkillInjector::from_workdir(&config.working_dir).await;
let context_builder = vol_llm_context::ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
        "You are an expert coding assistant. Help users understand, modify, and improve their codebase.".to_string(),
    )))
    .add_contributor(Box::new(skill_injector))
    .build();
```

With:
```rust
let skill_loader = Arc::new(vol_llm_skill::SkillLoader::new(Some(config.working_dir.clone())));
let _ = skill_loader.discover_all().await;

let skill_injector = vol_llm_skill::SkillInjector::new(skill_loader.clone());
let skill_tool = vol_llm_skill::SkillTool::new(skill_loader);

tool_registry.register(skill_tool);

let context_builder = vol_llm_context::ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(vol_llm_context::builtin::SimpleContributor::system(
        "You are an expert coding assistant. Help users understand, modify, and improve their codebase.".to_string(),
    )))
    .add_contributor(Box::new(skill_injector))
    .build();
```

### Imports

The dead `use vol_llm_skill::SkillTool;` import at line 5 already exists. Add `vol_llm_skill::SkillLoader` — but since `from_workdir` is no longer used, we can remove it or keep it for tests. `SkillTool` import becomes active.

### Testing

Verify with `cargo check -p vol-llm-agents` and `cargo test -p vol-llm-skill`.
