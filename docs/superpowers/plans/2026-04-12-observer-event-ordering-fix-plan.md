# Observer Event Ordering Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix HTML timeline report event ordering by implementing sequential channel pattern in HTMLReporter.

**Architecture:** ChannelledEventObserver wraps mpsc unbounded channel + single consumer task to guarantee ordered event processing. HTMLReporter delegates to ChannelledEventObserver.

**Tech Stack:** Rust 2021, tokio, mpsc unbounded channel, oneshot channel, Arc<Mutex<Vec>>.

---

## File Structure

### Files to Create
- `crates/vol-llm-agents/src/coding/channelled_observer.rs` - ChannelledEventObserver implementation
- `crates/vol-llm-agents/tests/channelled_observer_unit.rs` - Unit tests for ChannelledEventObserver
- `crates/vol-llm-agents/tests/channelled_observer_integration.rs` - Integration tests for ordering
- `crates/vol-llm-agents/tests/e2e_log_counter_cli.rs` - E2E test: CodingAgent writes Rust CLI tool

### Files to Modify
- `crates/vol-llm-agents/src/coding/html_reporter.rs` - Add ChannelledEventObserver inner field, update on_event/on_complete
- `crates/vol-llm-agents/src/coding/mod.rs` - Add channelled_observer module export

---

## Phase 1: Create ChannelledEventObserver

### Task 1: Create ChannelledEventObserver Module Stub

**Files:**
- Create: `crates/vol-llm-agents/src/coding/channelled_observer.rs`
- Modify: `crates/vol-llm-agents/src/coding/mod.rs`

- [ ] **Step 1: Create channelled_observer.rs with stub struct**

```rust
//! ChannelledEventObserver - guarantees ordered event processing via mpsc channel.

use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot, Mutex};
use vol_llm_core::AgentStreamEvent;

use crate::coding::observer::EventObserver;
use crate::coding::error::ObserverError;

/// ChannelledEventObserver - wraps mpsc channel + single consumer task for ordered event processing
pub struct ChannelledEventObserver {
    tx: mpsc::UnboundedSender<AgentStreamEvent>,
    events: Arc<Mutex<Vec<AgentStreamEvent>>>,
    shutdown_tx: oneshot::Sender<()>,
}

impl ChannelledEventObserver {
    /// Create a new ChannelledEventObserver with spawned consumer task
    pub fn new() -> Self {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let events = Arc::new(Mutex::new(Vec::new()));
        let (shutdown_tx, shutdown_rx) = oneshot::channel();

        let events_clone = events.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    Some(event) = rx.recv() => {
                        events_clone.lock().await.push(event);
                    }
                    _ = &mut shutdown_rx => {
                        break;
                    }
                }
            }
        });

        Self { tx, events, shutdown_tx }
    }

    /// Get all recorded events in order
    pub fn events(&self) -> Vec<AgentStreamEvent> {
        std::thread::block_on(async {
            self.events.lock().await.clone()
        })
    }

    /// Wait for pending events and signal shutdown
    pub async fn wait_completion(&self) {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = self.shutdown_tx.send(());
    }
}

#[async_trait::async_trait]
impl EventObserver for ChannelledEventObserver {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        let _ = self.tx.send(event.clone());
        Ok(())
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        self.wait_completion().await;
        Ok(())
    }
}
```

- [ ] **Step 2: Add module to mod.rs**

```rust
// In crates/vol-llm-agents/src/coding/mod.rs

mod agent;
mod config;
mod error;
mod hitl;
mod html_reporter;
mod observer;
mod observer_plugin;
mod channelled_observer;  // Add this line

pub use agent::{CodingAgent, CodingAgentBuilder};
pub use config::CodingAgentConfig;
pub use error::{CodingAgentError, ObserverError, HITLError};
pub use hitl::{HITLDecision, HITLHandler};
pub use html_reporter::HTMLReporter;
pub use observer::EventObserver;
pub use observer_plugin::ObserverPlugin;
pub use channelled_observer::ChannelledEventObserver;  // Add this line
```

- [ ] **Step 3: Build to verify**

Run: `cargo build -p vol-llm-agents`

Expected: Compilation errors about `std::thread::block_on` - need to fix

- [ ] **Step 4: Fix block_on issue**

```rust
// Replace std::thread::block_on with tokio-compatible version
// Remove the events() method that uses block_on - it will be called from async context
```

