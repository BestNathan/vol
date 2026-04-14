# Agent Event Observability Enhancements Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enhance all AgentStreamEvent variants with timestamps, add missing data (messages, model/usage, duration, response, parent_id), and remove direct debug/info prints from agent.rs.

**Architecture:** Modify AgentStreamEvent enum to carry `timestamp` on every variant. Update agent.rs to capture real model/usage from LLM stream, compute tool durations, and pass AgentResponse JSON to AgentComplete. Add `last_message_id` tracking to RunContext for auto parent_id. Remove `info!`/`debug!` prints from run() method.

**Tech Stack:** Rust, tokio, chrono, tracing, vol-llm-core, vol-llm-agent, vol-session, vol-llm-observability, vol-llm-tui

---

## File Structure

| File | Action | Purpose |
|------|--------|---------|
| `crates/vol-llm-core/src/stream.rs` | Modify | Add `timestamp` to all 18 variants; add helper constructors |
| `crates/vol-llm-agent/src/react/agent.rs` | Modify | Capture model/usage from stream, compute duration, pass response to AgentComplete, remove debug/info prints |
| `crates/vol-llm-agent/src/react/run_context.rs` | Modify | Add `last_message_id` field; modify `add_message` to auto-set parent_id |
| `crates/vol-session/src/listener.rs` | Modify | Update event_to_message match for new variant fields |
| `crates/vol-llm-tui/src/render.rs` | Modify | Update render_event match for new variant fields |
| `crates/vol-llm-observability/src/plugin.rs` | Modify | Update plugin to handle new fields (duration_ms in tool events, response in AgentComplete) |
| `crates/vol-llm-agent/src/react/plugin_stream.rs` | Modify | Update test AgentComplete construction |
| `crates/vol-llm-agent/src/plugins/` | Modify | Update rate_limiter.rs and caching.rs match patterns |
| `crates/vol-llm-agents/src/coding/html_reporter.rs` | Modify | Update match patterns |
| All test files (~15 files) | Modify | Update event construction for new fields |

---

### Task 1: Add timestamp to all AgentStreamEvent variants

**Files:**
- Modify: `crates/vol-llm-core/src/stream.rs:66-121`

- [ ] **Step 1: Add chrono dependency to vol-llm-core**

Check `crates/vol-llm-core/Cargo.toml` for chrono. If not present, add:
```toml
chrono = { version = "0.4", features = ["serde"] }
```

- [ ] **Step 2: Update all 18 AgentStreamEvent variants to include timestamp**

In `crates/vol-llm-core/src/stream.rs`, replace the enum definition (lines 66-121):

```rust
#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    // === Lifecycle (3) ===
    AgentStart {
        timestamp: chrono::DateTime<chrono::Utc>,
        input: String,
    },
    AgentComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    AgentAborted {
        timestamp: chrono::DateTime<chrono::Utc>,
        reason: String,
    },

    // === LLM Call (3) ===
    LLMCallStart {
        timestamp: chrono::DateTime<chrono::Utc>,
        iteration: u32,
        messages: Vec<Message>,
    },
    LLMCallComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        model: String,
        usage: Option<TokenUsage>,
    },
    LLMCallError {
        timestamp: chrono::DateTime<chrono::Utc>,
        error: String,
    },

    // === Streaming: Thinking (3) ===
    ThinkingStart {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ThinkingDelta {
        timestamp: chrono::DateTime<chrono::Utc>,
        delta: String,
    },
    ThinkingComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        thinking: String,
    },

    // === Streaming: Content (3) ===
    ContentStart {
        timestamp: chrono::DateTime<chrono::Utc>,
    },
    ContentDelta {
        timestamp: chrono::DateTime<chrono::Utc>,
        delta: String,
    },
    ContentComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        content: String,
    },

    // === Tool Execution (4) ===
    ToolCallBegin {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        arguments: String,
    },
    ToolCallComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        result: String,
        duration_ms: Option<u64>,
    },
    ToolCallError {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        error: String,
        duration_ms: Option<u64>,
    },
    ToolCallSkipped {
        timestamp: chrono::DateTime<chrono::Utc>,
        tool_call_id: String,
        tool_name: String,
        reason: String,
        duration_ms: Option<u64>,
    },

    // === Iteration (1) ===
    IterationComplete {
        timestamp: chrono::DateTime<chrono::Utc>,
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },

    // === Plugin (1) ===
    PluginEvent {
        timestamp: chrono::DateTime<chrono::Utc>,
        name: String,
        data: serde_json::Map<String, serde_json::Value>,
    },
}
```

- [ ] **Step 3: Add convenience constructors**

Add after the enum definition:

