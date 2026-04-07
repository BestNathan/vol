# ReAct Agent Stream Mode Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor ReActAgent::run() to return AgentStreamReceiver with Agent-level streaming events.

**Architecture:** 
1. Add `AgentStreamEvent` and `AgentStreamReceiver` types to `response.rs`
2. Refactor `ReActAgent::run()` to spawn async task and send events through mpsc channel
3. Add `consume_llm_stream()` helper to accumulate LLM stream events
4. Update tests to use new streaming interface

**Tech Stack:** Rust, tokio (async runtime, mpsc channels), existing vol-llm-core stream types

---

## File Structure

**Files to Create:**
- (None - all changes to existing files)

**Files to Modify:**
- `crates/vol-llm-agent/src/response.rs` - Add AgentStreamEvent and AgentStreamReceiver
- `crates/vol-llm-agent/src/agent.rs` - Refactor run() to return AgentStreamReceiver
- `crates/vol-llm-agent/src/lib.rs` - Export new types
- `crates/vol-llm-agent/tests/react_mock_test.rs` - Update tests for streaming

---

### Task 1: Add AgentStreamEvent and AgentStreamReceiver Types

**Files:**
- Modify: `crates/vol-llm-agent/src/response.rs`

- [ ] **Step 1: Add AgentStreamEvent enum to response.rs**

Add the following code to `crates/vol-llm-agent/src/response.rs` after the `AgentError` enum:

```rust
/// Agent streaming event
#[derive(Debug, Clone)]
pub enum AgentStreamEvent {
    /// Agent started execution
    AgentStart { input: String },
    
    /// LLM thinking completed
    ThinkingComplete { thinking: String },
    
    /// About to call tool
    ToolCallBegin { tool_name: String, arguments: String },
    
    /// Tool call completed
    ToolCallComplete { tool_name: String, result: String },
    
    /// One iteration completed (Reason-Act-Observation)
    IterationComplete {
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },
    
    /// Agent execution completed
    AgentComplete { response: AgentResponse },
    
    /// Error occurred
    Error { error: AgentError },
}

/// Agent stream receiver
pub struct AgentStreamReceiver {
    rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, AgentError>>,
}

impl AgentStreamReceiver {
    pub fn new(rx: tokio::sync::mpsc::Receiver<Result<AgentStreamEvent, AgentError>>) -> Self {
        Self { rx }
    }

    pub async fn recv(&mut self) -> Option<Result<AgentStreamEvent, AgentError>> {
        self.rx.recv().await
    }
}
```

- [ ] **Step 2: Run cargo check to verify code compiles**

```bash
cd crates/vol-llm-agent && cargo check
```

Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/response.rs
git commit -m "feat: add AgentStreamEvent and AgentStreamReceiver types"
```

---

### Task 2: Export New Types from lib.rs

**Files:**
- Modify: `crates/vol-llm-agent/src/lib.rs`

- [ ] **Step 1: Update lib.rs to export new types**

Modify `crates/vol-llm-agent/src/lib.rs`:

```rust
//! vol-llm-agent: ReAct Agent workflow orchestration.

pub mod agent;
pub mod response;
pub mod builder;
pub mod prompt;

pub use agent::*;
pub use response::*;  // Now exports AgentStreamEvent and AgentStreamReceiver
pub use builder::*;
pub use prompt::*;
```

- [ ] **Step 2: Run cargo check to verify exports**

```bash
cd crates/vol-llm-agent && cargo check
```

Expected: Compiles without errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/lib.rs
git commit -m "chore: export AgentStreamEvent and AgentStreamReceiver"
```

---

### Task 3: Add consume_llm_stream Helper Function

**Files:**
- Modify: `crates/vol-llm-agent/src/agent.rs`

- [ ] **Step 1: Add required imports at top of agent.rs**

Add these imports at the top of `crates/vol-llm-agent/src/agent.rs`:

```rust
use tokio::sync::mpsc;
use vol_llm_core::{StreamEventData, StreamReceiver};
use crate::{AgentStreamEvent, AgentStreamReceiver};
```

- [ ] **Step 2: Add consume_llm_stream helper function**

Add this helper function in `crates/vol-llm-agent/src/agent.rs` (before or after the `ReActAgent` impl block):

