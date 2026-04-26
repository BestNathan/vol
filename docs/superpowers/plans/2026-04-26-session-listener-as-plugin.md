# Remove SessionListener from Agent Run

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the broadcast-channel-based SessionListener and its spawn from agent.rs. SessionRecorderPlugin is added as a standalone `AgentPlugin` impl for future external use, but not registered anywhere.

**Architecture:** Delete `SessionListener` type and its broadcast-channel spawn in `agent.rs::run()`. Add `SessionRecorderPlugin` as a minimal `AgentPlugin` impl provided for future external registration, but not registered in this PR.

**Tech Stack:** Rust, async-trait, tokio, vol-session, vol-llm-core

---

### Task 1: Create SessionRecorderPlugin (standalone, not registered)

**Files:**
- Create: `crates/vol-session/src/recorder.rs`
- Modify: `crates/vol-session/src/lib.rs` (export new module)

- [ ] **Step 1: Write failing tests for SessionRecorderPlugin**

Create `crates/vol-session/src/recorder.rs` with the test module:

```rust
//! SessionRecorderPlugin — records agent events to session via AgentPlugin::listen().
//!
//! This is a standalone plugin provided for future external registration.
//! Not registered by default in agent.rs or CodingAgent.

#[cfg(test)]
mod tests {
    use super::*;
    use crate::InMemoryEntryStore;
    use crate::entry::{SessionEntryData, SessionEntryType, RUN_ID_KEY};
    use crate::SessionEntry;
    use vol_llm_core::{AgentStreamEvent, PluginContext};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn make_plugin() -> (SessionRecorderPlugin, PluginContext) {
        let entry_store: Arc<dyn crate::SessionEntryStore> = Arc::new(InMemoryEntryStore::new());
        let session = Session::new(entry_store.clone());
        let plugin = SessionRecorderPlugin::new(Arc::new(session), entry_store);

        let ctx = PluginContext {
            run_id: "test-run".to_string(),
            user_input: "test".to_string(),
            session_id: "test-session".to_string(),
            all_tool_calls: Arc::new(RwLock::new(vec![])),
            current_tool_calls: Arc::new(RwLock::new(vec![])),
            data: Arc::new(RwLock::new(HashMap::new())),
        };

        (plugin, ctx)
    }

    #[tokio::test]
    async fn test_plugin_id() {
        let (plugin, _) = make_plugin();
        assert_eq!(plugin.id(), "session_recorder");
    }

    #[tokio::test]
    async fn test_plugin_records_thinking_complete() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ThinkingComplete {
            timestamp: chrono::Utc::now(),
            thinking: "Let me think...".to_string(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&ctx.session_id).await.unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].r#type, SessionEntryType::Message);
    }

    #[tokio::test]
    async fn test_plugin_does_not_record_skipped_events() {
        let (plugin, ctx) = make_plugin();
        let event = AgentStreamEvent::ThinkingStart {
            timestamp: chrono::Utc::now(),
        };
        plugin.listen(&event, &ctx).await;

        let entries = plugin.entry_store.get_entries(&ctx.session_id).await.unwrap();
        assert!(entries.is_empty(), "ThinkingStart should not be recorded");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-session recorder -- --nocapture`
Expected: FAIL — `SessionRecorderPlugin` type not defined yet.

- [ ] **Step 3: Write the SessionRecorderPlugin implementation**

Add above the test module in `crates/vol-session/src/recorder.rs`:

