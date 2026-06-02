# AgentInput Unification Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `input: String` with `input: AgentInput` across agent-channel protocol, request types, dispatcher, and handler.

**Architecture:** Four targeted edits to existing files. `AgentPayload::Submit` drops `metadata` and `run_id` (now carried by `AgentInput`). `AgentRequest` drops `run_id` and `metadata`. Dispatcher fixes the broken `run_with_id` call by switching to `run_input(AgentInput)`. AgentHandler passes `AgentInput` through directly.

**Tech Stack:** Rust, serde, vol-llm-agent (already a direct dependency)

---

### Task 1: Update AgentPayload::Submit to use AgentInput

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs:1,156-175,439-447`

- [ ] **Step 1: Add AgentInput import**

Add `use vol_llm_agent::AgentInput;` after line 1:

```rust
use serde::{Deserialize, Serialize};
use vol_llm_agent::AgentInput;
```

- [ ] **Step 2: Update Submit variant to use AgentInput, drop metadata and run_id**

Replace `AgentPayload::Submit` variant (lines 439-447):

```rust
    Submit {
        input: AgentInput,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
    },
```

- [ ] **Step 3: Update the Submit decode path**

Replace lines 156-175 (the `Operation::Agent(AgentOperation::Submit)` match arm):

```rust
            Operation::Agent(AgentOperation::Submit) => {
                #[derive(Deserialize)]
                struct P {
                    input: AgentInput,
                    #[serde(default)]
                    target: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.submit"))?;
                Ok(Payload::Agent(AgentPayload::Submit {
                    input: p.input,
                    target: p.target,
                }))
            }
```

- [ ] **Step 4: Check compilation of just the protocol module**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | head -30`
Expected: errors about mismatched fields in `domain/agent.rs` and `request.rs` (will fix in subsequent tasks)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/agent_server_protocol.rs
git commit -m "refactor: use AgentInput in AgentPayload::Submit, drop duplicate fields"
```

---

### Task 2: Update AgentRequest to use AgentInput

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/request.rs`

- [ ] **Step 1: Replace AgentRequest struct and impl**

Replace entire contents of `request.rs`:

```rust
//! Agent request and result types.

use vol_llm_agent::{AgentInput, AgentResponse};

use crate::error::ChannelError;

/// External request to an agent.
#[derive(Debug, Clone)]
pub struct AgentRequest {
    /// Target agent ID for routing.
    pub target_id: String,
    /// Sender agent ID (Some for agent-to-agent calls).
    pub sender_id: Option<String>,
    /// Input to pass to ReActAgent::run_input().
    pub input: AgentInput,
}

impl AgentRequest {
    /// Create a new request.
    pub fn new(target_id: impl Into<String>, input: AgentInput) -> Self {
        Self {
            target_id: target_id.into(),
            sender_id: None,
            input,
        }
    }
}

/// Result delivered to the sender after execution.
#[derive(Debug)]
pub struct RunResult {
    /// Run ID for one inference run.
    pub run_id: String,
    /// Target agent that processed this.
    pub target_id: String,
    /// The agent response or error.
    pub response: Result<AgentResponse, ChannelError>,
}

/// Internal wrapper for a queued request awaiting execution.
pub(crate) struct PendingRequest {
    pub request: AgentRequest,
    pub tx: tokio::sync::oneshot::Sender<RunResult>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_request_new_stores_input() {
        let input = AgentInput::text("hello");
        let request = AgentRequest::new("agent_a", input);

        assert_eq!(request.target_id, "agent_a");
        assert_eq!(request.input.display_text(), "hello");
    }

    #[test]
    fn agent_request_with_run_id_on_input() {
        let input = AgentInput::text("hello").with_run_id("run_123");
        let request = AgentRequest::new("agent_a", input);

        assert_eq!(request.input.run_id.as_deref(), Some("run_123"));
        assert_eq!(request.input.display_text(), "hello");
    }
}
```

- [ ] **Step 2: Check compilation**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | head -40`
Expected: errors in `dispatcher.rs` and `domain/agent.rs` (still referencing old fields)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/request.rs
git commit -m "refactor: use AgentInput in AgentRequest, drop duplicate run_id/metadata"
```

---

### Task 3: Update AgentDispatcher to use run_input

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/dispatcher.rs`

- [ ] **Step 1: Fix the run call in the background loop (line 136)**

Replace `agent.run_with_id(&pending.request.input, pending.request.run_id.clone())` with:

```rust
                agent.run_input(pending.request.input.clone()),
```

- [ ] **Step 2: Update cancel() to match run_id from AgentInput**

Replace the cancel method body (lines 74-83) with matching via `input.run_id`:

```rust
    pub async fn cancel(&self, run_id: &str) -> bool {
        let mut queue = self.state.queue.lock().await;

        if let Some(pos) = queue
            .iter()
            .position(|p| p.request.input.run_id.as_deref() == Some(run_id))
        {
            let pending = queue.remove(pos).unwrap();
            drop(pending.tx);
            true
        } else {
            false
        }
    }
```

- [ ] **Step 3: Update log line run_id reference (line 142)**

Replace `tracing::error!(run_id = %pending.request.run_id, "agent run timed out after 5 minutes");` with:

```rust
                    tracing::error!(
                        run_id = ?pending.request.input.run_id,
                        "agent run timed out after 5 minutes"
                    );
```

- [ ] **Step 4: Update RunResult construction in run_loop (lines 150-154)**

Replace the `RunResult` construction with run_id read from input:

```rust
            let run_id = pending
                .request
                .input
                .run_id
                .clone()
                .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());

            let run_result = RunResult {
                run_id: run_id.clone(),
                target_id: pending.request.target_id.clone(),
                response: result.map_err(|e| ChannelError::AgentError(e.to_string())),
            };