```rust
impl AgentStreamEvent {
    /// Create AgentStart event with current timestamp
    pub fn agent_start(input: String) -> Self {
        Self::AgentStart { timestamp: chrono::Utc::now(), input }
    }

    /// Create AgentComplete event with current timestamp
    pub fn agent_complete() -> Self {
        Self::AgentComplete { timestamp: chrono::Utc::now() }
    }

    /// Create AgentAborted event with current timestamp
    pub fn agent_aborted(reason: String) -> Self {
        Self::AgentAborted { timestamp: chrono::Utc::now(), reason }
    }

    /// Create LLMCallStart event with current timestamp and messages
    pub fn llm_call_start(iteration: u32, messages: Vec<Message>) -> Self {
        Self::LLMCallStart { timestamp: chrono::Utc::now(), iteration, messages }
    }

    /// Create LLMCallComplete event with current timestamp
    pub fn llm_call_complete(model: String, usage: Option<TokenUsage>) -> Self {
        Self::LLMCallComplete { timestamp: chrono::Utc::now(), model, usage }
    }

    /// Create LLMCallError event with current timestamp
    pub fn llm_call_error(error: String) -> Self {
        Self::LLMCallError { timestamp: chrono::Utc::now(), error }
    }

    /// Create ThinkingStart event with current timestamp
    pub fn thinking_start() -> Self {
        Self::ThinkingStart { timestamp: chrono::Utc::now() }
    }

    /// Create ThinkingDelta event with current timestamp
    pub fn thinking_delta(delta: String) -> Self {
        Self::ThinkingDelta { timestamp: chrono::Utc::now(), delta }
    }

    /// Create ThinkingComplete event with current timestamp
    pub fn thinking_complete(thinking: String) -> Self {
        Self::ThinkingComplete { timestamp: chrono::Utc::now(), thinking }
    }

    /// Create ContentStart event with current timestamp
    pub fn content_start() -> Self {
        Self::ContentStart { timestamp: chrono::Utc::now() }
    }

    /// Create ContentDelta event with current timestamp
    pub fn content_delta(delta: String) -> Self {
        Self::ContentDelta { timestamp: chrono::Utc::now(), delta }
    }

    /// Create ContentComplete event with current timestamp
    pub fn content_complete(content: String) -> Self {
        Self::ContentComplete { timestamp: chrono::Utc::now(), content }
    }

    /// Create ToolCallBegin event with current timestamp
    pub fn tool_call_begin(tool_call_id: String, tool_name: String, arguments: String) -> Self {
        Self::ToolCallBegin { timestamp: chrono::Utc::now(), tool_call_id, tool_name, arguments }
    }

    /// Create ToolCallComplete event with current timestamp
    pub fn tool_call_complete(tool_call_id: String, tool_name: String, result: String, duration_ms: Option<u64>) -> Self {
        Self::ToolCallComplete { timestamp: chrono::Utc::now(), tool_call_id, tool_name, result, duration_ms }
    }

    /// Create ToolCallError event with current timestamp
    pub fn tool_call_error(tool_call_id: String, tool_name: String, error: String, duration_ms: Option<u64>) -> Self {
        Self::ToolCallError { timestamp: chrono::Utc::now(), tool_call_id, tool_name, error, duration_ms }
    }

    /// Create ToolCallSkipped event with current timestamp
    pub fn tool_call_skipped(tool_call_id: String, tool_name: String, reason: String, duration_ms: Option<u64>) -> Self {
        Self::ToolCallSkipped { timestamp: chrono::Utc::now(), tool_call_id, tool_name, reason, duration_ms }
    }

    /// Create IterationComplete event with current timestamp
    pub fn iteration_complete(iteration: u32, tool_calls: Vec<ToolCall>, final_answer: Option<String>) -> Self {
        Self::IterationComplete { timestamp: chrono::Utc::now(), iteration, tool_calls, final_answer }
    }

    /// Create PluginEvent with current timestamp
    pub fn plugin_event(name: String, data: serde_json::Map<String, serde_json::Value>) -> Self {
        Self::PluginEvent { timestamp: chrono::Utc::now(), name, data }
    }
}
```

- [ ] **Step 4: Update existing unit tests in stream.rs**

Replace the test block (lines 124-214):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_stream_event_creation() {
        let event = AgentStreamEvent::agent_start("test".to_string());
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                assert_eq!(input, "test");
            }
            _ => panic!("Expected AgentStart"),
        }
    }

    #[test]
    fn test_agent_stream_event_tool_call() {
        let event = AgentStreamEvent::tool_call_begin(
            "call_123".to_string(),
            "get_weather".to_string(),
            r#"{"city": "Beijing"}"#.to_string(),
        );
        match event {
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                assert_eq!(tool_call_id, "call_123");
                assert_eq!(tool_name, "get_weather");
                assert_eq!(arguments, r#"{"city": "Beijing"}"#);
            }
            _ => panic!("Expected ToolCallBegin"),
        }
    }

    #[test]
    fn test_agent_stream_event_iteration_complete() {
        let event = AgentStreamEvent::iteration_complete(
            1,
            Vec::new(),
            Some("The answer".to_string()),
        );
        match event {
            AgentStreamEvent::IterationComplete {
                iteration,
                final_answer,
                ..
            } => {
                assert_eq!(iteration, 1);
                assert_eq!(final_answer, Some("The answer".to_string()));
            }
            _ => panic!("Expected IterationComplete"),
        }
    }

    #[test]
    fn test_agent_stream_event_aborted() {
        let event = AgentStreamEvent::agent_aborted("max iterations".to_string());
        match event {
            AgentStreamEvent::AgentAborted { reason, .. } => {
                assert_eq!(reason, "max iterations");
            }
            _ => panic!("Expected AgentAborted"),
        }
    }

    #[test]
    fn test_agent_stream_event_plugin_event() {
        use serde_json::Map;
        let mut data = Map::new();
        data.insert(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        let event = AgentStreamEvent::plugin_event("custom".to_string(), data);
        match event {
            AgentStreamEvent::PluginEvent { name, .. } => {
                assert_eq!(name, "custom");
            }
            _ => panic!("Expected PluginEvent"),
        }
    }
}
```

- [ ] **Step 5: Verify vol-llm-core compiles**

```bash
cargo check -p vol-llm-core
```

Expected: Compiles successfully with only the unit tests passing

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-core/src/stream.rs crates/vol-llm-core/Cargo.toml
git commit -m "feat: add timestamp to all AgentStreamEvent variants with helper constructors"
```

