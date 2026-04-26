# Max Iteration Continuation via HITL Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When the coding agent reaches max iterations, instead of immediately exiting, use the existing HITL approval channel to ask the user if they want to continue. If yes, reset the iteration counter to 0 and keep running.

**Architecture:** Reuse the existing approval channel (`RunContext.approval_tx` / `run_cli_approval_loop`). The agent sends a continuation request through the same channel using a `__continue__` sentinel value. The CLI handler detects this sentinel and shows a different prompt. If approved, `RunContext.reset_iteration()` is called and the loop continues.

**Tech Stack:** Rust, tokio, crossterm (TUI rendering)

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-core/src/stream.rs` | Modify | Add `MaxIterationsReached` and `IterationContinued` event variants + constructors |
| `crates/vol-llm-agent/src/react/run_context.rs` | Modify | Add `request_continue_approval()` and `reset_iteration()` methods |
| `crates/vol-llm-agent/src/react/agent.rs` | Modify | Replace hard-stop at max iterations with HITL continuation flow |
| `crates/vol-llm-agent/src/react/hitl.rs` | Modify | Update `run_cli_approval_loop` to detect `__continue__` sentinel |
| `crates/vol-llm-agent/src/react/mod.rs` | Modify | Export new types if needed |
| `crates/vol-llm-tui/src/render.rs` | Modify | Handle and render the 2 new events |

---

### Task 1: Add new AgentStreamEvent variants

**Files:**
- Modify: `crates/vol-llm-core/src/stream.rs:66-166` (enum definition), `crates/vol-llm-core/src/stream.rs:168-226` (constructors)

- [ ] **Step 1: Add two new event variants to the enum**

In `crates/vol-llm-core/src/stream.rs`, add these two variants after `AgentAborted` (around line 78):

```rust
    AgentAborted {
        timestamp: chrono::DateTime<chrono::Utc>,
        reason: String,
    },

    /// Emitted when max iterations is reached, before asking for continuation.
    MaxIterationsReached {
        timestamp: chrono::DateTime<chrono::Utc>,
        current_iteration: u32,
        max_iterations: u32,
    },

    /// Emitted when user approves continuation and iteration counter resets.
    IterationContinued {
        timestamp: chrono::DateTime<chrono::Utc>,
        from_iteration: u32,
    },
```

- [ ] **Step 2: Add constructor functions**

In the `impl AgentStreamEvent` block (around line 180, after `agent_aborted`), add:

```rust
    pub fn agent_aborted(reason: String) -> Self {
        Self::AgentAborted { timestamp: chrono::Utc::now(), reason }
    }
    pub fn max_iterations_reached(current_iteration: u32, max_iterations: u32) -> Self {
        Self::MaxIterationsReached { timestamp: chrono::Utc::now(), current_iteration, max_iterations }
    }
    pub fn iteration_continued(from_iteration: u32) -> Self {
        Self::IterationContinued { timestamp: chrono::Utc::now(), from_iteration }
    }
```

- [ ] **Step 3: Add unit tests for the new events**

In the `#[cfg(test)] mod tests` block (around line 228), add:

```rust
    #[test]
    fn test_agent_stream_event_max_iterations() {
        let event = AgentStreamEvent::max_iterations_reached(5, 10);
        match event {
            AgentStreamEvent::MaxIterationsReached {
                current_iteration,
                max_iterations,
                ..
            } => {
                assert_eq!(current_iteration, 5);
                assert_eq!(max_iterations, 10);
            }
            _ => panic!("Expected MaxIterationsReached"),
        }
    }

    #[test]
    fn test_agent_stream_event_iteration_continued() {
        let event = AgentStreamEvent::iteration_continued(10);
        match event {
            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                assert_eq!(from_iteration, 10);
            }
            _ => panic!("Expected IterationContinued"),
        }
    }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-core`
Expected: Compiles successfully

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-core/src/stream.rs
git commit -m "feat: add MaxIterationsReached and IterationContinued stream events"
```

---

### Task 2: Add continuation methods to RunContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:69-151` (struct), `crates/vol-llm-agent/src/react/run_context.rs:364-388` (after `request_tool_approval`)

- [ ] **Step 1: Add a constant for the continue sentinel**

At the top of the file (after the imports, before line 21), add:

```rust
/// Sentinel value for ApprovalRequest.tool_name to indicate a continuation request.
pub(crate) const CONTINUE_SENTINEL: &str = "__continue__";
```

- [ ] **Step 2: Add `request_continue_approval()` method**

After `request_tool_approval()` (around line 388), add:

