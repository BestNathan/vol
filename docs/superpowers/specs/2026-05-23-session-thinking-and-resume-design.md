# Session Thinking Persistence & Resume History Preservation

## Problem

Two issues make session resume incomplete:

1. **Thinking content not persisted.** The `Message` struct has a `thinking: Option<String>` field but no code path populates it. Thinking is emitted as streaming events and recorded in transient `ReasoningStep` structs, but never attached to the persisted `Message`. On resume, the LLM cannot see its prior reasoning.

2. **AgentStart clears resumed history.** When a message is sent after resume, the `AgentStart` event clears all `ConversationEntry` items via `ac.entries.clear()`, destroying the resumed session history in the UI.

## Design

### Fix 1: Thinking persistence (2 files)

**`crates/vol-llm-core/src/message.rs`** — Add builder:
```rust
pub fn with_thinking(mut self, thinking: String) -> Self {
    self.thinking = Some(thinking);
    self
}
```

**`crates/vol-llm-agent/src/react/agent.rs`** — Wire thinking into two message construction sites:

- Assistant + tool_calls: `Message::assistant_with_tools(...).with_thinking(thinking)`
- Final assistant answer: `Message::assistant(content).with_thinking(thinking)`

No protocol changes. `SessionMessage` already wraps `Message` which serializes `thinking`. `SessionContributor` already returns `Message` with all fields. The LLM provider already handles `thinking` in API requests.

### Fix 2: Preserve resumed history (3 files)

**`crates/vol-llm-ui/src/state/mod.rs`** — Add `is_resumed: bool` to `AgentConversation`.

**`crates/vol-llm-ui/src/web/components/conversation.rs`** — In `reduce_conversation`, `AgentStart` handler: skip `entries.clear()` when `is_resumed` is true, then set `is_resumed = false`.

**`crates/vol-llm-ui/src/web/components/sessions_panel.rs`** — In resume callback, set `is_resumed = true` after loading entries.

## Verification

1. Send a message, verify the session JSONL file contains `thinking` in the assistant message entries
2. Resume a session, send a message, verify the conversation UI still shows the resumed history above the new message
3. Run `cargo test -p vol-llm-core -p vol-llm-agent -p vol-llm-agent-channel`
