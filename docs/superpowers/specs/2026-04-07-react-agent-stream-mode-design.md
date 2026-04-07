# ReAct Agent Stream Mode Design

**日期**: 2026-04-07  
**作者**: Claude (with user collaboration)  
**状态**: Approved

## Overview

将 ReAct Agent 的 `run` 方法改造为流式模式，返回 Agent 层级的事件流，而非直接返回最终响应。

## Goals

1. 重构 `ReActAgent::run()` 返回 `AgentStreamReceiver`
2. 定义 Agent 层级的流式事件（`AgentStreamEvent`）
3. 事件设计为语义完整的 Agent 事件，不透传底层 LLM delta
4. 保持与 `vol-llm-core` stream mode 设计一致的模式

## Non-Goals

- 不透传 LLM 底层的 `ContentDelta`/`ThinkingDelta` 事件
- 不修改 `vol-llm-provider` 或 `vol-llm-core`（已完成）
- 不提供向后兼容的同步接口（调用方自行消费流构建响应）

---

## Architecture

### Module Structure

```
crates/vol-llm-agent/src/
├── agent.rs        # ReActAgent 实现 (重构 run 方法)
├── response.rs     # AgentStreamEvent 和 AgentStreamReceiver 定义
├── builder.rs      # AgentBuilder (可能需要更新)
├── prompt.rs       # Prompt 相关 (不变)
└── lib.rs          # 导出新类型
```

### Event Flow

```
ReActAgent::run()
│
├─ 1. Send AgentStart { input }
│
├─ 2. LLM 流式调用 (内部消费，不对外暴露)
│   └─ 累积 thinking → send ThinkingComplete
│   └─ 累积 tool_calls → send ToolCallBegin/Complete
│
├─ 3. Send ToolCallBegin { tool_name, arguments }
│   └─ 执行工具
├─ 4. Send ToolCallComplete { tool_name, result }
│
├─ 5. Send IterationComplete { iteration, tool_calls, final_answer }
│
├─ 6. [重复 2-5 直到完成]
│
└─ 7. Send AgentComplete { response }
```

---

## Data Model

### AgentStreamEvent

```rust
/// Agent 流式事件
pub enum AgentStreamEvent {
    /// Agent 开始执行
    AgentStart { input: String },
    
    /// LLM 思考完成
    ThinkingComplete { thinking: String },
    
    /// 即将调用工具
    ToolCallBegin { tool_name: String, arguments: String },
    
    /// 工具调用完成
    ToolCallComplete { tool_name: String, result: String },
    
    /// 完成一轮迭代（Reason-Act-Observation）
    IterationComplete {
        iteration: u32,
        tool_calls: Vec<ToolCall>,
        final_answer: Option<String>,
    },
    
    /// Agent 执行完成
    AgentComplete { response: AgentResponse },
    
    /// 错误
    Error { error: AgentError },
}
```

### AgentStreamReceiver

```rust
/// Agent 流式接收器
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

### 使用方式

```rust
// 流式消费
let mut stream = agent.run(input, context).await?;
while let Some(event) = stream.recv().await {
    match event? {
        AgentStreamEvent::ToolCallBegin { tool_name, arguments } => {
            println!("准备调用工具：{} 参数：{}", tool_name, arguments);
        }
        AgentStreamEvent::ToolCallComplete { tool_name, result } => {
            println!("工具 {} 返回：{}", tool_name, result);
        }
        AgentStreamEvent::AgentComplete { response } => {
            println!("最终回答：{}", response.content);
            break;
        }
        _ => {}
    }
}

