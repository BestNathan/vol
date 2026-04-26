# Session User Input Persistence Fix Design

**Date**: 2026-04-26
**Status**: Approved

## Problem

User input messages are not persisted to the session JSONL file. `UserInputContributor` injects user input directly into the LLM context at each iteration without calling `add_message()`. The session only contains assistant and tool messages, so session resume cannot reconstruct the original user prompt.

## Root Cause

`run_context.rs:get_context()` builds context as:
1. Base contributors (system prompt, skills)
2. `SessionContributor` — historical messages from session
3. `UserInputContributor` — current user input (tail zone)

`RunContext:add_message()` is only called for assistant and tool messages in the agent loop. The user message flows through `UserInputContributor` directly into the context, bypassing session persistence.

## Solution

Persist the user message to the session at the start of each run, then remove `UserInputContributor` entirely. `SessionContributor` becomes the sole source of conversation history.

### Changes

**`crates/vol-llm-agent/src/react/agent.rs`**

- After `RunContext::new()` (before the spawn), add user message to session:
  ```rust
  run_ctx.add_message(Message::user(user_input.to_string())).await?;
  ```
- Change `run_ctx.get_context(&user_input).await` → `run_ctx.get_context().await` (line 295)

**`crates/vol-llm-agent/src/react/run_context.rs`**

- Remove import: `builtin::UserInputContributor`
- Change signature: `get_context(&self, user_input: &str)` → `get_context(&self)`
- Remove the `.add_contributor(Box::new(UserInputContributor::new(user_input.to_string())))` line
- `user_input` stays as a `RunContext` field — still used by `plugin_context_from_run_ctx()`, `AgentStreamEvent::agent_start()`, and `PluginContext`

**Tests in `run_context.rs`**

- `test_get_context_system_message`, `test_get_context_history`, `test_get_context_consistent`: change `.get_context("...").await` → `.get_context().await`
- `test_get_context_user_input`: remove — it tested that `UserInputContributor` injects user input, which no longer exists. Replace with a test that verifies a user message persisted via `add_message()` appears in `get_context()` output via `SessionContributor`.

## No TUI Changes

TUI passes `user_input` to `agent.run()` which handles persistence internally. No TUI code changes needed.

## Before/After

```
Before:
  SessionContributor (session messages, missing user input)
  UserInputContributor (current input, not persisted)

After:
  add_message(user_input) at run start → session contains user message
  SessionContributor (session messages, including user input)
  No UserInputContributor
```
