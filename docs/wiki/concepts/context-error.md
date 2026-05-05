---
type: concept
category: framework
tags: [context, error, session]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Context Error

**Category:** Error handling
**Related:** [[context-builder]], [[session-as-ssot]], [[session-contributor]]

## Definition

`ContextError` is the error type returned by `ContextContributor.contribute()` and `ContextBuilder.build()`. It covers contributor failures, token budget exceeded, and session read/write errors.

## Key Points
- Introduced as part of the Session-as-SSOT redesign [[session-ssot-redesign]]
- `ContextContributor` trait changed from `contribute() -> Vec<ContextBlock>` to `contribute() -> Result<Vec<ContextBlock>, ContextError>` [[session-ssot-redesign]]
- `ContextBuilder.build()` now returns `Result<ContextOutput, ContextError>` — errors abort build, no partial context [[session-ssot-redesign]]

## How It Works

```rust
pub enum ContextError {
    ContributorError(String, String),  // (contributor_name, error_message)
    BudgetExceeded(usize),             // total tokens exceeded budget
    Session(vol_session::SessionError), // session read failure
}
```

Error handling:
- `SimpleContributor`, `UserInputContributor`, etc. never fail — return `Ok(...)`
- `SessionContributor` can return `ContextError::Session(...)` on read failure
- Any contributor failure aborts the entire build — no partial context is produced
- `RunContext.get_context()` maps `ContextError` to `AgentError::SessionError(...)`

## Related Concepts
- [[context-builder]]: Build() returns Result with this error
- [[session-contributor]]: Can return Session variant on failure
- [[session-as-ssot]]: Error propagation in the new architecture
