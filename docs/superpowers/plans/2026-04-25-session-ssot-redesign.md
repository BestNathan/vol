# Session as SSOT — ReAct Agent Context Redesign

**Date**: 2026-04-25
**Status**: Draft

## Summary

Remove dual-write message synchronization from ReAct Agent. Session becomes the **single source of truth** for all conversation messages. `RunContext` no longer maintains its own `messages` vector. Context is built on-demand from Session via `ContextBuilder`, and all message writes go only to Session.

---

## 1. Core Architecture Changes

### 1.1 Before (dual-write)

```
RunContext
  ├── messages: Arc<RwLock<Vec<Message>>>    ← runtime copy
  ├── session: Arc<Session>                  ← persistent copy
  └── add_message() → writes BOTH            ← dual-write, needs sync

init_messages() → build context → cache in RunContext.messages
get_messages()  → return RunContext.messages (cached)
```

### 1.2 After (Session as SSOT)

```
RunContext
  └── session: Arc<Session>                  ← only message store
  └── get_context(input) → build on-demand   ← no cache
  └── add_message() → writes Session only    ← single write path

init_messages() → DELETED
get_messages() → DELETED (replaced by get_context)
```

### 1.3 Key Principles

- **Session is the only message store.** No secondary copies anywhere.
- **No `init_messages()`.** Context is built on every call to `get_context()`.
- **`ContextBuilder` drives construction.** System prompt → session history → user input, assembled via contributors.
- **Errors propagate.** Session read/write failures return `Result`, caller must handle them.
- **`resume()` deleted.** Caller passes an existing session to `AgentConfig`, then calls `run()` — history is auto-loaded from Session.

---

## 2. Interface Changes

### 2.1 `ContextContributor` trait

```rust
// BEFORE
async fn contribute(&self) -> Vec<ContextBlock>;

// AFTER
async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError>;
```

