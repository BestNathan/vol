# Agent Run Event-Driven Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `ReActAgent::run()` to return `Result<(), AgentError>` and use only the event bus for all observation, removing the stream receiver pattern.

**Architecture:** Single event delivery through `RunContext::emit()`, no `tx` channel duplication. All events emitted unconditionally. Observability plugin records complete event data.

**Tech Stack:** Rust, tokio async runtime, serde_json for logging.

---

### Task 1: Update `run()` Signature and Remove Stream Channel

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:111-368`

- [ ] **Step 1: Update `run()` return type**

```rust
// In crates/vol-llm-agent/src/react/agent.rs, line 116:

// Before
pub async fn run(
    &self,
    user_input: &str,
    context: ToolContext,
) -> Result<AgentStreamReceiver, crate::AgentError> {

// After
pub async fn run(
    &self,
    user_input: &str,
    context: ToolContext,
) -> Result<(), crate::AgentError> {
```

- [ ] **Step 2: Remove channel creation**

```rust
// Remove lines 169-170:
// let (tx, rx) = mpsc::channel(100);

// The spawned task no longer needs to send events externally
```

- [ ] **Step 3: Remove all `tx.send()` calls in spawned task**

```rust
// Line 191 - Remove:
// let _ = tx.send(Err(crate::AgentError::Context(reason))).await;
// return;

// Line 210-212 - Remove:
// let _ = tx.send(Err(crate::AgentError::MaxIterationsReached {
//     max: config.max_iterations
// })).await;

// Line 233 - Remove:
// let _ = tx.send(Err(crate::AgentError::Llm(e))).await;

// Line 242 - Remove:
// let _ = tx.send(Err(e)).await;

// Line 251 - Remove:
// let _ = tx.send(Ok(thinking_event)).await;

// Line 288 - Remove:
// let _ = tx.send(Err(crate::AgentError::Context(reason))).await;

// Line 297-300 - Remove:
// let _ = tx.send(Err(crate::AgentError::ToolExecution {
//     tool: call.name.clone(),
//     error: e.to_string(),
// })).await;

// Line 308-311 - Remove:
// let _ = tx.send(Ok(AgentStreamEvent::ToolCallComplete {
//     tool_name: call.name.clone(),
//     result: result.content.clone(),
// })).await;

// Line 315 - Remove:
// let _ = tx.send(Err(crate::AgentError::from(e))).await;

// Line 324-328 - Remove:
// let _ = tx.send(Ok(AgentStreamEvent::IterationComplete {
//     iteration,
//     tool_calls: tool_calls.clone(),
//     final_answer: None,
// })).await;

// Line 336-340 - Remove:
// let _ = tx.send(Ok(AgentStreamEvent::IterationComplete {
//     iteration,
//     tool_calls: Vec::new(),
//     final_answer: Some(content.clone()),
// })).await;

// Line 344 - Remove:
// let _ = tx.send(Err(crate::AgentError::from(e))).await;

// Line 357-358 - Remove:
// let _ = tx.send(Ok(complete_event)).await;
```

- [ ] **Step 4: Update return statement**

```rust
// Line 367 - Before:
Ok(AgentStreamReceiver::new(rx))

// After:
Ok(())
```

- [ ] **Step 5: Remove unused imports**

```rust
// Line 5 - Remove if no longer used:
// use tokio::sync::mpsc;

// Line 6 - Remove if no longer used:
// use tokio::task::JoinHandle;
```

- [ ] **Step 6: Run cargo check**

```bash
cargo check -p vol-llm-agent
```

Expected: Success (may have warnings about unused imports)

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "refactor: change run() to return Result<(), AgentError> and remove tx channel"
```

---

### Task 2: Ensure All Events Are Emitted Unconditionally

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:247-252`

- [ ] **Step 1: Remove conditional check for ThinkingComplete**

```rust
// Lines 247-252 - Before:
// Emit ThinkingComplete if we have thinking content
if !thinking.is_empty() {
    let thinking_event = AgentStreamEvent::ThinkingComplete { thinking };
    run_ctx.emit(thinking_event.clone()).await;
    let _ = tx.send(Ok(thinking_event)).await;
}

// After:
// Emit ThinkingComplete unconditionally
let thinking_event = AgentStreamEvent::ThinkingComplete { thinking };
run_ctx.emit(thinking_event).await;
```

- [ ] **Step 2: Verify all other events are already unconditional**

Check these are all unconditional emit() calls:
- Line 179: `run_ctx.emit(start_event.clone()).await;` ✓
- Line 190: `run_ctx.emit(AgentStreamEvent::AgentAborted { ... }).await;` ✓
- Line 208: `run_ctx.emit(AgentStreamEvent::AgentAborted { ... }).await;` ✓

- [ ] **Step 3: Run cargo check**

```bash
cargo check -p vol-llm-agent
```

Expected: Success

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "fix: emit ThinkingComplete event unconditionally even when empty"
```

---

### Task 3: Update Observability Plugin to Log Complete Event Data

**Files:**
- Modify: `crates/vol-llm-agent/src/observability/plugin.rs:22-64`

- [ ] **Step 1: Update `create_log_entry()` to serialize all fields**

```rust
// Lines 22-64 - Replace the entire create_log_entry function:

fn create_log_entry(&self, event: &AgentStreamEvent, ctx: &RunContext) -> LogEntry {
    // Extract event type name and data separately for structured logging
    let (event_name, data) = match event {
        AgentStreamEvent::AgentStart { input } => {
            ("AgentStart", json!({ "input": input }))
        }
        AgentStreamEvent::ThinkingComplete { thinking } => {
            ("ThinkingComplete", json!({ "thinking": thinking }))
        }
        AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
            ("ToolCallBegin", json!({ 
                "tool_name": tool_name, 
                "arguments": arguments 
            }))
        }
        AgentStreamEvent::ToolCallComplete { tool_name, result } => {
            ("ToolCallComplete", json!({ 
                "tool_name": tool_name, 
                "result": result 
            }))
        }
        AgentStreamEvent::IterationComplete { iteration, tool_calls, final_answer } => {
            ("IterationComplete", json!({
                "iteration": iteration,
                "tool_calls": tool_calls,
                "final_answer": final_answer,
            }))
        }
        AgentStreamEvent::AgentComplete { response } => {
            ("AgentComplete", json!({ 
                "response": response 
            }))
        }
        AgentStreamEvent::AgentAborted { reason } => {
            ("AgentAborted", json!({ "reason": reason }))
        }
        AgentStreamEvent::PluginEvent { name, data } => {
            ("PluginEvent", json!({ "name": name, "data": data }))
        }
    };

    LogEntry {
        timestamp: Utc::now(),
        run_id: ctx.run_id.clone(),
        agent_id: self.logger.agent_id().to_string(),
        event: event_name.to_string(),
        data,
    }
}
```

**Key changes:**
- `ThinkingComplete`: was `thinking_length` → now `thinking` (full content)
- `IterationComplete`: was summary fields → now full `tool_calls` array and `final_answer`
- `AgentComplete`: was summary → now full `response` object

- [ ] **Step 2: Update LogEntry struct if needed**

```rust
// In crates/vol-llm-agent/src/observability/logger.rs:
// Ensure LogEntry can hold any JSON data in `data` field

pub struct LogEntry {
    pub timestamp: DateTime<Utc>,
    pub run_id: String,
    pub agent_id: String,
    pub event: String,  // event type name
    pub data: Value,    // full event data
}
```

- [ ] **Step 3: Run cargo check**

```bash
cargo check -p vol-llm-agent
```

Expected: Success

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/observability/plugin.rs
git commit -m "feat: log complete event data in observability plugin"
```

---

### Task 4: Update Observability Integration Test

**Files:**
- Modify: `crates/vol-llm-agent/tests/observability_integration.rs`

- [ ] **Step 1: Write failing test for new behavior**

```rust
// In crates/vol-llm-agent/tests/observability_integration.rs:

#[tokio::test]
async fn test_agent_run_emits_empty_thinking_event() {
    let temp_dir = TempDir::new().unwrap();
    let log_base = temp_dir.path().to_path_buf();
    
    let session_store = Arc::new(InMemorySessionStore::new());
    let message_store = Arc::new(InMemoryMessageStore::new());
    let session = Arc::new(Session::new(
        "test-session".to_string(),
        session_store.clone(),
        message_store.clone(),
    ));
    
    let agent = ReActAgent::builder()
        .with_llm(Arc::new(MockLlmWithoutThinking))  // LLM that returns empty thinking
        .with_session(session)
        .with_agent_id("test_agent".to_string())
        .with_log_base_path(log_base.clone())
        .with_observability_plugin()
        .build()
        .unwrap();
    
    let context = ToolContext::default();
    let result = agent.run("Test query", context).await;
    
    // Agent should complete successfully
    assert!(result.is_ok());
    
    // Verify ThinkingComplete event was logged even with empty thinking
    let agent_path = log_base.join("test_agent");
    let runs_path = agent_path.join("runs");
    
    // Find run log file
    let run_files: Vec<_> = std::fs::read_dir(&runs_path)
        .unwrap()
        .filter(|e| e.as_ref().unwrap().file_name().to_string_lossy().starts_with("run_"))
        .collect();
    
    assert!(!run_files.is_empty(), "Expected run log file");
    
    let run_log = std::fs::read_to_string(&run_files[0].as_ref().unwrap().path()).unwrap();
    
    // Should contain ThinkingComplete event (even if thinking was empty)
    assert!(run_log.contains(r#""event":"ThinkingComplete""#), 
            "ThinkingComplete should be emitted even when thinking is empty");
}
```

- [ ] **Step 2: Create mock LLM without thinking**

```rust
struct MockLlmWithoutThinking;

#[async_trait]
impl LLMClient for MockLlmWithoutThinking {
    fn provider(&self) -> LLMProvider {
        LLMProvider::Anthropic
    }

    fn model(&self) -> &str {
        "mock-no-thinking"
    }

    fn supported_params(&self) -> &[vol_llm_core::SupportedParam] {
        &[]
    }

    async fn converse(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::ConversationResponse> {
        unimplemented!("Use converse_stream instead")
    }

    async fn converse_stream(&self, _request: ConversationRequest) -> vol_llm_core::Result<vol_llm_core::stream::StreamReceiver> {
        use tokio::sync::mpsc;

        let (tx, rx) = mpsc::channel(10);
        tokio::spawn(async move {
            // Return content without thinking
            let _ = tx.send(Ok(StreamEvent {
                id: "event_1".to_string(),
                data: StreamEventData::ContentComplete {
                    content: "Mock response without thinking".to_string(),
                },
            })).await;
        });

        Ok(vol_llm_core::StreamReceiver::new(rx))
    }
}
```

- [ ] **Step 3: Run test**

```bash
cargo test -p vol-llm-agent --test observability_integration test_agent_run_emits_empty_thinking_event
```

Expected: PASS

- [ ] **Step 4: Update existing test to check full data logging**

```rust
// Modify existing test to verify complete data is logged:

#[tokio::test]
async fn test_observability_logs_full_event_data() {
    // ... setup ...
    
    // Verify run log contains full thinking content (not just length)
    assert!(run_content.contains(r#""thinking":"#), 
            "Should log full thinking content, not just length");
    
    // Verify full tool_calls array is logged
    assert!(run_content.contains(r#""tool_calls":"#) || run_content.contains(r#""tool_calls":[],"#),
            "Should log full tool_calls array");
    
    // Verify full response is logged in AgentComplete
    assert!(run_content.contains(r#""response":{"#),
            "Should log full response object");
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/tests/observability_integration.rs
git commit -m "test: add test for empty thinking event and verify full data logging"
```

---

### Task 5: Update `agent_observability_test.rs` Example

**Files:**
- Modify: `crates/vol-llm-agent/examples/agent_observability_test.rs:119-182`

- [ ] **Step 1: Remove stream consumption logic**

```rust
// Lines 119-182 - Before:
let context = ToolContext::default();
let stream_result = agent.run(query, context).await;

// Consume stream and display events
println!();
println!("═══════════════════════════════════════════════════════════");
println!("  Agent Execution Results");
println!("═══════════════════════════════════════════════════════════");
println!();

match stream_result {
    Ok(mut stream) => {
        let mut got_final_answer = false;
        while let Some(event_result) = stream.recv().await {
            match event_result {
                Ok(event) => {
                    match &event {
                        AgentStreamEvent::AgentStart { input } => {
                            println!("[AgentStart] Input: {}", input);
                        }
                        // ... all other events ...
                    }
                }
                Err(e) => {
                    eprintln!("[Error] {}", e);
                    break;
                }
            }
        }

        if !got_final_answer {
            println!();
            println!("Note: Agent completed without a final answer.");
        }
    }
    Err(e) => {
        eprintln!("Agent run failed: {:?}", e);
    }
}

// After:
let context = ToolContext::default();
let result = agent.run(query, context).await;

println!();
println!("═══════════════════════════════════════════════════════════");
println!("  Agent Execution Results");
println!("═══════════════════════════════════════════════════════════");
println!();

match result {
    Ok(()) => {
        println!("Agent completed successfully.");
        println!("Events were logged to observability plugin.");
    }
    Err(e) => {
        eprintln!("Agent run failed: {:?}", e);
    }
}
```

- [ ] **Step 2: Remove unused import**

```rust
// Remove if not used elsewhere:
// use vol_llm_agent::AgentStreamEvent;
```

- [ ] **Step 3: Update output to show log file locations**

```rust
// Keep the existing log file display logic (lines 184-200)
// This shows users where to find logged events
```

- [ ] **Step 4: Run example to verify**

```bash
export ANTHROPIC_AUTH_TOKEN=sk-xxx  # if testing with real API
cargo run --example agent_observability_test
```

Expected: Runs without errors, logs created

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/examples/agent_observability_test.rs
git commit -m "refactor: update example to use new run() signature without stream"
```

---

### Task 6: Update `agent_cli_approval.rs` HITL Example

**Files:**
- Modify: `crates/vol-llm-agent/examples/agent_cli_approval.rs:280-343`

- [ ] **Step 1: Read current example structure**

```bash
head -300 crates/vol-llm-agent/examples/agent_cli_approval.rs
```

- [ ] **Step 2: Update to not consume stream**

```rust
// Lines 280-343 - Similar pattern to Task 5:

// Before:
let context = ToolContext::default();
let stream_result: Result<vol_llm_agent::AgentStreamReceiver, vol_llm_agent::AgentError> = agent.run(query, context).await;

// Consume stream and display events
// ... match stream_result with while let Some(event) ...

// After:
let context = ToolContext::default();
let result = agent.run(query, context).await;

println!();
println!("═══════════════════════════════════════════════════════════");
println!("  Agent Execution Results");
println!("═══════════════════════════════════════════════════════════");
println!();

match result {
    Ok(()) => {
        println!("Agent completed successfully.");
        println!("Check observability logs for event details.");
    }
    Err(e) => {
        eprintln!("Agent run failed: {:?}", e);
    }
}
```

- [ ] **Step 3: Remove unused imports**

```rust
// Remove if not used elsewhere:
// use vol_llm_agent::AgentStreamEvent;
```

- [ ] **Step 4: Run example**

```bash
cargo run --example agent_cli_approval
```

Expected: Runs without errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/examples/agent_cli_approval.rs
git commit -m "refactor: update HITL example to use new run() signature"
```

---

### Task 7: Update Other Tests That Consume Stream

**Files:**
- Modify: `crates/vol-llm-agent/tests/react_agent_integration.rs`
- Modify: `crates/vol-llm-agent/tests/agent_llm_integration.rs`
- Modify: `crates/vol-llm-agent/tests/plugin_flow_test.rs`

- [ ] **Step 1: Search for stream-consuming tests**

```bash
grep -r "\.run(.*).await" crates/vol-llm-agent/tests/ | grep -v ".await;"
grep -r "stream.recv()" crates/vol-llm-agent/tests/
```

- [ ] **Step 2: Update each test file**

For each test that does:
```rust
let mut stream = agent.run(query, ctx).await.unwrap();
while let Some(event) = stream.recv().await {
    // assert on events
}
```

Replace with test plugin pattern:
```rust
let (tx, mut rx) = mpsc::channel(100);
let capture_plugin = EventCapturePlugin::new(tx);

let agent = ReActAgent::builder()
    .with_llm(mock_llm)
    .with_plugin(capture_plugin)
    .build().unwrap();

agent.run(query, ctx).await.unwrap();

// Collect events from plugin
let mut events = Vec::new();
while let Ok(event) = rx.try_recv() {
    events.push(event);
}
assert!(!events.is_empty());
```

- [ ] **Step 3: Create EventCapturePlugin helper**

```rust
// In tests/ module or a shared test utility file:

struct EventCapturePlugin {
    tx: mpsc::Sender<AgentStreamEvent>,
}

impl EventCapturePlugin {
    fn new(tx: mpsc::Sender<AgentStreamEvent>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl AgentPlugin for EventCapturePlugin {
    fn id(&self) -> PluginId { "event-capture".to_string() }
    fn priority(&self) -> u32 { 100 }
    
    async fn intercept(&self, _event: &AgentStreamEvent, _ctx: &RunContext) -> PluginDecision {
        PluginDecision::Continue
    }
    
    async fn listen(&self, event: &AgentStreamEvent, _ctx: &RunContext) {
        let _ = self.tx.send(event.clone()).await;
    }
}
```

- [ ] **Step 4: Run all tests**

```bash
cargo test -p vol-llm-agent --lib --tests
```

Expected: All tests pass

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/tests/
git commit -m "refactor: update tests to use EventCapturePlugin instead of stream"
```

---

### Task 8: Final Verification and Cleanup

**Files:**
- Various

- [ ] **Step 1: Run full test suite**

```bash
cargo test -p vol-llm-agent
```

Expected: All tests pass

- [ ] **Step 2: Verify workspace builds**

```bash
cargo build --release
```

Expected: Success

- [ ] **Step 3: Check for any remaining `tx.send()` usage**

```bash
grep -r "tx.send" crates/vol-llm-agent/src/react/
```

Expected: No matches in agent.rs

- [ ] **Step 4: Check for any remaining stream consumption**

```bash
grep -r "stream.recv()" crates/vol-llm-agent/examples/
```

Expected: No matches (or only in intentionally retained examples)

- [ ] **Step 5: Verify observability logs contain full data**

Run example and check:
```bash
cargo run --example agent_observability_test 2>&1 | head -50
cat logs/agents/*/runs/*.jsonl | head -10
```

Expected: JSONL contains full event data

- [ ] **Step 6: Commit final changes**

```bash
git add -A
git commit -m "chore: final cleanup for event-driven refactor"
```

---

## Spec Coverage Check

| Spec Requirement | Task |
|-----------------|------|
| `run()` returns `Result<(), AgentError>` | Task 1 |
| Remove `tx` channel | Task 1 |
| All events via `run_ctx.emit()` only | Task 1 |
| `ThinkingComplete` emitted unconditionally | Task 2 |
| Observability logs full event data | Task 3 |
| Tests updated | Task 4, 7 |
| Examples updated | Task 5, 6 |
| All tests pass | Task 8 |

## Type Consistency Check

- `run()` signature: `Result<(), AgentError>` - consistent
- `LogEntry.data`: `serde_json::Value` - holds any event data
- `AgentStreamEvent` variants: unchanged, only logging format changed

---

**Plan complete and saved to `docs/superpowers/plans/2026-04-10-agent-run-event-driven-refactor.md`. Two execution options:**

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