// 或者累积为完整响应
let mut stream = agent.run(input, context).await?;
let mut final_response = None;
while let Some(event) = stream.recv().await {
    if let AgentStreamEvent::AgentComplete { response } = event? {
        final_response = Some(response);
        break;
    }
}
```

---

## Implementation Details

### ReActAgent::run 重构

**当前实现（同步）：**

```rust
pub async fn run(&self, user_input: &str, context: ToolContext) -> Result<AgentResponse, AgentError> {
    let mut messages = Vec::new();
    let mut iteration = 0;

    messages.push(Message::system(self.config.system_prompt.clone()));
    messages.push(Message::user(user_input));

    loop {
        iteration += 1;
        if iteration > self.config.max_iterations {
            return Err(AgentError::MaxIterationsReached { max: self.config.max_iterations });
        }

        // Call LLM (同步)
        let response = self.llm.converse(request).await?;

        // 检查工具调用
        if let Some(tool_calls) = &response.message.tool_calls {
            if !tool_calls.is_empty() {
                // 执行工具
                for call in tool_calls {
                    let result = self.tools.execute(call, &context).await?;
                    messages.push(Message::tool(result.content, call.id.clone()));
                }
                continue;
            }
        }

        // 返回最终响应
        return Ok(AgentResponse { content, ... });
    }
}
```

**重构后（流式）：**

```rust
pub async fn run(&self, user_input: &str, context: ToolContext) -> Result<AgentStreamReceiver, AgentError> {
    let (tx, rx) = tokio::sync::mpsc::channel(100);

    tokio::spawn(async move {
        // 发送 AgentStart 事件
        let _ = tx.send(Ok(AgentStreamEvent::AgentStart { input: user_input.to_string() })).await;

        let mut messages = Vec::new();
        let mut iteration = 0;

        messages.push(Message::system(self.config.system_prompt.clone()));
        messages.push(Message::user(user_input));

        loop {
            iteration += 1;
            if iteration > self.config.max_iterations {
                let _ = tx.send(Err(AgentError::MaxIterationsReached { max: self.config.max_iterations })).await;
                break;
            }

            // 调用 LLM (流式内部消费)
            let llm_stream = self.llm.converse_stream(request).await?;
            
            // 消费 LLM 流，累积事件
            let (thinking, tool_calls, content) = consume_llm_stream(llm_stream).await?;
            
            // 发送 ThinkingComplete
            if !thinking.is_empty() {
                let _ = tx.send(Ok(AgentStreamEvent::ThinkingComplete { thinking })).await;
            }

            // 发送 ToolCallBegin/Complete
            for call in &tool_calls {
                let _ = tx.send(Ok(AgentStreamEvent::ToolCallBegin { 
                    tool_name: call.name.clone(), 
                    arguments: call.arguments.clone(),
                })).await;
                
                let result = self.tools.execute(call, &context).await?;
                
                let _ = tx.send(Ok(AgentStreamEvent::ToolCallComplete { 
                    tool_name: call.name.clone(), 
                    result: result.content.clone(),
                })).await;
                
                messages.push(Message::tool(result.content, call.id.clone()));
            }

            // 发送 IterationComplete
            let _ = tx.send(Ok(AgentStreamEvent::IterationComplete {
                iteration,
                tool_calls: tool_calls.clone(),
                final_answer: if tool_calls.is_empty() { Some(content.clone()) } else { None },
            })).await;

            // 检查是否完成
            if tool_calls.is_empty() {
                let _ = tx.send(Ok(AgentStreamEvent::AgentComplete { 
                    response: AgentResponse { content, .. } 
                })).await;
                break;
            }
        }
    });

    Ok(AgentStreamReceiver::new(rx))
}
```

### 辅助函数：consume_llm_stream

```rust
/// 消费 LLM 流式响应，累积为完整数据
async fn consume_llm_stream(
    mut stream: StreamReceiver,
) -> Result<(String, Vec<ToolCall>, String), AgentError> {
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

---

## Error Handling

**错误场景：**

| 场景 | 返回类型 | 处理方式 |
|------|---------|---------|
| 参数验证失败 | `Err(AgentError)` | 直接返回 |
| LLM 调用失败 | `Err(AgentError)` 通过 channel 发送 | 发送 Error 事件 |
| 工具执行失败 | `Err(AgentError)` 通过 channel 发送 | 发送 Error 事件 |
| 超过最大迭代次数 | `Err(AgentError)` 通过 channel 发送 | 发送 Error 事件 |
| Receiver 已丢弃 | 检测 `tx.send().is_err()` | 停止发送 |

---

## Testing Strategy

### 单元测试

```rust
#[tokio::test]
async fn test_agent_stream_event_order() {
    // 验证事件顺序：AgentStart → ... → AgentComplete
}

#[tokio::test]
async fn test_agent_stream_tool_call_events() {
    // 验证 ToolCallBegin/Complete 成对出现
}
```

### 集成测试

```rust
#[tokio::test]
async fn test_agent_stream_with_mock_llm() {
    // 使用 mock LLM 验证完整事件流
}
```

---

## Backward Compatibility

**破坏性变更：**

- `ReActAgent::run()` 返回类型从 `Result<AgentResponse>` 变为 `Result<AgentStreamReceiver>`
- 现有调用方需要更新为流式消费模式

**迁移指南：**

```rust
// 旧代码
let response = agent.run(input, context).await?;

// 新代码（流式消费）
let mut stream = agent.run(input, context).await?;
while let Some(event) = stream.recv().await {
    if let AgentStreamEvent::AgentComplete { response } = event? {
        // 使用 response
    }
}

// 或（累积为响应）
let mut stream = agent.run(input, context).await?;
let mut final_response = None;
while let Some(event) = stream.recv().await {
    if let AgentStreamEvent::AgentComplete { response } = event? {
        final_response = Some(response);
        break;
    }
}
```

---

## Future Work

1. 如果需要，可以添加 `run_sync()` 辅助方法作为兼容层
2. 增加更多事件类型（如 `AgentPause`、`AgentResume`）
3. 支持事件过滤或订阅特定事件类型

---

## Appendix: Files to Modify

| 文件 | 变更类型 |
|------|---------|
| `crates/vol-llm-agent/src/response.rs` | 新增 `AgentStreamEvent` 和 `AgentStreamReceiver` |
| `crates/vol-llm-agent/src/agent.rs` | 重构 `run` 方法，返回 `AgentStreamReceiver` |
| `crates/vol-llm-agent/src/lib.rs` | 导出新类型 |
| `crates/vol-llm-agent/tests/*` | 更新测试使用流式接口 |
