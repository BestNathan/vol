# Session User Input Recording Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 修复 SessionListener 使其记录用户输入（AgentStart 事件），确保 session log 包含完整对话历史。

**Architecture:** 在 SessionListener::should_record() 中添加 AgentStart 事件支持，在 event_to_message() 中将其转换为用户消息。采用 TDD 方法，先写失败测试，再实现修复。

**Tech Stack:** Rust, tokio, vol-session, vol-llm-core

---

## File Structure

| File | 职责 | 变更类型 |
|------|------|----------|
| `crates/vol-session/src/listener.rs` | SessionListener 实现 | 修改：添加 AgentStart 支持 |
| `crates/vol-session/tests/integration_test.rs` | SessionListener 集成测试 | 修改：更新测试断言 |
| `crates/vol-llm-agent/tests/session_recording_test.rs` | Session 记录测试 | 修改：验证修复成功 |

---

### Task 1: 编写失败测试 - should_record AgentStart

**Files:**
- Modify: `crates/vol-session/src/listener.rs:206-212`

- [ ] **Step 1: 读取当前测试**

当前测试（行 206-212）：
```rust
#[tokio::test]
async fn test_should_not_record_agent_start() {
    let event = AgentStreamEvent::AgentStart {
        input: "test".to_string(),
    };
    assert!(!SessionListener::should_record(&event));
}
```

- [ ] **Step 2: 修改测试为期望失败**

将测试改为验证修复后的行为：
```rust
#[tokio::test]
async fn test_should_record_agent_start() {
    let event = AgentStreamEvent::AgentStart {
        input: "test".to_string(),
    };
    assert!(SessionListener::should_record(&event));
}
```

- [ ] **Step 3: 运行测试验证失败**

```bash
cargo test -p vol-session --lib listener::tests::test_should_record_agent_start
```
Expected: FAIL - 因为 should_record() 还未支持 AgentStart

- [ ] **Step 4: Commit 测试**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "test: add failing test for should_record AgentStart"
```

---

### Task 2: 实现 should_record() 支持 AgentStart

**Files:**
- Modify: `crates/vol-session/src/listener.rs:51-59`

- [ ] **Step 1: 修改 should_record() 函数**

将：
```rust
fn should_record(event: &AgentStreamEvent) -> bool {
    matches!(
        event,
        AgentStreamEvent::ThinkingComplete { .. }
            | AgentStreamEvent::ToolCallBegin { .. }
            | AgentStreamEvent::ToolCallComplete { .. }
            | AgentStreamEvent::IterationComplete { .. }
    )
}
```

改为：
```rust
fn should_record(event: &AgentStreamEvent) -> bool {
    matches!(
        event,
        AgentStreamEvent::AgentStart { .. }
            | AgentStreamEvent::ThinkingComplete { .. }
            | AgentStreamEvent::ToolCallBegin { .. }
            | AgentStreamEvent::ToolCallComplete { .. }
            | AgentStreamEvent::IterationComplete { .. }
    )
}
```

- [ ] **Step 2: 运行测试验证通过**

```bash
cargo test -p vol-session --lib listener::tests::test_should_record_agent_start
```
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "feat: should_record() now records AgentStart events"
```

---

### Task 3: 编写失败测试 - event_to_message AgentStart

**Files:**
- Modify: `crates/vol-session/src/listener.rs:163-`

- [ ] **Step 1: 添加新测试**

在 `#[cfg(test)] mod tests` 中添加：
```rust
#[tokio::test]
async fn test_event_to_message_agent_start() {
    let store = Arc::new(InMemoryMessageStore::new());
    let (_tx, rx) = broadcast::channel(100);
    let listener = SessionListener::new(rx, store, "session-1".to_string());

    let event = AgentStreamEvent::AgentStart {
        input: "User's question".to_string(),
    };

    let msg = listener.event_to_message(&event).unwrap();
    assert_eq!(msg.session_id, "session-1");
    assert_eq!(msg.message.role, vol_llm_core::MessageRole::User);
    assert_eq!(msg.message.content, Some("User's question".to_string()));
}
```

- [ ] **Step 2: 运行测试验证失败**

```bash
cargo test -p vol-session --lib listener::tests::test_event_to_message_agent_start
```
Expected: FAIL - 因为 event_to_message() 还未处理 AgentStart

- [ ] **Step 3: Commit 测试**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "test: add failing test for event_to_message AgentStart"
```

---

### Task 4: 实现 event_to_message() 支持 AgentStart

**Files:**
- Modify: `crates/vol-session/src/listener.rs:68-122`

- [ ] **Step 1: 修改 event_to_message() 函数**

将：
```rust
fn event_to_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
    match event {
        // ThinkingComplete -> Assistant message (thinking content)
        AgentStreamEvent::ThinkingComplete { thinking } => Some(SessionMessage::new(
            self.session_id.clone(),
            vol_llm_core::Message::assistant(thinking.clone()),
        )),
        // ... 其他事件
        _ => None,
    }
}
```

改为：
```rust
fn event_to_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
    match event {
        // AgentStart -> User message (NEW)
        AgentStreamEvent::AgentStart { input } => Some(SessionMessage::new(
            self.session_id.clone(),
            vol_llm_core::Message::user(input.clone()),
        )),

        // ThinkingComplete -> Assistant message (thinking content)
        AgentStreamEvent::ThinkingComplete { thinking } => Some(SessionMessage::new(
            self.session_id.clone(),
            vol_llm_core::Message::assistant(thinking.clone()),
        )),

        // ... 其他事件保持不变
    }
}
```

- [ ] **Step 2: 运行测试验证通过**

```bash
cargo test -p vol-session --lib listener::tests::test_event_to_message_agent_start
```
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-session/src/listener.rs
git commit -m "feat: event_to_message() converts AgentStart to User message"
```