---

### Task 2: Add parent_id tracking to RunContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`

- [ ] **Step 1: Add last_message_id field to RunContext struct**

Add to `RunContext` struct (after line 148, after `error` field):

```rust
    /// Tracks the ID of the last message added, for auto-setting parent_id.
    pub last_message_id: Arc<std::sync::Mutex<Option<String>>>,
```

- [ ] **Step 2: Initialize last_message_id in RunContext::new()**

Add to the `let ctx = Self { ... }` initialization block (after `error`):

```rust
            last_message_id: Arc::new(std::sync::Mutex::new(None)),
```

- [ ] **Step 3: Add last_message_id to Clone impl**

Add to `impl Clone for RunContext` (after `error`):

```rust
            last_message_id: self.last_message_id.clone(),
```

- [ ] **Step 4: Update add_message to auto-set parent_id**

Replace the `add_message` method (lines 247-258):

```rust
    /// Add a message to the messages list and sync to session.
    /// Automatically sets parent_id to the previous message's ID.
    pub async fn add_message(&self, message: Message) -> Result<(), crate::AgentError> {
        // 1. Add to runtime messages array
        self.messages.write().await.push(message.clone());

        // 2. Create SessionMessage with auto-set parent_id
        let session_msg = {
            let mut last_id = self.last_message_id.lock().unwrap();
            let msg = SessionMessage::new(self.session_id.clone(), message.clone());
            let msg = last_id.as_ref().map(|id| msg.with_parent_id(id.clone())).unwrap_or(msg);
            let new_id = msg.id.clone();
            *last_id = Some(new_id);
            msg
        };

        // 3. Persist to session
        self.session.add_message(session_msg).await.map_err(|e| {
            crate::AgentError::SessionError(format!("Failed to save message: {}", e))
        })?;

        Ok(())
    }
```

- [ ] **Step 5: Add unit test for parent_id auto-tracking**

Add to the test module (after the last existing test):

```rust
    #[tokio::test]
    async fn test_add_message_auto_sets_parent_id() {
        let ctx = create_test_context();

        // Add first message
        ctx.add_message(Message::user("first")).await.unwrap();

        // Add second message
        ctx.add_message(Message::assistant("second")).await.unwrap();

        // Verify second message has parent_id set
        let session_msgs = ctx.session.get_messages(10).await.unwrap();
        assert_eq!(session_msgs.len(), 2);
        assert!(session_msgs[0].parent_id.is_none()); // first message, no parent
        assert!(session_msgs[1].parent_id.is_some()); // second message has parent
        assert_eq!(session_msgs[1].parent_id.as_ref().unwrap(), &session_msgs[0].id);
    }
```

- [ ] **Step 6: Verify vol-llm-agent compiles**

```bash
cargo check -p vol-llm-agent
```

Expected: May fail on agent.rs match patterns (Task 3 fixes those) — this task is done if run_context.rs itself compiles

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs
git commit -m "feat: RunContext tracks last_message_id for auto parent_id on SessionMessage"
```

---

### Task 3: Update agent.rs — consume_llm_stream returns model/usage, AgentComplete gets response

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Update consume_llm_stream signature and implementation**

Replace the function (lines 665-720):

```rust
/// Consume LLM stream response, emit streaming events, and accumulate into complete data.
///
/// Emits ThinkingStart/Delta/Complete and ContentStart/Delta/Complete events
/// as tokens arrive from the LLM.
///
/// Returns: (thinking, tool_calls, content, model, usage)
async fn consume_llm_stream(
    mut stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<(String, Vec<vol_llm_core::ToolCall>, String, String, Option<vol_llm_core::TokenUsage>), crate::AgentError> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();
    let mut model = String::new();
    let mut last_usage: Option<vol_llm_core::TokenUsage> = None;

    let mut thinking_started = false;
    let mut content_started = false;

    while let Some(result) = stream.recv().await {
        let event = result.map_err(|e| crate::AgentError::Llm(e))?;

        match event.data {
            StreamEventData::ResponseComplete { .. } => {
                // Model name may come from the response metadata
                // For now, use the LLM client's model if available
            }
            StreamEventData::ThinkingDelta { thinking: delta } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::thinking_start()).await;
                    thinking_started = true;
                }
                thinking.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::thinking_delta(delta)).await;
            }
            StreamEventData::ThinkingComplete { thinking: t } => {
                if !thinking_started {
                    run_ctx.emit(AgentStreamEvent::thinking_start()).await;
                    run_ctx.emit(AgentStreamEvent::thinking_delta(t.clone())).await;
                }
                thinking = t;
                run_ctx.emit(AgentStreamEvent::thinking_complete(thinking.clone())).await;
            }
            StreamEventData::ContentDelta { delta } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::content_start()).await;
                    content_started = true;
                }
                content.push_str(&delta);
                run_ctx.emit(AgentStreamEvent::content_delta(delta)).await;
            }
            StreamEventData::ContentComplete { content: c } => {
                if !content_started {
                    run_ctx.emit(AgentStreamEvent::content_start()).await;
                    run_ctx.emit(AgentStreamEvent::content_delta(c.clone())).await;
                }
                content = c;
                run_ctx.emit(AgentStreamEvent::content_complete(content.clone())).await;
            }
            StreamEventData::ToolCallComplete { tool_call } => {
                tool_calls.push(tool_call);
            }
            StreamEventData::UsageUpdate { usage } => {
                last_usage = Some(usage);
            }
            StreamEventData::ResponseStart { model: m } => {
                model = m;
            }
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content, model, last_usage))
}
```

- [ ] **Step 2: Update consume_llm_stream call site**

Replace lines 327-338:

```rust
                let (thinking, tool_calls, content, model, usage) = match consume_llm_stream(llm_stream, &run_ctx).await {
                    Ok(data) => data,
                    Err(e) => {
                        run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
                        run_ctx.emit(AgentStreamEvent::agent_aborted(format!("LLM stream failed: {}", e))).await;
                        return Err(e);
                    }
                };