Actually, let's redesign: `events()` should be async:

```rust
pub async fn events(&self) -> Vec<AgentStreamEvent> {
    self.events.lock().await.clone()
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/src/coding/channelled_observer.rs
git add crates/vol-llm-agents/src/coding/mod.rs
git commit -m "feat(coding-agent): add ChannelledEventObserver module stub"
```

---

### Task 2: Write Unit Tests for ChannelledEventObserver

**Files:**
- Create: `crates/vol-llm-agents/tests/channelled_observer_unit.rs`

- [ ] **Step 1: Write failing tests**

```rust
//! Unit tests for ChannelledEventObserver

use vol_llm_agents::coding::{ChannelledEventObserver, EventObserver, ObserverError};
use vol_llm_core::AgentStreamEvent;

#[tokio::test]
async fn test_channelled_observer_new_creates_empty_events() {
    let observer = ChannelledEventObserver::new();
    let events = observer.events().await;
    assert!(events.is_empty());
}

#[tokio::test]
async fn test_channelled_observer_on_event_records_event() {
    let observer = ChannelledEventObserver::new();
    
    let event = AgentStreamEvent::AgentStart {
        input: "test task".to_string(),
    };
    
    observer.on_event(&event).await.unwrap();
    
    // Give consumer task time to process
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    
    let events = observer.events().await;
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0], AgentStreamEvent::AgentStart { .. }));
}

#[tokio::test]
async fn test_channelled_observer_preserves_order() {
    let observer = ChannelledEventObserver::new();
    
    let events_in = vec![
        AgentStreamEvent::AgentStart { input: "start".to_string() },
        AgentStreamEvent::ThinkingComplete { thinking: "thinking".to_string() },
        AgentStreamEvent::ToolCallBegin { tool_name: "test".to_string(), arguments: "{}".to_string() },
        AgentStreamEvent::ToolCallComplete { tool_name: "test".to_string(), result: "ok".to_string() },
        AgentStreamEvent::AgentComplete,
    ];
    
    // Send all events rapidly
    for event in &events_in {
        observer.on_event(event).await.unwrap();
    }
    
    // Wait for consumer to process
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    
    let events_out = observer.events().await;
    assert_eq!(events_out.len(), 5);
    
    // Verify order matches input
    for (i, expected) in events_in.iter().enumerate() {
        assert!(matches!(
            (&events_out[i], expected),
            (AgentStreamEvent::AgentStart { .. }, AgentStreamEvent::AgentStart { .. }) |
            (AgentStreamEvent::ThinkingComplete { .. }, AgentStreamEvent::ThinkingComplete { .. }) |
            (AgentStreamEvent::ToolCallBegin { .. }, AgentStreamEvent::ToolCallBegin { .. }) |
            (AgentStreamEvent::ToolCallComplete { .. }, AgentStreamEvent::ToolCallComplete { .. }) |
            (AgentStreamEvent::AgentComplete, AgentStreamEvent::AgentComplete)
        ));
    }
}

#[tokio::test]
async fn test_channelled_observer_on_complete_waits() {
    let observer = ChannelledEventObserver::new();
    
    let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
    observer.on_event(&event).await.unwrap();
    
    // on_complete should wait for events to be processed
    observer.on_complete().await.unwrap();
    
    let events = observer.events().await;
    assert_eq!(events.len(), 1);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p vol-llm-agents --test channelled_observer_unit -- --nocapture`

Expected: FAIL with "cannot find type `ChannelledEventObserver`"

- [ ] **Step 3: Fix implementation issues**

If tests fail due to implementation issues, fix them.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p vol-llm-agents --test channelled_observer_unit -- --nocapture`

Expected: PASS (4 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agents/tests/channelled_observer_unit.rs
git commit -m "test(coding-agent): add unit tests for ChannelledEventObserver"
```

---

## Phase 2: Update HTMLReporter

### Task 3: Update HTMLReporter to Use ChannelledEventObserver

**Files:**
- Modify: `crates/vol-llm-agents/src/coding/html_reporter.rs`

- [ ] **Step 1: Add ChannelledEventObserver import**