```rust
    /// Request human approval to continue after max iterations.
    /// Blocks until a HITL handler responds.
    ///
    /// Returns true if the user approved continuation, false to stop.
    pub async fn request_continue_approval(
        &self,
        current_iteration: u32,
        max_iterations: u32,
    ) -> Result<bool, crate::AgentError> {
        let (tx, rx) = oneshot::channel();
        let request = ApprovalRequest {
            tool_name: CONTINUE_SENTINEL.to_string(),
            reason: format!(
                "Agent reached max iterations ({}/{})",
                current_iteration, max_iterations
            ),
            metadata: serde_json::json!({
                "current_iteration": current_iteration,
                "max_iterations": max_iterations,
            }),
        };
        self.approval_tx
            .send((request, tx))
            .await
            .map_err(|e| crate::AgentError::Context(format!("Approval channel error: {}", e)))?;

        let response = rx.await
            .map_err(|e| crate::AgentError::Context(format!("Approval response error: {}", e)))?;

        Ok(response.approved)
    }
```

- [ ] **Step 3: Add `reset_iteration()` method**

After `next_iteration()` (around line 247), add:

```rust
    /// Reset the iteration counter to 0 (called after user approves continuation).
    pub fn reset_iteration(&self) {
        self.iteration.store(0, std::sync::atomic::Ordering::SeqCst);
    }
```

- [ ] **Step 4: Add unit tests for the new methods**

In the `#[cfg(test)] mod tests` block (around line 482), add:

```rust
    #[tokio::test]
    async fn test_run_context_reset_iteration() {
        let ctx = create_test_context();
        ctx.next_iteration();
        ctx.next_iteration();
        assert_eq!(ctx.current_iteration(), 2);
        ctx.reset_iteration();
        assert_eq!(ctx.current_iteration(), 0);
    }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-agent`
Expected: Compiles successfully (new events from Task 1 are used below, but the struct/method changes compile independently)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs
git commit -m "feat: add continuation approval request and iteration reset to RunContext"
```

---

### Task 3: Modify agent.rs max iteration check to use HITL continuation

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:239-254` (the loop start + max iteration check)

- [ ] **Step 1: Replace the hard-stop with continuation flow**

In `crates/vol-llm-agent/src/react/agent.rs`, replace lines 244-254 (the `if iteration > config.max_iterations` block):

**Current code:**
```rust
                if iteration > config.max_iterations {
                    // Emit max iterations reached event
                    let reason = format!("Max iterations ({}) reached", config.max_iterations);
                    run_ctx
                        .emit(AgentStreamEvent::agent_aborted(reason.clone()))
                        .await;

                    return Err(crate::AgentError::MaxIterationsReached {
                        max: config.max_iterations,
                    });
                }
```

**New code:**
```rust
                if iteration > config.max_iterations {
                    // Emit max iterations reached event
                    run_ctx.emit(AgentStreamEvent::max_iterations_reached(
                        iteration,
                        config.max_iterations,
                    )).await;

                    // Ask user via HITL approval channel whether to continue
                    let should_continue = match run_ctx.request_continue_approval(
                        iteration,
                        config.max_iterations,
                    ).await {
                        Ok(true) => true,
                        Ok(false) => {
                            // User declined — abort
                            let reason = format!("Max iterations ({}) reached, user declined to continue", config.max_iterations);
                            run_ctx.emit(AgentStreamEvent::agent_aborted(reason.clone())).await;
                            return Err(crate::AgentError::MaxIterationsReached {
                                max: config.max_iterations,
                            });
                        }
                        Err(e) => {
                            // No HITL handler connected or error — abort
                            let reason = format!("Max iterations ({}) reached, continuation request failed: {}", config.max_iterations, e);
                            run_ctx.emit(AgentStreamEvent::agent_aborted(reason.clone())).await;
                            return Err(crate::AgentError::MaxIterationsReached {
                                max: config.max_iterations,
                            });
                        }
                    };

                    // User approved — reset and continue
                    run_ctx.emit(AgentStreamEvent::iteration_continued(iteration)).await;
                    run_ctx.reset_iteration();
                    continue;
                }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: agent loop asks for continuation at max iterations via HITL"
```

---

### Task 4: Update CLI approval handler to detect continuation requests

**Files:**
- Modify: `crates/vol-llm-agent/src/react/hitl.rs:325-365` (`run_cli_approval_loop`)

- [ ] **Step 1: Update `run_cli_approval_loop` to detect `__continue__` sentinel**

Replace the `run_cli_approval_loop` function body. The key change: when `request.tool_name == "__continue__"`, show a different prompt.