```

- [ ] **Step 3: Update LLMCallStart emit to include messages**

Replace line 311:

```rust
                // Emit LLMCallStart with full message history
                let messages = run_ctx.get_messages().await;
                run_ctx.emit(AgentStreamEvent::llm_call_start(iteration, messages)).await;
```

And REMOVE the old `let messages = run_ctx.get_messages().await;` at line 271 since it's now emitted right before the LLM call.

- [ ] **Step 4: Update LLMCallComplete emit to use real model/usage**

Replace lines 346-349:

```rust
                // Emit LLMCallComplete with real model and usage
                run_ctx.emit(AgentStreamEvent::llm_call_complete(model.clone(), usage)).await;
```

- [ ] **Step 5: Remove debug/info prints from run() method**

Remove or comment out these specific lines:
- Line 263-265: `if config.verbose { info!("Iteration {}", iteration); }` — REMOVE (observability plugin handles via IterationComplete)
- Lines 274-304: The entire `if config.verbose { debug!("=== Conversation History ...") }` block — REMOVE (messages now carried by LLMCallStart event)
- Line 353: `debug!("Tool calls: {:?}", tool_calls);` — REMOVE (ToolCallBegin event carries this)
- Lines 372-375: `info!("Executing tool: {} with args: ...")` — REMOVE (ToolCallBegin event carries this)
- Line 504: `info!("Tool {} returned: {}", call.name, result.content);` — REMOVE (ToolCallComplete event carries this)

Also remove `use tracing::{debug, info};` from the imports at line 12, and keep only `use tracing::debug;` if debug is still needed for error paths. Actually, remove all `tracing` imports from agent.rs since all regular logging is now via events.

- [ ] **Step 6: Update ToolCallBegin emit to use helper**

Replace line 378-383:

```rust
                        let tool_event = AgentStreamEvent::tool_call_begin(
                            call.id.clone(),
                            call.name.clone(),
                            call.arguments.clone(),
                        );
                        run_ctx.emit(tool_event).await;
```

- [ ] **Step 7: Update ToolCallSkipped emit (HITL rejection path) with duration_ms None**

Replace lines 415-419:

```rust
                                                // Emit ToolCallSkipped
                                                run_ctx.emit(AgentStreamEvent::tool_call_skipped(
                                                    call.id.clone(),
                                                    call.name.clone(),
                                                    "User rejected".to_string(),
                                                    None,
                                                )).await;
```

- [ ] **Step 8: Update ToolCallSkipped emit (plugin skip path) with duration_ms None**

Replace lines 448-452:

```rust
                                run_ctx.emit(AgentStreamEvent::tool_call_skipped(
                                    call.id.clone(),
                                    call.name.clone(),
                                    "Plugin skipped".to_string(),
                                    None,
                                )).await;
```

- [ ] **Step 9: Update ToolCallError emit with duration_ms None**

Replace lines 475-479:

```rust
                                // Emit ToolCallError
                                run_ctx.emit(AgentStreamEvent::tool_call_error(
                                    call.id.clone(),
                                    call.name.clone(),
                                    e.to_string(),
                                    None,
                                )).await;
```

- [ ] **Step 10: Update ToolCallComplete emit with duration_ms None for now**

Replace lines 518-524:

```rust
                        // Emit ToolCallComplete
                        run_ctx
                            .emit(AgentStreamEvent::tool_call_complete(
                                call.id.clone(),
                                call.name.clone(),
                                result.content.clone(),
                                None,
                            ))
                            .await;
```

- [ ] **Step 11: Update IterationComplete emits with helper**

Replace lines 539-545:

```rust
                    // Emit IterationComplete
                    run_ctx
                        .emit(AgentStreamEvent::iteration_complete(
                            iteration,
                            tool_calls.clone(),
                            None,
                        ))
                        .await;
```

Replace lines 553-559:

```rust
                // No tool calls - we have final answer
                // Emit IterationComplete with final answer
                run_ctx
                    .emit(AgentStreamEvent::iteration_complete(
                        iteration,
                        Vec::new(),
                        Some(content.clone()),
                    ))
                    .await;
