# Session History Messages Configuration Design

**Date:** 2026-04-08
**Author:** BestNathan

## Overview

Currently, the ReAct Agent uses `max_iterations` to limit the number of historical messages retrieved from the session. This is semantically incorrect because:
- `max_iterations` controls the ReAct loop execution limit
- Historical message retrieval should have its own independent configuration

This design adds a new `max_history_messages` configuration option with a default value of 20.

## Architecture

### Current State

```rust
// Current implementation (incorrect)
let history = session.get_messages(config.max_iterations as usize).await?;
```

### Target State

```rust
// New implementation (correct)
let history = session.get_messages(config.max_history_messages).await?;
```

## Configuration

### New Field: `max_history_messages`

```rust
pub struct AgentConfig {
    pub max_iterations: u32,
    pub max_history_messages: usize,  // NEW, default: 20
    pub system_prompt: String,
    pub verbose: bool,
}
```

**Default value:** 20 messages

**Scope:** This limit applies to:
- User messages
- Assistant responses
- Tool result messages

**Excluded:**
- System prompt (injected dynamically at runtime, not persisted to session)

## Message Persistence

### What Gets Saved to Session

When the agent completes a conversation turn, the following messages are saved:

1. **User input** - The original user query
2. **Assistant response** - The final answer (after any tool calls)
3. **Tool results** - Results from tool executions during the ReAct loop

### What Does NOT Get Saved

- **System prompt** - Injected dynamically on each `run()` call
- **Intermediate reasoning** - Only the final assistant response is saved

This approach:
- Keeps session storage efficient
- Preserves conversation context for multi-turn dialogues
- Allows system prompt to be changed between runs without affecting history

## Files to Modify

| File | Changes |
|------|---------|
| `crates/vol-llm-agent/src/react/agent.rs` | Add `max_history_messages` field to `AgentConfig`, use it in `run()` method |
| `crates/vol-llm-agent/src/react/builder.rs` | Add `with_max_history_messages()` builder method |
| `crates/vol-llm-agent/examples/session_example.rs` | Update example to demonstrate the new configuration |

## Backward Compatibility

- Default value of 20 provides reasonable behavior for existing users
- Builder pattern allows explicit configuration when needed
- No breaking changes to public API

## Testing

- Verify default value is 20
- Verify custom values are respected
- Verify history loading works correctly with different limits
- Verify tool results are properly included in history
