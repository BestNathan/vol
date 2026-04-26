# Session User Input Persistence Fix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Persist user input messages to the session JSONL and remove `UserInputContributor` from context building, making `SessionContributor` the sole conversation history source.

**Architecture:** At the start of each `run()`, the user message is added to the session via `add_message()`. `get_context()` no longer takes a parameter — it reads all conversation history from the session via `SessionContributor`. The `user_input` field stays on `RunContext` for metadata (PluginContext, AgentStreamEvent).

**Tech Stack:** Rust, tokio, vol-llm-agent, vol-session, vol-llm-context

---

### Task 1: Remove `UserInputContributor` from `get_context()` and add user message persistence

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:6` (import), `crates/vol-llm-agent/src/react/run_context.rs:235-258` (get_context method)
- Modify: `crates/vol-llm-agent/src/react/agent.rs:144` (add user message after RunContext creation), `crates/vol-llm-agent/src/react/agent.rs:295` (remove get_context parameter)

- [ ] **Step 1: Write a failing test for user message persistence**

In `crates/vol-llm-agent/src/react/run_context.rs`, add this new test in the `#[cfg(test)]` module (around line 728, replacing the existing `test_get_context_user_input`):

```rust
#[tokio::test]
async fn test_get_context_user_message_from_session() {
    use vol_llm_context::builtin::SimpleContributor;

    let context_builder = ContextBuilderBuilder::new(128_000)
        .add_contributor(Box::new(SimpleContributor::system("System".to_string())))
        .build();

    let config = AgentConfig {
        context_builder,
        ..Default::default()
    };

    let session = Arc::new(Session::new(
        Arc::new(InMemoryEntryStore::new()),
    ));

    let (ctx, _rx, _approval_rx) = RunContext::new(
        "test-run".to_string(),
        "analyze market volatility".to_string(),
        "session-1".to_string(),
        session.clone(),
        Arc::new(vol_llm_tool::ToolRegistry::new()),
        config,
    );

    // Persist user message to session (simulating what agent.rs does at run start)
    ctx.add_message(Message::user("analyze market volatility")).await.unwrap();

    // get_context should now pick up the user message from the session
    let messages = ctx.get_context().await.unwrap();

    // Should have: system + user message from session = 2 messages
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, MessageRole::System);
    assert_eq!(messages[1].role, MessageRole::User);
    assert!(messages[1]
        .content
        .as_ref()
        .unwrap()
        .as_str()
        .contains("analyze market volatility"));
}
```

- [ ] **Step 2: Run the new test to verify it fails**

Run: `cargo test -p vol-llm-agent test_get_context_user_message_from_session -- --nocapture`
Expected: FAIL — `get_context()` doesn't exist without parameters yet, and `UserInputContributor` is still being added.

- [ ] **Step 3: Remove `UserInputContributor` import and update `get_context()`**

In `crates/vol-llm-agent/src/react/run_context.rs`, line 6, change:

```rust
use vol_llm_context::{ContextBuilderBuilder, builtin::UserInputContributor};
```

to:

```rust
use vol_llm_context::ContextBuilderBuilder;
```

- [ ] **Step 4: Update `get_context()` method**

In `crates/vol-llm-agent/src/react/run_context.rs`, lines 235-258, change:

```rust
    /// Build the full LLM context for a run iteration.
    ///
    /// Combines:
    /// 1. Base contributors from config (e.g., system prompt, project context)
    /// 2. SessionContributor (Middle zone) — historical messages from session
    ///
    /// Call this at the start of each iteration to get the current message list.
    pub async fn get_context(&self) -> Result<Vec<Message>, crate::AgentError> {
        let context_builder = ContextBuilderBuilder::new(
            self.config.context_builder.token_budget().total,
        )
        .add_contributors_from(&self.config.context_builder)
        .add_contributor(Box::new(SessionContributor::new(
            Arc::new(tokio::sync::Mutex::new((*self.session).clone())),
            self.config.max_history_messages,
        )))
        .build();

        let output = context_builder.build().await.map_err(|e| {
            crate::AgentError::Context(format!("Failed to build context: {}", e))
        })?;
        Ok(output.messages)
    }
```

- [ ] **Step 5: Add user message persistence in `agent.rs`**

In `crates/vol-llm-agent/src/react/agent.rs`, after the `RunContext::new()` block (after line 143, before line 145), add:

```rust
        // Persist user message to session so it's available via SessionContributor.
        // This replaces the old UserInputContributor which injected input directly
        // into the context without persisting it.
        let user_msg = Message::user(user_input.to_string());
        run_ctx.add_message(user_msg).await.map_err(|e| {
            crate::AgentError::SessionError(format!("Failed to persist user message: {}", e))
        })?;
```

- [ ] **Step 6: Remove `get_context(&user_input)` parameter in `agent.rs`**

In `crates/vol-llm-agent/src/react/agent.rs`, line 295, change:

```rust
let messages = run_ctx.get_context(&user_input).await.map_err(|e| crate::AgentError::from(e))?;
```

to:

```rust
let messages = run_ctx.get_context().await.map_err(|e| crate::AgentError::from(e))?;
```

- [ ] **Step 7: Update existing tests — remove parameter from `get_context()` calls**

In `crates/vol-llm-agent/src/react/run_context.rs`, update these test methods:

`test_get_context_system_message` (line 632):
```rust
let messages = ctx.get_context().await.unwrap();
```

`test_get_context_history` (line 679):
```rust
let messages = ctx.get_context().await.unwrap();
```

Also update the assertion on line 681-689. After removing `UserInputContributor`, there are only 2 messages (system + history):
```rust
        // Should have: system + history = 2 messages (user input comes from session, not parameter)
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1].role, MessageRole::User);
        assert!(messages[1]
            .content
            .as_ref()
            .unwrap()
            .as_str()
            .contains("Previous conversation"));
```

`test_get_context_user_input` (line 693-728): Replace the entire test with the new `test_get_context_user_message_from_session` from Step 1.

`test_get_context_consistent` (lines 761-768):
```rust
        let messages_first = ctx.get_context().await.unwrap();
        let messages_second = ctx.get_context().await.unwrap();

        // Same count since no new messages were added between calls
        assert_eq!(messages_first.len(), messages_second.len());

        // Verify we have: system + history = 2 messages (user input from session, not parameter)
        assert_eq!(messages_second.len(), 2);
```

- [ ] **Step 8: Run tests and verify**

Run: `cargo test -p vol-llm-agent -- --test-threads=1`
Expected: All tests pass

- [ ] **Step 9: Verify workspace build**

Run: `cargo check --workspace`
Expected: No errors

- [ ] **Step 10: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs crates/vol-llm-agent/src/react/agent.rs
git commit -m "fix: persist user message to session and remove UserInputContributor

User input was passed via UserInputContributor directly into the LLM context
without being persisted to the session. Now the user message is added to the
session at run start, and get_context() reads all history from SessionContributor."
```