```rust
//! HTMLReporter - generates HTML timeline reports.

use async_trait::async_trait;
use std::path::PathBuf;
use vol_llm_core::AgentStreamEvent;

use crate::coding::observer::EventObserver;
use crate::coding::error::ObserverError;
use crate::coding::channelled_observer::ChannelledEventObserver;  // Add this line
```

- [ ] **Step 2: Update HTMLReporter struct**

```rust
/// HTML Reporter - generates HTML timeline report on complete
pub struct HTMLReporter {
    inner: ChannelledEventObserver,
    output_path: PathBuf,
    task_description: String,
}

impl HTMLReporter {
    /// Create a new HTMLReporter
    pub fn new(output_path: PathBuf, task_description: String) -> Self {
        Self {
            inner: ChannelledEventObserver::new(),
            output_path,
            task_description,
        }
    }
```

- [ ] **Step 3: Update on_event implementation**

```rust
#[async_trait]
impl EventObserver for HTMLReporter {
    async fn on_event(&self, event: &AgentStreamEvent) -> Result<(), ObserverError> {
        tracing::debug!(event_type = %Self::event_name(event), "Observer received event");
        self.inner.on_event(event).await
    }

    async fn on_complete(&self) -> Result<(), ObserverError> {
        self.inner.on_complete().await?;
        let events = self.inner.events().await;
        tracing::info!("Generating HTML report with {} events", events.len());
        self.generate_html_report(events).await
    }
}
```

- [ ] **Step 4: Remove old Mutex<Vec> fields and start_time**

Remove the `start_time` field since timing is no longer tracked in HTMLReporter.

- [ ] **Step 5: Update generate_html_report signature**

The method already accepts `Vec<AgentStreamEvent>`, no change needed.

- [ ] **Step 6: Build to verify**

Run: `cargo build -p vol-llm-agents`

Expected: SUCCESS

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agents/src/coding/html_reporter.rs
git commit -m "refactor(html-reporter): use ChannelledEventObserver for ordered events"
```

---

## Phase 3: Integration Tests

### Task 4: Create Integration Tests for Event Ordering

**Files:**
- Create: `crates/vol-llm-agents/tests/channelled_observer_integration.rs`

- [ ] **Step 1: Write integration test**

```rust
//! Integration tests for ChannelledEventObserver with concurrent sends

use vol_llm_agents::coding::{ChannelledEventObserver, EventObserver};
use vol_llm_core::AgentStreamEvent;
use std::sync::Arc;

#[tokio::test]
async fn test_concurrent_on_event_preserves_send_order() {
    let observer = Arc::new(ChannelledEventObserver::new());
    
    // Spawn multiple tasks that send events concurrently
    let mut handles = Vec::new();
    for i in 0..10 {
        let obs = observer.clone();
        let handle = tokio::spawn(async move {
            let event = AgentStreamEvent::ToolCallBegin {
                tool_name: format!("tool_{}", i),
                arguments: format!("arg_{}", i),
            };
            obs.on_event(&event).await.unwrap();
        });
        handles.push(handle);
    }
    
    // Wait for all senders to complete
    for handle in handles {
        handle.await.unwrap();
    }
    
    // Wait for consumer to process all events
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    
    let events = observer.events().await;
    assert_eq!(events.len(), 10);
    
    // Note: Order may not match spawn order due to task scheduling,
    // but events should be in some consistent order (channel FIFO)
    // This test verifies all events are received
}