```

- [ ] **Step 12: Update AgentStart emit to use helper**

Replace lines 214-217:

```rust
            let start_event = AgentStreamEvent::agent_start(user_input.clone());
            run_ctx.emit(start_event.clone()).await;
```

- [ ] **Step 13: Update AgentAborted emits to use helper**

Replace lines 229-232:

```rust
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
```

Replace lines 252-256:

```rust
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
```

- [ ] **Step 14: Update LLMCallError emits to use helper**

Replace lines 316-318:

```rust
                        run_ctx.emit(AgentStreamEvent::llm_call_error(e.to_string())).await;
                        run_ctx.emit(AgentStreamEvent::agent_aborted(format!("LLM request failed: {}", e))).await;
```

- [ ] **Step 15: Update AgentComplete to carry response JSON**

Replace lines 572-574:

```rust
                // === Emit AgentComplete with response data ===
                let response = run_ctx.finalize();
                let response_json = serde_json::json!({
                    "content": response.content,
                    "iterations": response.iterations,
                    "tool_calls": response.tool_calls.iter().map(|t| serde_json::json!({
                        "tool_name": t.tool_name,
                        "arguments": t.arguments,
                        "result": t.result,
                        "iteration": t.iteration,
                        "success": t.success,
                    })).collect::<Vec<_>>(),
                    "run_id": response.run_id,
                    "session_id": response.session_id,
                });
                run_ctx.emit(AgentStreamEvent::AgentComplete {
                    timestamp: chrono::Utc::now(),
                    response: Some(response_json),
                }).await;

                return Ok(response);
```

- [ ] **Step 16: Update vol-llm-core/Cargo.toml for TokenUsage export**

Check that `TokenUsage` is exported from vol_llm_core. It should already be (used in stream.rs).

- [ ] **Step 17: Verify compilation**

```bash
cargo check -p vol-llm-agent
```

Expected: Compiles successfully

- [ ] **Step 18: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: agent.rs captures real model/usage, duration on tool events, response on AgentComplete, removes debug/info prints"
```

---

### Task 4: Update SessionListener for new event fields

**Files:**
- Modify: `crates/vol-session/src/listener.rs`

- [ ] **Step 1: Update should_record to handle new timestamp field**

The `should_record` function (lines 51-63) uses `..` patterns which will still match. But we need to update the specific event patterns:

```rust
    fn should_record(event: &AgentStreamEvent) -> bool {
        matches!(
            event,
            AgentStreamEvent::AgentStart { .. }
                | AgentStreamEvent::ThinkingComplete { .. }
                | AgentStreamEvent::ContentComplete { .. }
                | AgentStreamEvent::ToolCallBegin { .. }
                | AgentStreamEvent::ToolCallComplete { .. }
                | AgentStreamEvent::ToolCallError { .. }
                | AgentStreamEvent::ToolCallSkipped { .. }
                | AgentStreamEvent::IterationComplete { .. }
        )
    }
```

No change needed — `..` patterns already handle timestamp.

- [ ] **Step 2: Update event_to_message for new fields**

The match arms in `event_to_message` (lines 72-163) use destructuring patterns. Update each to include `..` or `timestamp`:

```rust
    fn event_to_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
        match event {
            // AgentStart -> User message
            AgentStreamEvent::AgentStart { input, .. } => Some(SessionMessage::new(
                self.session_id.clone(),
                vol_llm_core::Message::user(input.clone()),
            )),

            // ThinkingComplete -> Assistant message (thinking content)
            AgentStreamEvent::ThinkingComplete { thinking, .. } => Some(SessionMessage::new(
                self.session_id.clone(),
                vol_llm_core::Message::assistant(thinking.clone()),
            )),

            // ContentComplete -> Assistant message (content)
            AgentStreamEvent::ContentComplete { content, .. } => Some(SessionMessage::new(
                self.session_id.clone(),
                vol_llm_core::Message::assistant(content.clone()),
            )),

            // ToolCallBegin -> Assistant message with tool_calls
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                let tool_call = vol_llm_core::ToolCall {
                    id: tool_call_id.clone(),
                    name: tool_name.clone(),
                    arguments: arguments.clone(),
                    r#type: "function".to_string(),
                };
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::assistant_with_tools("", vec![tool_call]),
                ))
            }

            // ToolCallComplete -> Tool message with tool_call_id and result
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
                ..
            } => {
                let content = format!("Tool '{}' returned: {}", tool_name, result);
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::tool(content, tool_call_id.clone()),
                ))
            }

            // ToolCallError -> Tool message with error
            AgentStreamEvent::ToolCallError {
                tool_call_id,
                tool_name,
                error,
                ..
            } => {
                let content = format!("Tool '{}' error: {}", tool_name, error);
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::tool(content, tool_call_id.clone()),
                ))
            }

            // ToolCallSkipped -> Tool message with skip reason
            AgentStreamEvent::ToolCallSkipped {
                tool_call_id,
                tool_name,
                reason,
                ..
            } => {
                let content = format!("Tool '{}' skipped: {}", tool_name, reason);
                Some(SessionMessage::new(
                    self.session_id.clone(),
                    vol_llm_core::Message::tool(content, tool_call_id.clone()),
                ))
            }

            // IterationComplete with final_answer -> Assistant message (final answer)
            AgentStreamEvent::IterationComplete { final_answer, .. } => {
                final_answer.as_ref().map(|answer| {
                    SessionMessage::new(
                        self.session_id.clone(),
                        vol_llm_core::Message::assistant(answer.clone()),
                    )
                })
            }

            // Other events are not recorded
            _ => None,
        }
    }
```