`ContextError` is a new error type in `vol-llm-context`:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ContextError {
    #[error("contributor {0} failed: {1}")]
    ContributorError(String, String),
    #[error("token budget exceeded: {0}")]
    BudgetExceeded(usize),
    #[error("session error: {0}")]
    Session(#[from] vol_session::SessionError),
}
```

### 2.2 `ContextBuilder::build()`

```rust
// BEFORE
pub async fn build(mut self) -> ContextOutput;

// AFTER
pub async fn build(mut self) -> Result<ContextOutput, ContextError>;
```

The build loop now propagates `contribute()` errors. On error, build aborts — no partial context.

### 2.3 `SessionContributor`

**Deleted**: `cached_blocks: Mutex<Option<Vec<ContextBlock>>>` field.

**Simplified**:

```rust
pub struct SessionContributor {
    session: Arc<tokio::sync::Mutex<Session>>,
    max_history: usize,
}

#[async_trait]
impl ContextContributor for SessionContributor {
    async fn contribute(&self) -> Result<Vec<ContextBlock>, ContextError> {
        let history = self.session.lock().await
            .get_messages()
            .await?;
        if history.is_empty() {
            return Ok(vec![]);
        }
        // Apply max_history limit (take last N)
        let trimmed: Vec<Message> = history
            .into_iter()
            .map(|sm| sm.message)
            .rev()
            .take(self.max_history)
            .rev()
            .collect();
        let block = ContextBlock::new(trimmed, AttentionAnchor::Middle(0));
        Ok(vec![block])
    }

    async fn compress(&mut self) {
        let messages = self.session.lock().await
            .get_messages()
            .await
            .unwrap_or_default();
        if messages.is_empty() {
            return;
        }
        self.session.lock().await.compress(messages).await;
    }

    fn estimate_size(&self) -> usize {
        // Best-effort estimate without full Session read.
        // Returns 0 if unknown; ContextBuilder will still
        // check budget after collecting blocks.
        0
    }
}
```

No caching needed — Session itself is the cache. `compress()` mutates Session in place; next `contribute()` sees the compressed result.

### 2.4 `RunContext`

**Deleted fields**:
- `messages: Arc<RwLock<Vec<Message>>>`
- `last_message_id` is kept (needed for `add_message` parent_id tracking)

**Deleted methods**:
- `init_messages(&self) -> Result<(), AgentError>`
- `get_messages(&self) -> Vec<Message>`

**New method**:

```rust
pub async fn get_context(&self, user_input: &str) -> Result<Vec<Message>, crate::AgentError> {
    let context_builder = ContextBuilderBuilder::new(
        self.config.context_builder.token_budget().total,
    )
    .add_contributors_from(&self.config.context_builder)  // system, context files
    .add_contributor(Box::new(SessionContributor::new(
        Arc::new(tokio::sync::Mutex::new((*self.session).clone())),
        self.config.max_history_messages,
    )))
    .add_contributor(Box::new(UserInputContributor::new(user_input.to_string())))
    .build();

    let output = context_builder.build().await?;
    Ok(output.messages)
}
```

**Modified method**:

```rust
pub async fn add_message(&self, message: Message) -> Result<(), crate::AgentError> {
    // Only write to Session
    let session_msg = {
        let mut last_id = self.last_message_id.lock().unwrap();
        let mut msg = SessionMessage::new(self.session.id.clone(), message);
        if let Some(id) = last_id.as_ref() {
            msg = msg.with_parent_id(id.clone());
        }
        let new_id = msg.id.clone();
        *last_id = Some(new_id);
        msg
    };

    self.session.add_message(session_msg).await.map_err(|e| {
        crate::AgentError::SessionError(format!("Failed to save message: {}", e))
    })
}
```

No `self.messages.write().await.push(...)` anymore.

**Clone impl update**: Remove `messages` field from clone body.

### 2.5 `PluginContext` and `plugin_context_from_run_ctx`

**Delete** `messages` field from `PluginContext`:

```rust
pub struct PluginContext {
    pub run_id: String,
    pub user_input: String,
    pub session_id: String,
    // messages: Arc<RwLock<Vec<Message>>>  ← DELETED
    pub all_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub current_tool_calls: Arc<RwLock<Vec<ToolCall>>>,
    pub data: Arc<RwLock<HashMap<String, serde_json::Value>>>,
}
```

`plugin_context_from_run_ctx` no longer needs to populate messages:

```rust
pub fn plugin_context_from_run_ctx(ctx: &RunContext) -> PluginContext {
    PluginContext {
        run_id: ctx.run_id.clone(),
        user_input: ctx.user_input.clone(),
        session_id: ctx.session_id.clone(),
        all_tool_calls: ctx.all_tool_calls.clone(),
        current_tool_calls: ctx.current_tool_calls.clone(),
        data: ctx.data.clone(),
    }
}
```

Plugins work entirely through events (intercept/listen) and never read the message list.

### 2.6 `ReActAgent`

**`run()` method changes**:

```diff
- // Phase 2: Initialize messages
- run_ctx.init_messages().await?;

  loop {
      // ...
-     let messages = run_ctx.get_messages().await;
+     let messages = run_ctx.get_context(&user_input).await?;

      let request = ConversationRequest::with_history(None, messages)
          .with_tools(tools_defs)
          .with_tool_choice(ToolChoice::Auto);

-     // Emit LLMCallStart with full message history
-     let messages = run_ctx.get_messages().await;
+     let messages = run_ctx.get_context(&user_input).await?;
      run_ctx.emit(AgentStreamEvent::llm_call_start(iteration, messages)).await;
```

**Delete `resume()` method entirely**:

```rust
// DELETE the entire ReActAgent::resume() impl
// Caller migrates to: pass existing session into Config → call run()
```

---

## 3. All Implementations to Update

### 3.1 vol-llm-context

| File | Change |
|------|--------|
| `context_contributor.rs` | `contribute()` returns `Result<Vec<ContextBlock>, ContextError>`; update trait + test |
| `builder.rs` | `build()` returns `Result<ContextOutput, ContextError>`; propagate errors; update tests |
| `builtin/simple.rs` | `contribute()` wraps in `Ok(...)` |
| `builtin/user_input.rs` | `contribute()` wraps in `Ok(...)` |
| `builtin/file.rs` | `contribute()` wraps in `Ok(...)` (if exists) |
| `lib.rs` | Export `ContextError` |

### 3.2 vol-llm-agent

| File | Change |
|------|--------|
| `react/run_context.rs` | Delete `messages`, `init_messages`, `get_messages`; add `get_context`; update `add_message`; update `Clone`; update `plugin_context_from_run_ctx` |
| `react/agent.rs` | Delete `init_messages` call; replace `get_messages` with `get_context`; delete `resume()` method |
| `react/context_contributors.rs` | Simplify `SessionContributor`: delete `cached_blocks`, update `contribute()` to return `Result`, simplify `compress()`, simplify `estimate_size()` |
| `react/mod.rs` | Update `PluginContext` (remove `messages`); re-export `ContextError` if needed |
| `react/state.rs` | No change |
| `react/stream.rs` | No change |

### 3.3 vol-llm-agents

| File | Change |
|------|--------|
| `coding/agent.rs` | Delete any `resume()` usage; update test references to `init_messages`/`get_messages` |
| `advice/agent.rs` | Same if applicable |

### 3.4 vol-llm-tui

| File | Change |
|------|--------|
| `main.rs` | Delete any `resume()` calls; replace with session passthrough to `run()` |

### 3.5 Other consumers

| File | Change |
|------|--------|
| Tests referencing `init_messages`, `get_messages`, `RunContext.messages` | Update to use `get_context` and Session |
| `html_reporter.rs` (if reads messages) | No change if event-driven |
| `ObservabilityPlugin` | No change (event-driven) |

---

## 4. Error Handling

### 4.1 Session read failure in `get_context()`

`SessionContributor.contribute()` returns `Err(ContextError::Session(...))`. `ContextBuilder.build()` propagates it. `RunContext.get_context()` maps it to `AgentError::SessionError(...)`. Agent run aborts — the caller sees the error.

### 4.2 Session write failure in `add_message()`

`add_message()` returns `Err(AgentError::SessionError(...))`. Agent run aborts. No partial state — the message was not written.

### 4.3 Contributor non-Session failures

`SimpleContributor`, `UserInputContributor`, etc. never fail — they return `Ok(...)`. Only `SessionContributor` (and future contributors reading external data) can return errors.

---

## 5. Resume Flow Migration

### 5.1 Current flow (to delete)

```
ReActAgent.resume(user_input):
  1. session.resume_messages() → load checkpoint messages
  2. For each msg: session.add_message(msg) → pre-populate
  3. self.run(user_input) → SessionContributor loads from entry store
```

### 5.2 New flow

```
// Caller creates agent with the existing session
let agent = ReActAgentBuilder::new()
    .session(existing_session)   // already has entry store with checkpoint
    .build();

agent.run(user_input).await?;
// SessionContributor reads history from session's entry store
// No special resume path needed
```

The `Session` + `SessionEntryStore` already persist checkpoint state. When `run()` is called, `get_context()` → `SessionContributor.contribute()` → `session.get_messages()` automatically includes all checkpointed messages.

### 5.3 Caller migration

```rust
// OLD
let agent = builder.build();
agent.resume("continue fixing...").await?;

// NEW
let session = Session::resume_from_store(existing_session_id, entry_store);
let agent = ReActAgentBuilder::new().session(session).build();
agent.run("continue fixing...").await?;
```

For CodingAgent / TUI, this means: store the `Session` reference between runs, pass it into the next `CodingAgentConfig`.

---

## 6. Migration Plan

1. **vol-llm-context**: Add `ContextError`, change `contribute()` and `build()` to return `Result`
2. **vol-llm-context builtin contributors**: Wrap returns in `Ok(...)`
3. **vol-llm-agent context_contributors.rs**: Simplify SessionContributor, delete cache
4. **vol-llm-agent run_context.rs**: Delete messages/init_messages/get_messages, add get_context, update add_message
5. **vol-llm-agent agent.rs**: Delete init_messages call, replace get_messages, delete resume()
6. **vol-llm-agent mod.rs**: Simplify PluginContext, update plugin_context_from_run_ctx
7. **vol-llm-agents**: Remove resume() usage in CodingAgent/advice
8. **vol-llm-tui**: Remove resume() usage
9. **Tests**: Update all affected tests
10. **Full workspace verification**: `cargo test --workspace`