```rust
/// Consume LLM stream response and accumulate into complete data
async fn consume_llm_stream(
    mut stream: StreamReceiver,
) -> Result<(String, Vec<vol_llm_core::ToolCall>, String), crate::AgentError> {
    let mut thinking = String::new();
    let mut tool_calls = Vec::new();
    let mut content = String::new();

    while let Some(result) = stream.recv().await {
        match result? {
            StreamEventData::ThinkingComplete { thinking: t } => {
                thinking = t;
            }
            StreamEventData::ToolCallComplete { tool_call } => {
                tool_calls.push(tool_call);
            }
            StreamEventData::ContentComplete { content: c } => {
                content = c;
            }
            _ => {}
        }
    }

    Ok((thinking, tool_calls, content))
}
```

- [ ] **Step 3: Run cargo check to verify helper compiles**

```bash
cd crates/vol-llm-agent && cargo check
```

Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/agent.rs
git commit -m "feat: add consume_llm_stream helper function"
```

---

### Task 4: Refactor ReActAgent::run to Return AgentStreamReceiver

**Files:**
- Modify: `crates/vol-llm-agent/src/agent.rs`

- [ ] **Step 1: Replace run method with streaming implementation**

Replace the existing `run` method in `crates/vol-llm-agent/src/agent.rs`:

```rust
pub async fn run(
    &self,
    user_input: &str,
    context: ToolContext,
) -> Result<AgentStreamReceiver, crate::AgentError> {
    let (tx, rx) = mpsc::channel(100);

    tokio::spawn(async move {
        // Send AgentStart event
        let _ = tx.send(Ok(AgentStreamEvent::AgentStart { 
            input: user_input.to_string() 
        })).await;

        let mut messages = Vec::new();
        let mut iteration = 0;

        messages.push(Message::system(self.config.system_prompt.clone()));
        messages.push(Message::user(user_input));

        loop {
            iteration += 1;

            if iteration > self.config.max_iterations {
                let _ = tx.send(Err(crate::AgentError::MaxIterationsReached { 
                    max: self.config.max_iterations 
                })).await;
                break;
            }

            if self.config.verbose {
                info!("Iteration {}", iteration);
            }

            // Reason phase - call LLM with streaming
            let tools = self.tools.definitions();
            let request = ConversationRequest::with_history(None, messages.clone())
                .with_tools(tools)
                .with_tool_choice(ToolChoice::Auto);

            let llm_stream = match self.llm.converse_stream(request).await {
                Ok(stream) => stream,
                Err(e) => {
                    let _ = tx.send(Err(crate::AgentError::Llm(e))).await;
                    break;
                }
            };

            // Consume LLM stream and accumulate events
            let (thinking, tool_calls, content) = match consume_llm_stream(llm_stream).await {
                Ok(data) => data,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    break;
                }
            };

            // Send ThinkingComplete if we have thinking content
            if !thinking.is_empty() {
                let _ = tx.send(Ok(AgentStreamEvent::ThinkingComplete { thinking })).await;
            }

            // Check if tool calls
            if !tool_calls.is_empty() {
                debug!("Tool calls: {:?}", tool_calls);

                // Act phase - execute tools
                for call in &tool_calls {
                    info!("Executing tool: {} with args: {}", call.name, call.arguments);

                    // Send ToolCallBegin
                    let _ = tx.send(Ok(AgentStreamEvent::ToolCallBegin {
                        tool_name: call.name.clone(),
                        arguments: call.arguments.clone(),
                    })).await;

                    // Execute tool
                    let result = match self.tools.execute(call, &context).await {
                        Ok(r) => r,
                        Err(e) => {
                            let _ = tx.send(Err(crate::AgentError::ToolExecution {
                                tool: call.name.clone(),
                                error: e.to_string(),
                            })).await;
                            break;
                        }
                    };

                    info!("Tool {} returned: {}", call.name, result.content);

                    // Send ToolCallComplete
                    let _ = tx.send(Ok(AgentStreamEvent::ToolCallComplete {
                        tool_name: call.name.clone(),
                        result: result.content.clone(),
                    })).await;

                    // Add tool result to messages
                    messages.push(Message::tool(result.content.clone(), call.id.clone()));
                }

                // Send IterationComplete
                let _ = tx.send(Ok(AgentStreamEvent::IterationComplete {
                    iteration,
                    tool_calls: tool_calls.clone(),
                    final_answer: None,
                })).await;

                // Continue to next iteration
                continue;
            }

            // No tool calls - we have final answer
            // Send IterationComplete with final answer
            let _ = tx.send(Ok(AgentStreamEvent::IterationComplete {
                iteration,
                tool_calls: Vec::new(),
                final_answer: Some(content.clone()),
            })).await;

            // Send AgentComplete
            let response = AgentResponse {
                content,
                reasoning: String::new(),
                iterations: iteration,
                tool_calls,
            };

            let _ = tx.send(Ok(AgentStreamEvent::AgentComplete { response })).await;
            break;
        }
    });

    Ok(AgentStreamReceiver::new(rx))
}
```

- [ ] **Step 2: Run cargo check to verify implementation compiles**

```bash
cd crates/vol-llm-agent && cargo check
```

Expected: Compiles without errors (tests may fail, will fix in Task 5)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/agent.rs
git commit -m "feat: refactor ReActAgent::run to return AgentStreamReceiver"
```