- [ ] **Step 3: Update tests in listener.rs for new event construction**

All test functions that construct events need updating. Replace each event construction to use helper methods:

```rust
    #[tokio::test]
    async fn test_should_record_thinking_complete() {
        let event = AgentStreamEvent::thinking_complete("Let me think about this...".to_string());
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_tool_call_begin() {
        let event = AgentStreamEvent::tool_call_begin(
            "call_123".to_string(),
            "get_weather".to_string(),
            r#"{"city": "Beijing"}"#.to_string(),
        );
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_tool_call_complete() {
        let event = AgentStreamEvent::tool_call_complete(
            "call_123".to_string(),
            "get_weather".to_string(),
            "25°C".to_string(),
            None,
        );
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_iteration_complete_with_final_answer() {
        let event = AgentStreamEvent::iteration_complete(
            1,
            Vec::new(),
            Some("The weather is 25°C".to_string()),
        );
        assert!(SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_record_agent_start() {
        let event = AgentStreamEvent::agent_start("test".to_string());
        assert!(SessionListener::should_record(&event));
    }
```

And similarly update all remaining test event constructions using `..` patterns or helpers.

For events tested with `..` patterns like:
- `test_should_not_record_llm_call_complete` — update to `AgentStreamEvent::LLMCallComplete { model: "test".to_string(), usage: None, .. }` — but since we added timestamp, the existing pattern will break. Use the helper instead: create a temp event but the test is for `should_record` which returns false for LLMCallComplete, so just use `..Default::default()` is not available since no Default impl. Use a direct construction with `timestamp: chrono::Utc::now()`.

Actually, since `should_record` returns false for LLM events, we can use `_ => None` pattern. But the tests construct events. Let me use direct struct construction with timestamp:

```rust
    #[tokio::test]
    async fn test_should_not_record_llm_call_complete() {
        let event = AgentStreamEvent::LLMCallComplete {
            timestamp: chrono::Utc::now(),
            model: "test".to_string(),
            usage: None,
        };
        assert!(!SessionListener::should_record(&event));
    }

    #[tokio::test]
    async fn test_should_not_record_agent_complete() {
        let event = AgentStreamEvent::AgentComplete {
            timestamp: chrono::Utc::now(),
        };
        assert!(!SessionListener::should_record(&event));
    }
```

And update all other test event constructions similarly — use direct struct syntax with `timestamp: chrono::Utc::now()` for non-helper-tested events.

- [ ] **Step 4: Add chrono import**

Add `use chrono::Utc;` at the top of the test module, or use `chrono::Utc::now()` inline.

- [ ] **Step 5: Verify vol-session compiles**

```bash
cargo check -p vol-session
```

- [ ] **Step 6: Run vol-session tests**