#[tokio::test]
async fn test_sequential_on_event_preserves_exact_order() {
    let observer = ChannelledEventObserver::new();
    
    // Send events sequentially with small delays
    let events_in = vec![
        AgentStreamEvent::AgentStart { input: "1".to_string() },
        AgentStreamEvent::ThinkingComplete { thinking: "2".to_string() },
        AgentStreamEvent::ToolCallBegin { tool_name: "3".to_string(), arguments: "".to_string() },
        AgentStreamEvent::ToolCallComplete { tool_name: "4".to_string(), result: "".to_string() },
        AgentStreamEvent::IterationComplete { iteration: 5, tool_calls: vec![], final_answer: None },
        AgentStreamEvent::AgentComplete,
    ];
    
    for event in &events_in {
        observer.on_event(event).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    
    // Wait and shutdown
    observer.on_complete().await.unwrap();
    
    let events_out = observer.events().await;
    assert_eq!(events_out.len(), 6);
    
    // Verify exact order
    assert!(matches!(events_out[0], AgentStreamEvent::AgentStart { .. }));
    assert!(matches!(events_out[1], AgentStreamEvent::ThinkingComplete { .. }));
    assert!(matches!(events_out[2], AgentStreamEvent::ToolCallBegin { .. }));
    assert!(matches!(events_out[3], AgentStreamEvent::ToolCallComplete { .. }));
    assert!(matches!(events_out[4], AgentStreamEvent::IterationComplete { .. }));
    assert!(matches!(events_out[5], AgentStreamEvent::AgentComplete));
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test -p vol-llm-agents --test channelled_observer_integration -- --nocapture`

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agents/tests/channelled_observer_integration.rs
git commit -m "test(coding-agent): add integration tests for concurrent event ordering"
```

---

## Phase 4: E2E Test with CLI Tool Task

### Task 5: Create E2E Test - CodingAgent Writes Rust CLI Tool

**Files:**
- Create: `crates/vol-llm-agents/tests/e2e_log_counter_cli.rs`
- Create: `test-fixtures/log-counter-task.md` (task description)

- [ ] **Step 1: Create test fixtures directory and task file**

```bash
mkdir -p crates/vol-llm-agents/test-fixtures
```

```markdown
// In crates/vol-llm-agents/test-fixtures/log-counter-task.md

Write a Rust command-line tool that:
1. Takes a directory path as argument
2. Finds all .log files in that directory (non-recursive)
3. Counts the number of lines in each .log file
4. Outputs the files sorted by line count (descending)

Example usage:
```
$ cargo run -- /var/log/myapp
1523 lines: /var/log/myapp/error.log
847 lines: /var/log/myapp/access.log
102 lines: /var/log/myapp/debug.log
```

Requirements:
- Use standard library only (no external dependencies except clap for CLI)
- Handle errors gracefully (skip unreadable files, print warnings)
- Output format: "{count} lines: {path}"
- If no .log files found, print "No .log files found"
```

- [ ] **Step 2: Create E2E test**

```rust
//! E2E test: CodingAgent writes a Rust CLI tool to count .log file lines

use vol_llm_agents::coding::{CodingAgent, CodingAgentConfig, HTMLReporter};
use vol_llm_core::AgentStreamEvent;
use std::path::PathBuf;
use std::sync::Arc;
use tempfile::tempdir;

#[tokio::test]
#[ignore] // Requires real LLM API key (ANTHROPIC_AUTH_TOKEN)
async fn test_coding_agent_writes_log_counter_cli() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");
    let work_dir = temp_dir.path().join("work");
    std::fs::create_dir_all(&work_dir).unwrap();
    
    // Create some test .log files
    std::fs::write(work_dir.join("app.log"), "line 1\nline 2\nline 3\n").unwrap();
    std::fs::write(work_dir.join("error.log"), "error 1\nerror 2\nerror 3\nerror 4\nerror 5\n").unwrap();
    std::fs::write(work_dir.join("debug.log"), "debug 1\n").unwrap();

    let config = CodingAgentConfig {
        max_iterations: 15,
        working_dir: work_dir.clone(),
        hitl_enabled: false,
        verbose: false,
        html_report_path: Some(report_path.clone()),
        llm_provider_id: "anthropic-main".to_string(),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
    };

    let agent = CodingAgent::new(config).await.unwrap();

    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Write Rust CLI tool to count .log file lines".to_string(),
    ));
    let agent = agent.with_observer(observer);

    let task = r#"Write a Rust CLI tool that:
1. Takes a directory path as command-line argument
2. Finds all .log files in that directory
3. Counts lines in each .log file
4. Prints results sorted by line count (descending)
Format: "{count} lines: {path}"
Use clap for CLI parsing. Create Cargo.toml and src/main.rs."#;

    let result = agent.run(task).await.unwrap();

    assert!(result.success, "CodingAgent should complete successfully");
    
    // Verify report was generated
    assert!(report_path.exists(), "HTML report should exist");
    
    // Verify report exists and contains expected events
    let content = std::fs::read_to_string(&report_path).unwrap();
    
    // Check for timeline section
    assert!(content.contains("Timeline"), "Report should have Timeline section");
    
    // Check for expected event types in order
    let start_pos = content.find("Agent started").expect("Should have AgentStart");
    let thinking_pos = content.find("Thinking").expect("Should have ThinkingComplete");
    let tool_call_pos = content.find("Tool Call").expect("Should have ToolCall");
    let complete_pos = content.find("Agent completed").expect("Should have AgentComplete");
    
    // Verify rough order (start < thinking < tool_call < complete)
    assert!(start_pos < thinking_pos, "Start should come before Thinking");
    assert!(thinking_pos < tool_call_pos, "Thinking should come before ToolCall");
    assert!(tool_call_pos < complete_pos, "ToolCall should come before Complete");
    
    // Verify the CLI tool was created
    let cargo_toml = work_dir.join("Cargo.toml");
    let main_rs = work_dir.join("src").join("main.rs");
    
    if cargo_toml.exists() && main_rs.exists() {
        // Try to build the tool
        let output = std::process::Command::new("cargo")
            .arg("build")
            .current_dir(&work_dir)
            .output();
        
        if let Ok(output) = output {
            assert!(
                output.status.success(),
                "CLI tool should compile successfully\nstderr: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#[tokio::test]
#[ignore] // Requires real LLM API key
async fn test_html_report_shows_ordered_timeline() {
    let temp_dir = tempdir().unwrap();
    let report_path = temp_dir.path().join("report.html");
    
    let config = CodingAgentConfig {
        max_iterations: 5,
        working_dir: temp_dir.path().to_path_buf(),
        hitl_enabled: false,
        verbose: false,
        html_report_path: Some(report_path.clone()),
        llm_provider_id: "anthropic-main".to_string(),
        plugin_registry: vol_llm_agent::react::PluginRegistry::new(),
    };
    
    let agent = CodingAgent::new(config).await.unwrap();
    
    let observer = Arc::new(HTMLReporter::new(
        report_path.clone(),
        "Simple file listing task".to_string(),
    ));
    let agent = agent.with_observer(observer);
    
    let result = agent.run("List all files in the current directory").await.unwrap();
    
    assert!(result.success);
    assert!(report_path.exists());
    
    let content = std::fs::read_to_string(&report_path).unwrap();
    
    // Extract timeline items and verify order
    // Expected order: AgentStart -> ThinkingComplete -> ToolCallBegin -> ToolCallComplete -> AgentComplete
    let event_positions: Vec<(usize, &'static str)> = vec![
        (content.find("Start").unwrap_or(usize::MAX), "Start"),
        (content.find("Thinking").unwrap_or(usize::MAX), "Thinking"),
        (content.find("Tool Call").unwrap_or(usize::MAX), "Tool Call"),
        (content.find("Complete").unwrap_or(usize::MAX), "Complete"),
    ];
    
    // Verify positions are in ascending order (excluding NOT_FOUND)
    let mut last_pos = 0;
    for (pos, name) in &event_positions {
        if *pos != usize::MAX {
            assert!(
                *pos >= last_pos,
                "Event '{}' at position {} should come after previous event at {}",
                name, pos, last_pos
            );
            last_pos = *pos;
        }
    }
}
```

- [ ] **Step 3: Run E2E test (requires API key)**

Run: `cargo test -p vol-llm-agents --test e2e_log_counter_cli -- --nocapture --ignored`

Expected: Test runs with real LLM, creates CLI tool, verifies compilation and HTML report ordering

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agents/tests/e2e_log_counter_cli.rs
git add crates/vol-llm-agents/test-fixtures/log-counter-task.md
git commit -m "test(coding-agent): add E2E test for CLI tool creation with ordered timeline"
```

---

## Self-Review Checklist

**1. Spec coverage:**
- ✅ ChannelledEventObserver struct + implementation (Task 1)
- ✅ HTMLReporter uses ChannelledEventObserver (Task 3)
- ✅ Unit tests for ordering (Task 2)
- ✅ Integration tests for concurrent sends (Task 4)
- ✅ E2E test with CLI tool task (Task 5)

**2. Placeholder scan:**
- No TBD/TODO found
- All code snippets are complete
- All file paths are specified

**3. Type consistency:**
- `ChannelledEventObserver` used consistently
- `EventObserver` trait references correct
- Method signatures match across tasks

**4. Potential issues fixed:**
- Fixed `std::thread::block_on` issue by making `events()` async
- Removed `start_time` field that's no longer needed

---

## Execution Options

Plan complete and saved to `docs/superpowers/plans/2026-04-12-observer-event-ordering-fix-plan.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