```rust
use std::sync::Arc;
use async_trait::async_trait;
use vol_llm_core::{AgentPlugin, AgentStreamEvent, PluginContext};

use crate::entry::{SessionEntry, RUN_ID_KEY};
use crate::{Session, SessionEntryStore, SessionMessage};
use vol_llm_core::{Message, ToolCall};

/// Plugin that records key agent events to the session entry store.
///
/// Implements AgentPlugin::listen() to record events as SessionEntry.
/// Not registered by default — callers may register it externally.
pub struct SessionRecorderPlugin {
    session: Arc<Session>,
    entry_store: Arc<dyn SessionEntryStore>,
}

impl SessionRecorderPlugin {
    pub fn new(session: Arc<Session>, entry_store: Arc<dyn SessionEntryStore>) -> Self {
        Self { session, entry_store }
    }

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

    fn event_to_session_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
        match event {
            AgentStreamEvent::AgentStart { input, .. } => {
                Some(SessionMessage::new(self.session.id.clone(), Message::user(input.clone())))
            }
            AgentStreamEvent::ThinkingComplete { thinking, .. } => {
                Some(SessionMessage::new(self.session.id.clone(), Message::assistant(thinking.clone())))
            }
            AgentStreamEvent::ContentComplete { content, .. } => {
                Some(SessionMessage::new(self.session.id.clone(), Message::assistant(content.clone())))
            }
            AgentStreamEvent::ToolCallBegin {
                tool_call_id,
                tool_name,
                arguments,
                ..
            } => {
                let tool_call = ToolCall {
                    id: tool_call_id.clone(),
                    name: tool_name.clone(),
                    arguments: arguments.clone(),
                    r#type: "function".to_string(),
                };
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::assistant_with_tools("", vec![tool_call]),
                ))
            }
            AgentStreamEvent::ToolCallComplete {
                tool_call_id,
                tool_name,
                result,
                ..
            } => {
                let content = format!("Tool '{}' returned: {}", tool_name, result);
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::tool(content, tool_call_id.clone()),
                ))
            }
            AgentStreamEvent::ToolCallError {
                tool_call_id,
                tool_name,
                error,
                ..
            } => {
                let content = format!("Tool '{}' error: {}", tool_name, error);
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::tool(content, tool_call_id.clone()),
                ))
            }
            AgentStreamEvent::ToolCallSkipped {
                tool_call_id,
                tool_name,
                reason,
                ..
            } => {
                let content = format!("Tool '{}' skipped: {}", tool_name, reason);
                Some(SessionMessage::new(
                    self.session.id.clone(),
                    Message::tool(content, tool_call_id.clone()),
                ))
            }
            AgentStreamEvent::IterationComplete { final_answer, .. } => {
                final_answer.as_ref().map(|answer| {
                    SessionMessage::new(
                        self.session.id.clone(),
                        Message::assistant(answer.clone()),
                    )
                })
            }
            _ => None,
        }
    }
}

#[async_trait]
impl AgentPlugin for SessionRecorderPlugin {
    fn id(&self) -> vol_llm_core::PluginId {
        "session_recorder".to_string()
    }

    fn priority(&self) -> u32 {
        0
    }

    async fn listen(&self, event: &AgentStreamEvent, ctx: &PluginContext) {
        if !Self::should_record(event) {
            return;
        }

        let Some(msg) = self.event_to_session_message(event) else {
            return;
        };
        let msg = msg.with_metadata(RUN_ID_KEY, &ctx.run_id);
        let entry = SessionEntry::from_message(msg);

        if let Err(e) = self.entry_store.save(entry).await {
            tracing::error!("Failed to save session entry: {}", e);
        }
    }
}
```

- [ ] **Step 4: Export the new module from lib.rs**

In `crates/vol-session/src/lib.rs`, add:

```rust
pub mod recorder;
pub use recorder::SessionRecorderPlugin;
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test -p vol-session recorder -- --nocapture`
Expected: All 3 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-session/src/recorder.rs crates/vol-session/src/lib.rs
git commit -m "feat: add SessionRecorderPlugin implementing AgentPlugin::listen()"
```

---

### Task 2: Remove SessionListener from agent.rs

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Remove SessionListener spawn**

In `crates/vol-llm-agent/src/react/agent.rs`, remove the Phase 2.5 block (lines ~230-243):

```rust
// === Phase 2.5: Spawn SessionListener for session recording ===
use vol_session::{FileSessionEntryStore, SessionListener};

let mut session_listener = SessionListener::new(
    run_ctx.event_tx.subscribe(),
    Arc::new(FileSessionEntryStore::new(
        config.working_dir.join("logs/agents").join(&config.agent_id),
    )),
    session.id.clone(),
    run_id.clone(),
);
let session_listener_handle = tokio::spawn(async move {
    let _ = session_listener.run().await;
});
```

- [ ] **Step 2: Remove SessionListener handle wait**

Remove lines ~655-669:

```rust
// Wait for SessionListener to finish with timeout
let session_listener_result =
    tokio::time::timeout(std::time::Duration::from_secs(5), session_listener_handle).await;

match session_listener_result {
    Ok(Ok(())) => {}
    Ok(Err(join_err)) => {
        tracing::warn!(%join_err, "SessionListener task panicked");
    }
    Err(_timeout) => {
        tracing::warn!(
            "SessionListener task timeout after 5s - task may be hanging, proceeding anyway"
        );
    }
}
```

- [ ] **Step 3: Verify compiles**

Run: `cargo check -p vol-llm-agent`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "refactor: remove SessionListener broadcast spawn from agent run"
```