**New implementation (replace the entire function starting at line 325):**

```rust
pub fn run_cli_approval_loop(
    rx: tokio::sync::mpsc::Receiver<(
        super::run_context::ApprovalRequest,
        tokio::sync::oneshot::Sender<super::run_context::ApprovalResponse>,
    )>,
) {
    use std::io::{self, BufRead, Write};

    const CONTINUE_SENTINEL: &str = "__continue__";

    std::thread::spawn(move || {
        let stdin = io::stdin();
        let mut rx = rx; // Make mutable

        while let Some((request, tx)) = rx.blocking_recv() {
            // Check if this is a continuation request
            let is_continue = request.tool_name == CONTINUE_SENTINEL;

            // Display request with different format for continuation
            println!();
            if is_continue {
                println!("\u{26a0} Agent reached max iterations");
                println!("  {}", request.reason);
                println!("  Continue? (iteration counter will reset) [y/n] > ");
            } else {
                println!("\u{26a0} Approval required:");
                println!("  Tool: {}", request.tool_name);
                println!("  Reason: {}", request.reason);
                print!("  Approve? [y/n] > ");
            }
            let _ = io::stdout().flush();

            // Read response
            let mut line = String::new();
            let approved = match stdin.lock().read_line(&mut line) {
                Ok(_) => {
                    let trimmed = line.trim().to_lowercase();
                    trimmed == "y" || trimmed == "yes" || trimmed.is_empty()
                }
                Err(_) => false,
            };

            let response = if approved {
                super::run_context::ApprovalResponse::approved()
            } else {
                super::run_context::ApprovalResponse::rejected("User rejected".into())
            };

            let _ = tx.send(response);
        }
    });
}
```

Note: `\u{26a0}` is the ⚠ character (same as current code).

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/hitl.rs
git commit -m "feat: CLI approval handler detects and formats continuation requests"
```

---

### Task 5: Update TUI renderer for new events

**Files:**
- Modify: `crates/vol-llm-tui/src/render.rs:48-188` (the `render` method match arms)

- [ ] **Step 1: Add match arms for the 2 new events**

In `crates/vol-llm-tui/src/render.rs`, in the `render()` method's `match event` block, add two new arms. Insert them after `AgentAborted` (around line 82) — before the LLM Call suppression block:

```rust
            AgentStreamEvent::AgentAborted { reason, .. } => {
                println!();
                print_colored(Color::Red, &format!("Aborted: {}\n", reason));
            }

            AgentStreamEvent::MaxIterationsReached { current_iteration, max_iterations, .. } => {
                println!();
                print_colored(Color::Yellow, &format!(
                    "\u26a0 Max iterations reached ({}/{}) — waiting for user decision...\n",
                    current_iteration, max_iterations,
                ));
            }

            AgentStreamEvent::IterationContinued { from_iteration, .. } => {
                println!();
                print_colored(Color::Green, &format!(
                    ">>> Continuing from iteration {} (counter reset to 0)\n",
                    from_iteration,
                ));
            }

            // LLM Call — meta events, not displayed
```

Note: `\u26a0` is the ⚠ warning character.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-tui`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tui/src/render.rs
git commit -m "feat: TUI renders MaxIterationsReached and IterationContinued events"
```

---

### Task 6: Full workspace verification

**Files:** No changes — just verification

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: All crates compile

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace --lib`
Expected: All tests pass

- [ ] **Step 3: Commit** (if any test fixes were needed)

No changes needed if all passes.

---

## Summary of Changes

| File | Change | Lines Changed |
|------|--------|---------------|
| `crates/vol-llm-core/src/stream.rs` | Add 2 event variants + 2 constructors + 2 tests | +30 |
| `crates/vol-llm-agent/src/react/run_context.rs` | Add sentinel const + 2 methods + 1 test | +45 |
| `crates/vol-llm-agent/src/react/agent.rs` | Replace 11-line hard-stop with 35-line continuation flow | ~24 net |
| `crates/vol-llm-agent/src/react/hitl.rs` | Update CLI handler to detect sentinel | ~15 |
| `crates/vol-llm-tui/src/render.rs` | Add 2 new match arms for rendering | +16 |

**Key behavioral changes:**
1. Agent no longer exits on max iterations — asks user via HITL
2. User enters `y` → iteration counter resets to 0, agent continues
3. User enters `n` → agent aborts as before (MaxIterationsReached error)
4. No HITL handler (test/non-interactive) → falls back to abort (fail-safe)
5. TUI shows yellow warning when max iterations hit, green info when continuing
6. Conversation history is preserved across continuation (same ReActAgent instance, same session)