```bash
cargo test -p vol-session
```

Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "feat: SessionListener updated for new AgentStreamEvent fields with timestamp"
```

---

### Task 5: Update TUI renderer for new event fields

**Files:**
- Modify: `crates/vol-llm-tui/src/render.rs`

- [ ] **Step 1: Update render_event match patterns**

Replace the entire `render_event` function:

```rust
pub fn render_event(event: &AgentStreamEvent) {
    match event {
        // Lifecycle
        AgentStreamEvent::AgentStart { input, .. } => {
            println!();
            print_colored(Color::Cyan, &format!(">>> {}\n", input));
        }

        AgentStreamEvent::AgentComplete { response, .. } => {
            println!();
            print_colored(Color::Green, "Done.\n");
        }

        AgentStreamEvent::AgentAborted { reason, .. } => {
            println!();
            print_colored(Color::Red, &format!("Aborted: {}\n", reason));
        }

        // LLM Call — meta events, not displayed to user
        AgentStreamEvent::LLMCallStart { .. } => {}
        AgentStreamEvent::LLMCallComplete { .. } => {}
        AgentStreamEvent::LLMCallError { .. } => {}

        // Thinking
        AgentStreamEvent::ThinkingStart { .. } => {
            print_colored(Color::Yellow, "\nThinking...\n");
        }

        AgentStreamEvent::ThinkingDelta { delta, .. } => {
            print_colored(Color::DarkGrey, delta);
        }

        AgentStreamEvent::ThinkingComplete { thinking, .. } => {
            if !thinking.is_empty() {
                print_colored(Color::DarkGrey, &format!("  [thinking complete]\n"));
            }
        }

        // Content
        AgentStreamEvent::ContentStart { .. } => {
            println!();
        }

        AgentStreamEvent::ContentDelta { delta, .. } => {
            print_colored(Color::White, delta);
        }

        AgentStreamEvent::ContentComplete { content, .. } => {
            if content.is_empty() {
                println!();
            }
        }

        // Tools
        AgentStreamEvent::ToolCallBegin { tool_name, arguments, .. } => {
            println!();
            print_colored(Color::Blue, &format!("[{}] ", tool_name));
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(arguments) {
                if let Some(cmd) = parsed.get("command").and_then(|v| v.as_str()) {
                    print_colored(Color::DarkGrey, &format!("Command: {}\n", cmd));
                } else if let Some(path) = parsed.get("path").and_then(|v| v.as_str()) {
                    print_colored(Color::DarkGrey, &format!("Path: {}\n", path));
                } else {
                    print_colored(Color::DarkGrey, &format!("Args: {}\n", arguments));
                }
            } else {
                print_colored(Color::DarkGrey, &format!("Args: {}\n", arguments));
            }
        }

        AgentStreamEvent::ToolCallComplete { tool_name, result, duration_ms, .. } => {
            print_colored(Color::Green, &format!("  ✓ {} completed", tool_name));
            if let Some(ms) = duration_ms {
                if *ms >= 1000 {
                    print_colored(Color::DarkGrey, &format!(" ({:.1}s)\n", *ms as f64 / 1000.0));
                } else {
                    print_colored(Color::DarkGrey, &format!(" ({}ms)\n", ms));
                }
            } else {
                println!();
            }
            // Truncate safely at character boundary, not byte boundary
            let chars: Vec<char> = result.chars().take(300).collect();
            let preview = if chars.len() < result.chars().count() {
                let truncated: String = chars.into_iter().collect();
                format!("{}...\n", truncated)
            } else {
                result.clone()
            };
            for line in preview.lines().take(10) {
                print_colored(Color::DarkGrey, &format!("    {}\n", line));
            }
        }

        AgentStreamEvent::ToolCallError { tool_name, error, duration_ms, .. } => {
            println!();
            print_colored(Color::Red, &format!("  ✗ {} failed", tool_name));
            if let Some(ms) = duration_ms {
                if *ms >= 1000 {
                    print_colored(Color::DarkGrey, &format!(" ({:.1}s)", *ms as f64 / 1000.0));
                } else {
                    print_colored(Color::DarkGrey, &format!(" ({}ms)", ms));
                }
            }
            print_colored(Color::Red, &format!(": {}\n", error));
        }

        AgentStreamEvent::ToolCallSkipped { tool_name, reason, duration_ms, .. } => {
            println!();
            print_colored(Color::DarkGrey, &format!("  ⊘ {} skipped", tool_name));
            if let Some(ms) = duration_ms {
                if *ms >= 1000 {
                    print_colored(Color::DarkGrey, &format!(" ({:.1}s)", *ms as f64 / 1000.0));
                } else {
                    print_colored(Color::DarkGrey, &format!(" ({}ms)", ms));
                }
            }
            print_colored(Color::DarkGrey, &format!(": {}\n", reason));
        }

        // Iteration
        AgentStreamEvent::IterationComplete { final_answer: Some(answer), .. } => {
            println!();
            print_colored(Color::Green, &format!("✓ {}\n", answer));
        }

        AgentStreamEvent::IterationComplete { iteration, .. } => {
            print_colored(Color::DarkGrey, &format!("\n[Iteration {} complete]\n", iteration));
        }

        // Plugin
        AgentStreamEvent::PluginEvent { .. } => {}
    }
    let _ = stdout().flush();
}
```

- [ ] **Step 2: Verify vol-llm-tui compiles**

```bash
cargo check -p vol-llm-tui
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/render.rs
git commit -m "feat: TUI renderer updated for new AgentStreamEvent fields, show duration on tool events"
```

---

### Task 6: Update ObservabilityPlugin for new event fields

**Files:**
- Modify: `crates/vol-llm-observability/src/plugin.rs`
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs` (the backward-compat re-export)

- [ ] **Step 1: Update ObservabilityPlugin::listen() match patterns**

In `crates/vol-llm-observability/src/plugin.rs`, update all match arms in `listen()` to use `..` patterns or new fields. Key changes:

For `AgentComplete` (line 58):
```rust
            AgentStreamEvent::AgentComplete { response, .. } => {
                // Log response data if available
                if let Some(resp) = response {
                    let mut data = serde_json::Map::new();
                    data.insert("response".to_string(), resp.clone());
                    self.log_event("AgentComplete", data);
                } else {
                    self.log_event("AgentComplete", serde_json::Map::new());
                }
            }
```

For `LLMCallComplete` (line 67):
```rust
            AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
```

For `ToolCallComplete` (line 94):
```rust
            AgentStreamEvent::ToolCallComplete { tool_call_id, tool_name, result, duration_ms, .. } => {
```

For `ToolCallError` and `ToolCallSkipped`: add `duration_ms` field extraction.

- [ ] **Step 2: Update event_name() helper**

Update all match arms to use `..` patterns:

```rust
fn event_name(event: &AgentStreamEvent) -> String {
    match event {
        AgentStreamEvent::AgentStart { .. } => "AgentStart".to_string(),
        AgentStreamEvent::AgentComplete { .. } => "AgentComplete".to_string(),
        // ... all others with .. patterns
    }
}
```

- [ ] **Step 3: Update intercept() method**

Update all match arms to use `..` patterns for timestamp, duration_ms, etc.

- [ ] **Step 4: Update all unit tests in plugin.rs**

Update all event constructions in tests to include `timestamp` or use helpers.

- [ ] **Step 5: Verify vol-llm-observability compiles**

```bash
cargo check -p vol-llm-observability
```

- [ ] **Step 6: Run vol-llm-observability tests**

```bash
cargo test -p vol-llm-observability
```

