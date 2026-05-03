# CodingAgent Skill + Session + Context Builder Integration

**Date**: 2026-04-25
**Status**: Draft

## Summary

Integrate SkillInjector into CodingAgent's ContextBuilder so that skill metadata appears in the system prompt. SessionContributor (from vol-session) provides session history. Both are assembled by ContextBuilder via the existing `AgentConfig.context_builder` field — no ReActAgent code changes needed.

---

## 1. Architecture

### 1.1 ContextBuilder Assembly Order

```
Head zone (position 0) — System Prompt
  └── SimpleContributor::system("You are an expert coding assistant...")

Head zone (position 1) — Skill Metadata  ← NEW
  └── SkillInjector::new(loader)

Middle zone (position 0) — Session History  ← added in get_context()
  └── SessionContributor::new(session, max_history)

Tail zone (position 0) — User Input  ← added in get_context()
  └── UserInputContributor::new(user_input)
```

### 1.2 Flow

```
CodingAgent::new()
  └── Builds ContextBuilder with system prompt + SkillInjector
  └── Creates AgentConfig with context_builder
  └── Creates ReActAgent with AgentConfig + Session

ReActAgent::run(user_input)
  └── get_context(user_input):
      ├── Clones config's context_builder contributors (system, skills)
      ├── Adds SessionContributor (session history)
      ├── Adds UserInputContributor (current input)
      └── ContextBuilder.build() → Vec<Message>
```

### 1.3 No ReActAgent Changes

`ReActAgent::get_context()` already calls `add_contributors_from(&config.context_builder)` which clones all static contributors from config. SkillInjector implements `clone_box()`, so it copies cleanly. ReActAgent needs zero code changes.

---

## 2. Changes

### 2.1 Path Resolution Principle

**Rule**: Callers pass only `working_dir`. Each component appends its own subdirectory internally.

This prevents double-appending, mismatched conventions, and lets each component own its path logic.

| Component | Subdirectory |
|-----------|-------------|
| SkillInjector | `{working_dir}/.agents/skills` |
| Session | `{working_dir}/.agents/sessions` |
| LocalSandbox | `{working_dir}/.` (root) |

### 2.2 SkillInjector API Change

Add a constructor that takes `working_dir` and resolves its own path:

```rust
impl SkillInjector {
    pub fn from_workdir(working_dir: &Path) -> Self {
        let skill_dir = working_dir.join(".agents/skills");
        let loader = Arc::new(SkillLoader::new(Some(skill_dir)));
        Self::new(loader)
    }
}
```

### 2.3 CodingAgent::new()

```rust
let mut context_builder = ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(SimpleContributor::system(
        "You are an expert coding assistant...".to_string(),
    )))
    .add_contributor(Box::new(SkillInjector::from_workdir(&config.working_dir)));
```

No new fields on `CodingAgentConfig` — `working_dir` is already there.

---

## 3. Tests

### 3.1 Unit Tests (vol-llm-skill)

| Test | What it verifies |
|------|-----------------|
| `test_skill_injector_clone_box` (existing) | clone_box preserves name |
| `test_skill_injector_clone_contribute` (new) | clone_box produces identical contribute output |
| `test_skill_injector_compress_noop` (existing) | compress is no-op |

### 3.2 Integration Tests (vol-llm-agents)

| Test | What it verifies |
|------|-----------------|
| `test_coding_agent_context_with_skills` | ContextBuilder with SkillInjector + SessionContributor produces messages with skill metadata in Head zone |
| `test_coding_agent_context_zone_ordering` | Verify zone order: system → skills → session → user_input |
| `test_coding_agent_context_empty_session` | Empty session + skills still produces valid context |
| `test_coding_agent_context_clone` | ContextBuilder.clone() preserves all contributors including SkillInjector |

### 3.3 Test Tools

Use MockLlmClient from vol-llm-core. No real LLM API calls needed — tests verify context assembly, not LLM behavior.

---

## 4. File Changes

| File | Change |
|------|--------|
| `crates/vol-llm-agents/src/coding/agent.rs` | Build ContextBuilder with SkillInjector, derive path from `working_dir` |
| `crates/vol-llm-skill/src/injector.rs` | Add `test_skill_injector_clone_contribute` test |
| `crates/vol-llm-agents/src/coding/tests.rs` | Add integration tests for skill integration |
