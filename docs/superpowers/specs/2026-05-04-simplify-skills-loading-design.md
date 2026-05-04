# Simplify Skills Loading Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove `SkillsConfig` wrapper; make skills auto-load during `AgentConfigBuilder::build()`, using `AgentDef.working_dir` to decide scope (project vs global).

**Architecture:** `AgentConfigBuilder` gains a `working_dir` field. `build()` constructs `SkillLoader::new(working_dir)` which auto-discovers skills from both `~/.agents/skills` (always) and `{working_dir}/.agents/skills` (if set). Skills are registered into the tool registry and context builder before the config is finalized.

**Tech Stack:** Rust, vol-llm-skill (SkillLoader, SkillTool, SkillInjector), vol-llm-agent

---

## Current State

```
SkillsConfig (agent.rs:92-125)
  ├── from_workdir(working_dir) → creates Arc<SkillLoader>
  ├── register_tool(&self, registry) → registry.register(SkillTool)
  └── enhance_context_builder(&self, existing) → adds SkillInjector

Caller (CodingAgent):
  let skills = SkillsConfig::from_workdir(&dir);
  skills.register_tool(&mut registry);
  let context = skills.enhance_context_builder(&base);
```

`SkillsConfig` is a thin wrapper that forces callers to manually call two methods. The same logic can be handled automatically inside the builder.

## Target State

```
AgentConfigBuilder
  ├── working_dir: Option<PathBuf>
  └── build() {
        let loader = SkillLoader::new(working_dir);
        loader.register_tool(&mut tool_registry);   // auto
        loader.inject(&mut context_builder);         // auto
      }
```

No `SkillsConfig` needed. Skills always load — global skills if `working_dir` is None, global + project-level if set.

## Design Decisions

### 1. `working_dir` belongs on `AgentDef`, not on `AgentConfigBuilder`

`AgentDef` is the declarative source of truth. The builder extracts `working_dir` from `def.working_dir` during `build()`. If no def is provided, `working_dir` can also be set via `AgentConfigBuilder::with_working_dir()` for programmatic use.

### 2. Skills always load — no opt-in

`SkillLoader::new(working_dir)` already handles the scope logic:
- `Some(wd)` → registers both `~/.agents/skills` and `{wd}/.agents/skills`
- `None` → registers only `~/.agents/skills`

No error if directories don't exist — the loader silently discovers nothing.

### 3. Delete `SkillsConfig` entirely

After moving the logic into the builder, `SkillsConfig` and `AgentConfig::with_skills()` are deleted. All their functionality is absorbed into `build()`.

## File Changes

| File | Change |
|------|--------|
| `vol-llm-agent/src/agent_def.rs` | Add `working_dir: Option<PathBuf>` to `AgentDef` and `AgentFrontmatter`. Update `new()`. |
| `vol-llm-agent/src/react/config_builder.rs` | Add `working_dir: Option<PathBuf>` field and `with_working_dir()` method. In `build()`, auto-load skills via `SkillLoader::new(working_dir)`. |
| `vol-llm-agent/src/react/agent.rs` | Delete `SkillsConfig` struct and `AgentConfig::with_skills()`. Remove from `mod.rs` re-exports. |
| `vol-llm-agent/src/react/mod.rs` | Remove `SkillsConfig` re-export. |
| `vol-llm-agents/src/coding/agent.rs` | Remove `SkillsConfig` import and usage. Pass `working_dir` through builder instead. |
| `vol-llm-agent/src/react/tests.rs` | Update tests that used `SkillsConfig`. |
| `vol-llm-agents/src/coding/tests.rs` | Update tests that used `SkillsConfig`. |

## Implementation Order

1. Add `working_dir` to `AgentDef` + frontmatter parsing
2. Update `AgentConfigBuilder` to accept `working_dir` and auto-load skills in `build()`
3. Delete `SkillsConfig` and `with_skills()`
4. Update all callers (CodingAgent, tests)
5. `cargo check --workspace` + `cargo test`