```

- [ ] **Step 5: Update test module — fix AgentRequest construction and cancel test**

Replace the entire test module (lines 163-252) with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::request::AgentRequest;
    use vol_llm_agent::AgentInput;

    #[tokio::test]
    async fn test_state_queue_push_pop() {
        let state = DispatcherState::new();

        let (tx1, _) = oneshot::channel();
        let req1 = AgentRequest::new("agent_a", AgentInput::text("hello"));
        state.queue.lock().await.push_back(PendingRequest {
            request: req1,
            tx: tx1,
        });

        assert_eq!(state.queue.lock().await.len(), 1);

        let (tx2, _) = oneshot::channel();
        let req2 = AgentRequest::new("agent_b", AgentInput::text("world"));
        state.queue.lock().await.push_back(PendingRequest {
            request: req2,
            tx: tx2,
        });

        assert_eq!(state.queue.lock().await.len(), 2);

        let first = state.queue.lock().await.pop_front();
        assert!(first.is_some());
        assert_eq!(first.unwrap().request.input.display_text(), "hello");

        assert_eq!(state.queue.lock().await.len(), 1);
    }

    #[tokio::test]
    async fn test_cancel_removes_from_queue() {
        let state = DispatcherState::new();

        let (tx1, _rx1) = oneshot::channel::<RunResult>();
        let req1 = AgentRequest::new(
            "agent_a",
            AgentInput::text("hello").with_run_id("run-1"),
        );
        state.queue.lock().await.push_back(PendingRequest {
            request: req1,
            tx: tx1,
        });

        let (tx2, _rx2) = oneshot::channel::<RunResult>();
        let req2 = AgentRequest::new(
            "agent_a",
            AgentInput::text("world").with_run_id("run-2"),
        );
        state.queue.lock().await.push_back(PendingRequest {
            request: req2,
            tx: tx2,
        });

        assert_eq!(state.queue.lock().await.len(), 2);

        // Cancel run-1
        let mut queue = state.queue.lock().await;
        let pos = queue
            .iter()
            .position(|p| p.request.input.run_id.as_deref() == Some("run-1"));
        assert!(pos.is_some());
        let pending = queue.remove(pos.unwrap()).unwrap();
        drop(pending.tx);

        assert_eq!(queue.len(), 1);
        assert_eq!(
            queue[0].request.input.run_id.as_deref(),
            Some("run-2")
        );
    }

    #[tokio::test]
    async fn test_cancel_nonexistent_returns_false() {
        let state = DispatcherState::new();

        let found = state
            .queue
            .lock()
            .await
            .iter()
            .any(|p| p.request.input.run_id.as_deref() == Some("nonexistent"));
        assert!(!found);
    }

    #[tokio::test]
    async fn test_busy_state() {
        let state = Arc::new(DispatcherState::new());

        // Not busy initially
        assert!(state.busy.try_lock().is_ok());

        // When someone holds the lock, try_lock fails
        let _permit = state.busy.lock().await;
        assert!(state.busy.try_lock().is_err());
    }
}
```