---

### Task 3: Remove listener.rs and update tests

**Files:**
- Delete: `crates/vol-session/src/listener.rs`
- Modify: `crates/vol-session/src/lib.rs` (remove listener module)
- Modify: `crates/vol-llm-agent/tests/session_recording_test.rs` (update to use plugin)
- Modify: `crates/vol-llm-agent/tests/agent_run_tests.rs` (update comment)

- [ ] **Step 1: Remove listener from lib.rs**

In `crates/vol-session/src/lib.rs`, remove:

```rust
pub mod listener;
```

and remove:

```rust
pub use listener::SessionListener;
```

- [ ] **Step 2: Delete listener.rs**

```bash
rm crates/vol-session/src/listener.rs
```

- [ ] **Step 3: Verify vol-session compiles**

Run: `cargo check -p vol-session`
Expected: PASS.

- [ ] **Step 4: Replace session_recording_test.rs**

Replace entire contents of `crates/vol-llm-agent/tests/session_recording_test.rs`:

```rust
//! Test session recording completeness via SessionRecorderPlugin.
//!
//! Run with: cargo test --test session_recording_test

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use vol_session::{InMemoryEntryStore, Session, SessionEntryStore, SessionRecorderPlugin};
use vol_llm_core::{
    AgentStreamEvent, PluginContext,
};

/// Test SessionRecorderPlugin records events directly
#[tokio::test]
async fn test_session_recorder_plugin_records_events() {
    let entry_store: Arc<dyn SessionEntryStore> = Arc::new(InMemoryEntryStore::new());
    let session = Session::new(entry_store.clone());
    let plugin = SessionRecorderPlugin::new(
        Arc::new(session.clone()),
        entry_store.clone(),
    );

    let plugin_ctx = PluginContext {
        run_id: "test-run".to_string(),
        user_input: "test".to_string(),
        session_id: session.id.clone(),
        all_tool_calls: Arc::new(RwLock::new(vec![])),
        current_tool_calls: Arc::new(RwLock::new(vec![])),
        data: Arc::new(RwLock::new(HashMap::new())),
    };

    // Send AgentStart event (user input)
    plugin.listen(&AgentStreamEvent::AgentStart {
        input: "User's first input".to_string(),
        timestamp: chrono::Utc::now(),
    }, &plugin_ctx).await;

    // Send ThinkingComplete event
    plugin.listen(&AgentStreamEvent::ThinkingComplete {
        thinking: "Let me think...".to_string(),
        timestamp: chrono::Utc::now(),
    }, &plugin_ctx).await;

    let entries = entry_store.get_entries(&session.id).await.unwrap();
    assert_eq!(entries.len(), 2, "Should have 2 recorded entries");

    // First entry: user input
    if let vol_session::SessionEntryData::Message { message } = &entries[0].data {
        assert_eq!(message.message.role, vol_llm_core::MessageRole::User);
        assert!(message.message.content.as_ref().unwrap().as_str().contains("User's first input"));
    } else {
        panic!("Expected message entry");
    }

    // Second entry: thinking
    if let vol_session::SessionEntryData::Message { message } = &entries[1].data {
        assert_eq!(message.message.role, vol_llm_core::MessageRole::Assistant);
        assert!(message.message.content.as_ref().unwrap().as_str().contains("Let me think"));
    } else {
        panic!("Expected message entry");
    }

    println!("Test passed: {} entries recorded", entries.len());
}
```

- [ ] **Step 5: Update agent_run_tests.rs comment**

In `crates/vol-llm-agent/tests/agent_run_tests.rs`, line ~239, change:

```rust
// Allow SessionListener to flush
```

to:

```rust
// Allow async session writes to complete
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-llm-agent --test session_recording_test -- --nocapture`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-session/src/lib.rs crates/vol-session/src/listener.rs crates/vol-llm-agent/tests/session_recording_test.rs crates/vol-llm-agent/tests/agent_run_tests.rs
git commit -m "refactor: remove SessionListener, update tests for SessionRecorderPlugin"
```

---

### Task 4: Full Workspace Verification

- [ ] **Step 1: Run full workspace test suite**

Run: `cargo test --workspace`
Expected: All tests pass.

- [ ] **Step 2: Run workspace check**

Run: `cargo check --workspace`
Expected: No errors or warnings.
