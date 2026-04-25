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

### 2.1 CodingAgentConfig

Add optional skill directory path:
```rust
pub struct CodingAgentConfig {
    // ... existing fields ...
    pub skill_dir: Option<PathBuf>,  // NEW
}
```

### 2.2 CodingAgent::new()

Build ContextBuilder with SkillInjector:
```rust
let mut context_builder = ContextBuilderBuilder::new(128_000)
    .add_contributor(Box::new(SimpleContributor::system(
        "You are an expert coding assistant...".to_string(),
    )));

// Add SkillInjector if skill_dir configured
if let Some(skill_dir) = &config.skill_dir {
    let loader = Arc::new(SkillLoader::new(Some(skill_dir.clone())));
    context_builder = context_builder.add_contributor(
        Box::new(SkillInjector::new(loader)),
    );
}

let context_builder = context_builder.build();
```

### 2.3 ReActAgent

No changes. Already supports arbitrary contributors via `AgentConfig.context_builder`.

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
| `crates/vol-llm-agents/src/coding/config.rs` | Add `skill_dir: Option<PathBuf>` |
| `crates/vol-llm-agents/src/coding/agent.rs` | Build ContextBuilder with SkillInjector |
| `crates/vol-llm-skill/src/injector.rs` | Add `test_skill_injector_clone_contribute` test |
| `crates/vol-llm-agents/src/coding/tests.rs` | Add integration tests for skill integration |