---

### Task 5: Update Tests for Streaming Interface

**Files:**
- Modify: `crates/vol-llm-agent/tests/react_mock_test.rs`

- [ ] **Step 1: Update test_agent_basic to use streaming**

Replace the test in `crates/vol-llm-agent/tests/react_mock_test.rs`:

```rust
#[tokio::test]
async fn test_agent_basic() {
    let mock_llm = MockStreamingLlm::new()
        .with_response("The weather in Beijing is sunny.");

    let tools = Arc::new(ToolRegistry::new());
    let config = AgentConfig {
        max_iterations: 5,
        system_prompt: system_prompt().to_string(),
        verbose: false,
    };

    let agent = ReActAgent::new(Arc::new(mock_llm), tools, config);
    let context = ToolContext::default();

    // Consume stream to get final response
    let mut stream = agent.run("What's the weather in Beijing?", context).await.unwrap();
    
    let mut final_response = None;
    while let Some(event) = stream.recv().await {
        match event.unwrap() {
            AgentStreamEvent::AgentComplete { response } => {
                final_response = Some(response);
                break;
            }
            _ => {}
        }
    }

    let response = final_response.expect("Should have final response");
    assert!(response.content.contains("Beijing"));
}
```

- [ ] **Step 2: Run tests to verify they pass**

```bash
cd crates/vol-llm-agent && cargo test -- --nocapture
```

Expected: All tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/tests/react_mock_test.rs
git commit -m "test: update tests for streaming interface"
```

---

### Task 6: Add Unit Tests for AgentStreamEvent

**Files:**
- Modify: `crates/vol-llm-agent/src/response.rs`

- [ ] **Step 1: Add unit tests at end of response.rs**

Add to `crates/vol-llm-agent/src/response.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_stream_event_creation() {
        let event = AgentStreamEvent::AgentStart { input: "test".to_string() };
        match event {
            AgentStreamEvent::AgentStart { input } => {
                assert_eq!(input, "test");
            }
            _ => panic!("Expected AgentStart"),
        }
    }

    #[test]
    fn test_agent_stream_event_tool_call() {
        let event = AgentStreamEvent::ToolCallBegin {
            tool_name: "get_weather".to_string(),
            arguments: r#"{"city": "Beijing"}"#.to_string(),
        };
        match event {
            AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
                assert_eq!(tool_name, "get_weather");
                assert_eq!(arguments, r#"{"city": "Beijing"}"#);
            }
            _ => panic!("Expected ToolCallBegin"),
        }
    }

    #[test]
    fn test_agent_stream_event_iteration_complete() {
        let event = AgentStreamEvent::IterationComplete {
            iteration: 1,
            tool_calls: Vec::new(),
            final_answer: Some("The answer".to_string()),
        };
        match event {
            AgentStreamEvent::IterationComplete { iteration, final_answer, .. } => {
                assert_eq!(iteration, 1);
                assert_eq!(final_answer, Some("The answer".to_string()));
            }
            _ => panic!("Expected IterationComplete"),
        }
    }
}
```

- [ ] **Step 2: Run unit tests**

```bash
cd crates/vol-llm-agent && cargo test response::tests -- --nocapture
```

Expected: All 3 tests pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/response.rs
git commit -m "test: add unit tests for AgentStreamEvent"
```

---

## Self-Review

**1. Spec Coverage:**
- [x] AgentStreamEvent enum with all variants - Task 1
- [x] AgentStreamReceiver struct - Task 1
- [x] consume_llm_stream helper - Task 3
- [x] ReActAgent::run refactored - Task 4
- [x] Tests updated for streaming - Task 5
- [x] Unit tests for AgentStreamEvent - Task 6

**2. Placeholder Scan:**
- No TBD/TODO in steps
- All code steps contain actual code
- All test steps contain actual test code

**3. Type Consistency:**
- `AgentStreamEvent` variants match spec
- `AgentStreamReceiver::recv()` return type consistent
- `consume_llm_stream` signature matches usage in Task 4

---

Plan complete and saved to `docs/superpowers/plans/2026-04-07-react-agent-stream-mode.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