- [ ] **Step 6: Check compilation**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | head -40`
Expected: only errors in `domain/agent.rs` remain

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent-channel/src/dispatcher.rs
git commit -m "fix: switch dispatcher from run_with_id to run_input(AgentInput)"
```

---

### Task 4: Update AgentHandler to pass AgentInput through

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`

- [ ] **Step 1: Replace the Submit match arm (lines 56-116)**

Replace the Submit handler to pass `AgentInput` directly into `AgentRequest`, and read `run_id` from the input:

```rust
            (
                AgentOperation::Submit,
                Payload::Agent(AgentPayload::Submit {
                    input,
                    target,
                }),
            ) => {
                let target_id = {
                    let holders = self.holders.lock().unwrap();
                    target
                        .filter(|t| holders.contains_key(t))
                        .or_else(|| holders.keys().next().cloned())
                        .unwrap_or_else(|| "agent".to_string())
                };

                let request = AgentRequest::new(&target_id, input);
                let run_id = request.input.run_id.clone()
                    .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
                let run_id_clone = run_id.clone();

                match self.router.send(&target_id, request).await {
                    Ok(rx) => {
                        let router = self.router.clone();
                        tokio::spawn(async move {
                            Self::process_run_result(rx, &run_id_clone, &router).await;
                        });

                        Ok(vec![
                            AgentServerMessage::new_ack(
                                message.message_id.clone(),
                                Operation::Agent(AgentOperation::Submit),
                                Payload::Agent(AgentPayload::SubmitAck {
                                    run_id: run_id.clone(),
                                    accepted: true,
                                }),
                            ),
                            AgentServerMessage::new_result(
                                message.message_id,
                                Operation::Agent(AgentOperation::Submit),
                                Payload::Agent(AgentPayload::SubmitResult {
                                    run_id: run_id.clone(),
                                    response: serde_json::json!({"run_id": run_id}),
                                }),
                            ),
                        ])
                    }
                    Err(e) => Ok(vec![AgentServerMessage::new_error(
                        message.message_id,
                        Operation::Agent(AgentOperation::Submit),
                        crate::agent_server_protocol::ErrorPayload {
                            code: "agent_submit_failed".to_string(),
                            message: e.to_string(),
                            detail: None,
                            terminal: true,
                        },
                    )]),
                }
            }
```

- [ ] **Step 2: Check full compilation**

Run: `cargo check -p vol-llm-agent-channel 2>&1`
Expected: clean compilation (no errors)

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1`
Expected: all tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/agent.rs
git commit -m "refactor: pass AgentInput through AgentHandler submit flow"
```

---

### Task 5: Verify E2E and broader workspace

**Files:**
- Verify: `crates/vol-llm-agent-channel/` (all)
- Verify: `crates/vol-agent-manager/` (depends on channel)
- Verify: `crates/vol-llm-ui/` (depends on channel through manager)

- [ ] **Step 1: Run full channel test suite**

Run: `cargo test -p vol-llm-agent-channel 2>&1`
Expected: all tests pass

- [ ] **Step 2: Check agent-manager compiles**

Run: `cargo check -p vol-agent-manager 2>&1 | head -20`
Expected: clean or only pre-existing warnings

- [ ] **Step 3: Run any E2E JSON-RPC tests**

Run: `cargo test -p vol-llm-agent-channel --test '*' 2>&1`
Expected: any integration/E2E tests pass or skip gracefully if no test binary exists

- [ ] **Step 4: Commit any straggling test fixes**

If E2E tests needed fixes:
```bash
git add -A && git commit -m "test: fix E2E tests for AgentInput unification"
```
Otherwise no commit needed.

---

### Task 6: Wiki ingest

**Files:**
- Update: `docs/wiki/` (via wiki-ingest skill)

- [ ] **Step 1: Invoke wiki-ingest skill**

Use the `wiki-ingest` skill to update the project wiki, documenting that:
- `AgentPayload::Submit` now carries `AgentInput` instead of `String`
- `AgentRequest` now carries `AgentInput` instead of `String`
- `AgentDispatcher` calls `run_input(AgentInput)` instead of `run_with_id(&str, run_id)`
- Backwards compatibility: `AgentInput` deserializes from plain string

- [ ] **Step 2: Commit wiki update**

```bash
git add docs/wiki/ && git commit -m "docs: update wiki for AgentInput unification"
```
