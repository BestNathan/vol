# ContextBuilder in ReActAgent Design Spec

## Overview

Replace `PromptContext` with `vol-llm-context`'s `ContextBuilder` as the unified context assembly mechanism for `ReActAgent`.

## Problem

`RunContext::init_messages()` manually assembles messages from `PromptContext.build_system()` + session history + user input. This is rigid — adding new context sources (skills, memory, files) requires modifying `init_messages()` directly. `vol-llm-context` already provides a trait-based, zone-aware, budget-managed alternative.

## Solution

Use `ContextBuilder` to build the entire message list for the agent run, including system prompts, skills, session history, and user input. Replace `prompt_context: PromptContext` in `AgentConfig` with `context_builder: ContextBuilder`.

## Architecture

```
AgentConfig { context_builder: ContextBuilder }
    → RunContext::init_messages()
    → ContextBuilder.build()
    → [Head blocks | Middle blocks | Tail blocks]
```

### New Contributors

**SessionContributor** — Wraps `Session` + `max_history` as a `ContextContributor`. Returns historical session messages as `ContextBlock` with `Middle(0)` anchor. **Placed in `vol-llm-agent`** (not `vol-llm-context`) to avoid a circular crate dependency — it depends on `vol-session` which `vol-llm-context` doesn't know about.

**UserInputContributor** — Wraps a `&str` as a `ContextContributor`. Returns one `Message::user(...)` with `Tail(0)` anchor. Placed in `vol-llm-context/src/builtin/` as a general-purpose contributor.

### Existing Contributors Used

- `SkillsContributor` → `Head(20)`
- `FileContributor` → configurable via `FileSpec { path, anchor }`

### Token Budget

- `total` = configured `context_window` from LLM config (passed to both ContextBuilder and LLM)
- `head_size` = `total / 4` (default, overridable)
- `tail_size` = `total / 4` (default, overridable)
- Middle gets the remainder (~50%)

Actual token usage from LLM responses can refine estimates via the `UsageUpdate` event.

### AgentConfig Changes

```rust
// Before
pub struct AgentConfig {
    pub prompt_context: PromptContext,
    // ...
}

// After
pub struct AgentConfig {
    pub context_builder: ContextBuilder,
    // ...
}
```

### RunContext::init_messages() Changes

```rust
// Before: system from PromptContext, then history, then user_input
let system_content = self.config.prompt_context.build_system();
messages.push(Message::system(system_content));
// history from session...
messages.push(Message::user(self.user_input.clone()));

// After: all from ContextBuilder
let mut builder = self.config.context_builder.clone();
builder.add_contributor(Box::new(SessionContributor::new(
    self.session.clone(),
    self.config.max_history_messages,
)));
builder.add_contributor(Box::new(UserInputContributor::new(
    self.user_input.clone(),
)));
let output = builder.build().await;
*self.messages.write().await = output.messages;
```

SessionContributor and UserInputContributor are added dynamically in `init_messages()` because they depend on runtime state (session, user_input). The builder's contributors (skills, files) are set up at agent construction time.

### AgentBuilder Integration

```rust
AgentBuilder::new()
    .context_window(128_000)           // sets token budget
    .add_contributor(Box::new(skills_contributor))
    .add_contributor(Box::new(FileContributor::new(vec![...])))
    // ... other config ...
    .build()
```

### ContextBuilder Cloning

`ContextBuilder` needs to implement `Clone` (or provide a `clone_builder()` method) since it's stored in `AgentConfig` (which is `Clone`) and used by multiple runs.

## File Changes

| File | Action | Reason |
|------|--------|--------|
| `crates/vol-llm-context/src/builtin/user_input.rs` | Create | `UserInputContributor` |
| `crates/vol-llm-context/src/builtin/mod.rs` | Modify | Export `UserInputContributor` |
| `crates/vol-llm-context/src/builder.rs` | Modify | Implement `Clone` for `ContextBuilder` and `ContextBuilderBuilder` |
| `crates/vol-llm-agent/Cargo.toml` | Modify | Add `vol-llm-context` dependency |
| `crates/vol-llm-agent/src/react/agent.rs` | Modify | Replace `PromptContext` with `ContextBuilder` in `AgentConfig` |
| `crates/vol-llm-agent/src/react/run_context.rs` | Modify | Rewrite `init_messages()` to use `ContextBuilder` |
| `crates/vol-llm-agent/src/react/builder.rs` | Modify | Update `AgentBuilder` to accept contributors and context_window |
| `crates/vol-llm-agent/src/react/context_contributors.rs` | Create | `SessionContributor` (agent-local, depends on vol-session) |
| `crates/vol-llm-agent/src/prompt_context/` | Keep | Not deleted — may be used elsewhere |

## Error Handling

- If `ContextBuilder.build()` returns empty messages, `init_messages()` still pushes the user input as a safety net (via `UserInputContributor`).
- Contributor failures are not fatal — each contributor handles its own errors (FileContributor logs warnings, SkillsContributor returns empty on no skills).

## Tests

1. `test_session_contributor_empty_session` — no history, returns empty
2. `test_session_contributor_with_history` — returns historical messages
3. `test_user_input_contributor` — returns single user message with Tail(0)
4. `test_init_messages_with_context_builder` — integration test: build full context
5. `test_agent_config_with_context_builder` — AgentConfig accepts ContextBuilder
6. Existing `test_init_messages_*` tests should pass after migration
