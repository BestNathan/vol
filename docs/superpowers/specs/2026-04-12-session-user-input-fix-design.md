# Session 记录 User Input 修复设计文档

**日期:** 2026-04-12  
**状态:** 已批准

---

## 概述

Session 记录缺少用户的第一次输入（user input）。当前 SessionListener 只记录Thinking/ToolCall/Iteration 事件，过滤掉了 `AgentStart` 事件。

**问题现象：**
- Session log 文件只包含助手回复
- 缺少用户输入，无法还原完整对话历史

**原因：**
- `SessionListener::should_record()` 过滤掉 `AgentStart` 事件
- `init_messages()` 只写入 runtime messages，不持久化到 session

---

## 目标

修复后，session log 文件应包含完整对话历史：
1. User input（第一条消息）
2. Assistant thinking
3. Tool calls/results
4. Final answer

---

## 实施方案

### 修改文件

| 文件 | 变更类型 | 说明 |
|------|----------|------|
| `crates/vol-session/src/listener.rs` | 修改 | 添加 AgentStart 事件支持 |
| `crates/vol-session/tests/integration_test.rs` | 修改 | 更新测试验证 user input 被记录 |
| `crates/vol-llm-agent/tests/session_recording_test.rs` | 修改 | 将文档测试改为修复验证测试 |

---

### 详细变更

#### 1. listener.rs - should_record()

**变更前（行 51-59）：**
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

**变更后：**
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

#### 2. listener.rs - event_to_message()

**变更前（行 68-122）：**
```rust
fn event_to_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
    match event {
        // ThinkingComplete -> Assistant message
        AgentStreamEvent::ThinkingComplete { thinking } => Some(SessionMessage::new(
            self.session_id.clone(),
            vol_llm_core::Message::assistant(thinking.clone()),
        )),
        // ... 其他事件
        _ => None,
    }
}
```

**变更后：**
```rust
fn event_to_message(&self, event: &AgentStreamEvent) -> Option<SessionMessage> {
    match event {
        // AgentStart -> User message (NEW)
        AgentStreamEvent::AgentStart { input } => Some(SessionMessage::new(
            self.session_id.clone(),
            vol_llm_core::Message::user(input.clone()),
        )),

        // ThinkingComplete -> Assistant message
        AgentStreamEvent::ThinkingComplete { thinking } => Some(SessionMessage::new(
            self.session_id.clone(),
            vol_llm_core::Message::assistant(thinking.clone()),
        )),
        // ... 其他事件保持不变
    }
}
```

#### 3. 更新测试

**vol-session/tests/integration_test.rs - test_session_listener_filters_events:**

更新测试断言，确认 `AgentStart` 现在**会被记录**。

**vol-llm-agent/tests/session_recording_test.rs:**

将 `test_session_records_user_input` 改为验证修复成功：
```rust
assert!(
    contains_user_input,
    "Session log should contain user input"
);
```

---

## 验收标准

- [ ] `SessionListener::should_record()` 返回 `true` 对于 `AgentStart` 事件
- [ ] `SessionListener::event_to_message()` 将 `AgentStart` 转换为用户消息
- [ ] 运行完整 agent 后，session log 第一条消息是 user input
- [ ] 所有现有测试通过
- [ ] 新增测试验证 user input 被记录

---

## 影响范围

### 内部影响
- SessionListener 记录更多事件（符合预期）
- Session log 文件会增加一条 user input 消息

### 外部影响
- 无破坏性变更
- 依赖 session log 的工具将看到更完整的对话历史

---

## 后续工作

修复完成后，session log 文件的预期结构：
```
{"message":{"role":"user","content":"What is the weather?"}}
{"message":{"role":"assistant","content":"Let me think..."}}
{"message":{"role":"assistant","tool_calls":[...]}}
{"message":{"role":"tool","content":"Tool result"}}
{"message":{"role":"assistant","content":"Final answer"}}
```
