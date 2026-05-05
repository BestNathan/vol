---
type: concept
category: framework
tags: [context, builder, contributor]
created: 2026-05-04
updated: 2026-05-04
source_count: 2
---

# Context Builder

**Category:** Prompt construction framework
**Related:** [[session-as-ssot]], [[session-contributor]], [[skill-system]], [[run-context]]

## Definition

A builder pattern for constructing agent prompt context from multiple contributors. The `ContextBuilder` assembles system prompt, session history, skill context, and user input into a single message list via pluggable `ContextContributor` implementations.

## Key Points
- `ContextContributor` trait defines `contribute()` → `Result<Vec<ContextBlock>, ContextError>` [[session-ssot-redesign]]
- `ContextBuilder.build()` → `Result<ContextOutput, ContextError>` — errors propagate, no partial context [[session-ssot-redesign]]
- `ContextBuilderBuilder` provides fluent API for adding contributors with token budget management [[skills-as-react-native]]
- Multiple built-in contributors: `SimpleContributor` (system prompt), `SessionContributor` (history), `UserInputContributor` (current input), `SkillInjector` (skill context) [[session-ssot-redesign]]
- `ContextError` enum: `ContributorError`, `BudgetExceeded`, `Session` [[session-ssot-redesign]]

## How It Works

Context construction uses a pipeline of contributors:

```rust
let context_builder = ContextBuilderBuilder::new(128_000) // token budget
    .add_contributor(Box::new(SimpleContributor::system(prompt)))
    .add_contributor(Box::new(SessionContributor::new(session, max_history)))
    .add_contributor(Box::new(SkillInjector::new(loader)))
    .add_contributor(Box::new(UserInputContributor::new(input)))
    .build();

let output = context_builder.build().await?;
```

Each contributor is called in order, returning `Vec<ContextBlock>`. Blocks are accumulated until the token budget is exceeded. If any contributor returns an error, the build aborts — no partial context is produced.

`ContextBuilderBuilder::add_contributors_from(existing)` copies all contributors from an existing builder, enabling enhancement patterns (e.g., `SkillsConfig::enhance_context_builder()` copies existing contributors and appends `SkillInjector`).

### ContextContributor Trait

```rust
#[async_trait]
pub trait ContextContributor {
    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError>;
    async fn compress(&mut self);
    fn estimate_size(&self) -> usize;
}
```

`contribute()` returns the blocks this contributor provides. `compress()` allows the contributor to reduce its output size (e.g., LLM summarization). `estimate_size()` provides a pre-construction size estimate for budget checking.

### ContextError

```rust
pub enum ContextError {
    ContributorError(String, String),  // (contributor_name, error_message)
    BudgetExceeded(usize),             // total tokens
    Session(vol_session::SessionError), // session read failure
}
```

## Examples / Applications

- **Session as SSOT**: `SessionContributor` reads from Session on every build, no caching [[session-as-ssot]]
- **Skill integration**: `SkillInjector` appends skill-specific instructions [[skill-system]]
- **Token budgeting**: Builder stops adding blocks when budget is reached
- **Error propagation**: Session read failures abort the build, agent run sees the error

## Related Concepts
- [[session-contributor]]: Reads session history as context blocks
- [[session-as-ssot]]: Context built from Session on-demand
- [[skill-system]]: SkillInjector as a contributor
- [[context-error]]: Error types for context building
- [[agent-builder-pattern]]: ContextBuilder configured via agent builder
- [[run-context]]: Uses ContextBuilder for get_context()