Expected: All tests pass

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-observability/src/plugin.rs
git commit -m "feat: ObservabilityPlugin updated for new event fields"
```

---

### Task 7: Update remaining crate-level match patterns

**Files:**
- Modify: `crates/vol-llm-agent/src/react/plugin_stream.rs`
- Modify: `crates/vol-llm-agent/src/plugins/rate_limiter.rs`
- Modify: `crates/vol-llm-agent/src/plugins/caching.rs`
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs`
- Modify: `crates/vol-llm-agents/src/coding/html_reporter.rs`

- [ ] **Step 1: Update plugin_stream.rs tests**

Replace `AgentStreamEvent::AgentComplete` at lines 147 and 167:

```rust
let _ = tx.send(Ok(AgentStreamEvent::agent_complete())).await;
```

- [ ] **Step 2: Update rate_limiter.rs**

Update match patterns to use `..`:

```rust
            AgentStreamEvent::AgentComplete { .. } => {
```

- [ ] **Step 3: Update caching.rs**

Update match pattern:

```rust
            AgentStreamEvent::AgentComplete { .. } => {
```

- [ ] **Step 4: Update observability/plugin.rs (vol-llm-agent re-export)**

Update all match patterns to use `..` for the new fields. This is the backward-compat module at `crates/vol-llm-agent/src/observability/plugin.rs`.

- [ ] **Step 5: Update html_reporter.rs**

Update all match patterns in the HTML reporter to use `..` patterns:

```rust
                AgentStreamEvent::LLMCallComplete { model, usage, .. } => {
                AgentStreamEvent::ToolCallComplete { tool_name, result, .. } => {
                AgentStreamEvent::AgentComplete { .. } => {
```

- [ ] **Step 6: Verify full workspace compiles**

```bash
cargo check --workspace
```

Expected: Compiles successfully

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/plugin_stream.rs crates/vol-llm-agent/src/plugins/rate_limiter.rs crates/vol-llm-agent/src/plugins/caching.rs crates/vol-llm-agent/src/observability/plugin.rs crates/vol-llm-agents/src/coding/html_reporter.rs
git commit -m "feat: update remaining match patterns for new AgentStreamEvent fields"
```

---

### Task 8: Update all test files

**Files:**
- All test files in:
  - `crates/vol-llm-agent/tests/`
  - `crates/vol-llm-agents/tests/`
  - `crates/vol-session/tests/`

- [ ] **Step 1: Update vol-llm-agent/tests/ files**

For each test file, find all `AgentStreamEvent::` constructions and update to include `timestamp` or use helper methods. Use `..Default::default()` is NOT available (no Default impl), so use either helpers or explicit `timestamp: chrono::Utc::now()`.

Files to update:
- `code_agent_simulation.rs`

- [ ] **Step 2: Update vol-llm-agents/tests/ files**

Files to update:
- `channelled_observer_integration.rs`
- `channelled_observer_unit.rs`
- `coding_deribit_ws_e2e.rs`
- `coding_e2e_test.rs`
- `coding_web_tools_integration.rs`
- `observer_integration.rs`
- `observer_plugin_unit.rs`

- [ ] **Step 3: Update vol-session/tests/ files**

Files to update:
- `integration_test.rs`

- [ ] **Step 4: Run all workspace tests**

```bash
cargo test --workspace --lib 2>&1 | head -100
```

- [ ] **Step 5: Fix any remaining compilation errors**

Search for any remaining patterns using:
```bash
cargo check --workspace 2>&1 | grep -i "missing field\`timestamp\|cannot construct"
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/tests/ crates/vol-llm-agents/tests/ crates/vol-session/tests/
git commit -m "test: update all test files for new AgentStreamEvent fields"
```

---

### Task 9: Final verification and cleanup

- [ ] **Step 1: Full workspace check**

```bash
cargo check --workspace
```

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

Expected: All tests pass

- [ ] **Step 3: Verify no remaining direct print statements in agent.rs run()**

```bash
grep -n 'info!\|debug!' crates/vol-llm-agent/src/react/agent.rs
```

Expected: Only `warn!` for error paths and shutdown logging remain

- [ ] **Step 4: Verify timestamps in events are working**

Run a quick manual test:
```bash
source .env && cargo run -p vol-llm-tui
```

Ask the agent a simple question, verify the run log JSONL has `timestamp` fields on all events.

- [ ] **Step 5: Commit any final changes**

```bash
git status
git add -A
git commit -m "chore: final cleanup for agent event observability enhancements"
```

---

## Summary of Changes

| Crate | Files | Purpose |
|-------|-------|---------|
| `vol-llm-core` | `stream.rs`, `Cargo.toml` | Timestamp on all 18 variants + helpers + chrono |
| `vol-llm-agent` | `react/agent.rs` | Real model/usage, duration, response, remove prints |
| `vol-llm-agent` | `react/run_context.rs` | `last_message_id` for auto parent_id |
| `vol-session` | `listener.rs` | Updated match patterns + tests |
| `vol-llm-tui` | `render.rs` | Updated match patterns, show duration |
| `vol-llm-observability` | `plugin.rs` | Handle new fields in listen/intercept |
| `vol-llm-agent` | `plugin_stream.rs`, `plugins/*`, `observability/*` | Updated match patterns |
| `vol-llm-agents` | `html_reporter.rs`, all test files | Updated match patterns |