---

### Task 5: 更新现有测试断言

**Files:**
- Modify: `crates/vol-session/tests/integration_test.rs:101-166`

- [ ] **Step 1: 读取 test_session_listener_filters_events 测试**

当前测试（行 125-159）期望 AgentStart **不**被记录：
```rust
// Send events that should NOT be recorded
let _ = event_tx.send(TracedEvent::without_span(AgentStreamEvent::AgentStart {
    input: "test input".to_string(),
}));

// ...

assert_eq!(
    lines.len(),
    2,
    "Expected 2 lines (filtered), got {}",
    lines.len()
);
```

- [ ] **Step 2: 更新测试断言**

修改测试期望 AgentStart **会**被记录：
```rust
// Send AgentStart event (should NOW be recorded)
event_tx
    .send(TracedEvent::without_span(AgentStreamEvent::AgentStart {
        input: "test input".to_string(),
    }))
    .map_err(|_| "send error")
    .unwrap();

// ...

// Should have 3 lines now (Thinking + AgentStart + ToolCallComplete)
assert_eq!(
    lines.len(),
    3,
    "Expected 3 lines (AgentStart now recorded), got {}",
    lines.len()
);

// Verify AgentStart is recorded as user message
let contains_user_input = content.lines().any(|l| l.contains("test input"));
assert!(
    contains_user_input,
    "Session log should contain user input 'test input'"
);
```

- [ ] **Step 3: 运行测试验证通过**

```bash
cargo test -p vol-session --test integration_test test_session_listener_filters_events
```
Expected: PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-session/tests/integration_test.rs
git commit -m "test: update filters test to expect AgentStart is recorded"
```

---

### Task 6: 更新 session_recording_test.rs 验证修复

**Files:**
- Modify: `crates/vol-llm-agent/tests/session_recording_test.rs`

- [ ] **Step 1: 修改 test_session_listener_records_what_events 测试**

将断言从期望不记录改为期望记录：
```rust
// AgentStart SHOULD NOW be recorded
let contains_user_input = content.contains("User's first input");
assert!(
    contains_user_input,
    "AgentStart should be recorded as user message, content was: {}",
    content
);
```

- [ ] **Step 2: 修改 test_session_records_user_input 测试**

将断言改为期望成功：
```rust
assert!(
    contains_user_input,
    "Session log should contain user input '{}', content was: {}",
    user_input, content
);
```

- [ ] **Step 3: 运行测试验证通过**

```bash
cargo test -p vol-llm-agent --test session_recording_test -- --nocapture
```
Expected: Both tests PASS

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/tests/session_recording_test.rs
git commit -m "test: update session_recording_test to verify user input is recorded"
```

---

### Task 7: 运行完整测试套件

**Files:**
- Test: Full test suite

- [ ] **Step 1: 运行 vol-session 测试**

```bash
cargo test -p vol-session
```
Expected: All tests pass

- [ ] **Step 2: 运行 vol-llm-agent 测试**

```bash
cargo test -p vol-llm-agent
```
Expected: All tests pass

- [ ] **Step 3: 验证编译无警告**

```bash
cargo check -p vol-session
cargo check -p vol-llm-agent
```
Expected: No warnings

- [ ] **Step 4: 验证 clippy**

```bash
cargo clippy -p vol-session
cargo clippy -p vol-llm-agent
```
Expected: No new warnings

---

## Self-Review Checklist

**1. Spec coverage:**
- ✅ should_record() 支持 AgentStart (Task 2)
- ✅ event_to_message() 转换 AgentStart 为用户消息 (Task 4)
- ✅ 测试验证修复成功 (Task 1, 3, 5, 6)
- ✅ 更新现有测试断言 (Task 5, 6)
- ✅ 完整测试套件通过 (Task 7)

**2. Placeholder scan:**
- ✅ 无 TBD/TODO
- ✅ 所有代码步骤都有具体代码
- ✅ 所有命令都有预期输出

**3. Type consistency:**
- ✅ `AgentStreamEvent::AgentStart` 使用正确
- ✅ `vol_llm_core::Message::user()` 使用正确
- ✅ `MessageRole::User` 使用正确
- ✅ `SessionMessage::new()` 使用正确

---

Plan complete and saved to `docs/superpowers/plans/2026-04-12-session-user-input-fix-plan.md`.

Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
