# Agent 生命周期事件系统 — 完整设计

**创建日期**: 2026-04-14  
**状态**: 设计中  
**替代**: `AgentStreamEvent` (原 8 种事件)

---

## 1. 动机

当前 `AgentStreamEvent` 只有 8 种事件，存在以下问题：

1. **事件不完整** — LLM 调用失败、工具执行失败等错误路径直接 `return Err`，不 emit 任何事件，外部观察者看到 agent "突然消失"
2. **缺少流式语义** — LLM 输出被折叠成一个 `ThinkingComplete`，TUI 看不到实时输出，Observability 无法测量 TTFT
3. **缺少 LLM 调用边界** — 多轮迭代中无法区分 "这是第几轮 LLM 调用"、"LLM 调用耗时多久"
4. **工具事件不成对** — `ToolCallBegin` 只有成功时才有对应的 `ToolCallComplete`，失败时没有配对事件

**设计原则**: 每个活动都有 Start + End(成功/失败) 配对，每条执行路径都以终端事件结束。

---

## 2. 事件枚举 (18 种)

### 2.1 Lifecycle (3)

| 事件 | 触发时机 | Payload |
|------|---------|---------|
| `AgentStart` | `run()` 开始 | `input: String` |
| `AgentComplete` | 成功给出最终答案 | — |
| `AgentAborted` | 失败/中止 | `reason: String` |

### 2.2 LLM Call (3)

| 事件 | 触发时机 | Payload |
|------|---------|---------|
| `LLMCallStart` | LLM 请求发出 | `iteration: u32` |
| `LLMCallComplete` | LLM 响应完成 | `model: String`, `usage: Option<TokenUsage>` |
| `LLMCallError` | LLM 请求/流错误 | `error: String` |

### 2.3 Streaming: Thinking (3)

| 事件 | 触发时机 | Payload |
|------|---------|---------|
| `ThinkingStart` | 第一个 thinking token 到达 | — |
| `ThinkingDelta` | 流式 thinking 片段 | `delta: String` |
| `ThinkingComplete` | thinking 结束 | `content: String` (完整内容) |

### 2.4 Streaming: Content (3)

| 事件 | 触发时机 | Payload |
|------|---------|---------|
| `ContentStart` | 第一个 content token 到达 | — |
| `ContentDelta` | 流式 content 片段 | `delta: String` |
| `ContentComplete` | content 结束 | `content: String` (完整内容) |

### 2.5 Tool Execution (4)

| 事件 | 触发时机 | Payload |
|------|---------|---------|
| `ToolCallBegin` | 工具开始执行 | `tool_call_id`, `tool_name`, `arguments` |
| `ToolCallComplete` | 工具成功返回 | `tool_call_id`, `tool_name`, `result` |
| `ToolCallError` | 工具执行失败 | `tool_call_id`, `tool_name`, `error` |
| `ToolCallSkipped` | HITL 拒绝 / 插件 skip | `tool_call_id`, `tool_name`, `reason` |

### 2.6 Iteration (1)

| 事件 | 触发时机 | Payload |
|------|---------|---------|
| `IterationComplete` | 一轮结束 | `iteration`, `tool_calls`, `final_answer: Option<String>` |

### 2.7 Plugin (1)

| 事件 | 触发时机 | Payload |
|------|---------|---------|
| `PluginEvent` | 插件自定义 | `name`, `data` |

---

## 3. 语义保证

### 保证 1: 终端事件

每条执行路径都以 `AgentComplete` 或 `AgentAborted` 结束，不存在 "没有终点" 的路径。

### 保证 2: LLM 调用配对

```
LLMCallStart → LLMCallComplete  或  LLMCallError
```

### 保证 3: 工具执行配对

```
ToolCallBegin → ToolCallComplete  或  ToolCallError  或  ToolCallSkipped
```

### 保证 4: Delta 序列完整性

```
ThinkingStart → ThinkingDelta×N → ThinkingComplete
ContentStart  → ContentDelta×N  → ContentComplete
```

如果 LLM 没有 thinking（如 OpenAI），ThinkingStart/Delta/Complete 全都不 emit。
如果 LLM 没有 content（纯 tool_use），ContentStart/Delta/Complete 全都不 emit。

### 保证 5: 错误路径完整

所有错误路径都 emit 错误事件 + 终端事件：
```
LLM 失败:  AgentStart → LLMCallStart → LLMCallError → AgentAborted
工具失败:  AgentStart → ... → ToolCallBegin → ToolCallError → AgentAborted
```

---

## 4. 事件时序

### 4.1 成功路径（单轮回答，无工具）

```
AgentStart
  LLMCallStart { iteration: 1 }
    ThinkingStart
    ThinkingDelta { "Let me think..." }
    ThinkingDelta { " about this" }
    ThinkingComplete { "Let me think about this..." }
    ContentStart
    ContentDelta { "The answer" }
    ContentDelta { " is 42." }
    ContentComplete { "The answer is 42." }
  LLMCallComplete { model: "qwen3.5-plus", usage: {...} }
IterationComplete { iteration: 1, tool_calls: [], final_answer: Some("The answer is 42.") }
AgentComplete
```

### 4.2 成功路径（多轮工具调用）

```
AgentStart
  LLMCallStart { iteration: 1 }
    ThinkingStart → ThinkingDelta×N → ThinkingComplete
    ContentStart → ContentDelta×N → ContentComplete
  LLMCallComplete { ... }
  ToolCallBegin { "bash", "ls -la" }
  ToolCallComplete { "file1.txt\nfile2.txt" }
  ToolCallBegin { "read_file", "file1.txt" }
  ToolCallComplete { "content of file1" }
IterationComplete { iteration: 1, tool_calls: [...], final_answer: None }
  LLMCallStart { iteration: 2 }
    ThinkingStart → ... → ThinkingComplete
    ContentStart → ... → ContentComplete
  LLMCallComplete { ... }
IterationComplete { iteration: 2, tool_calls: [], final_answer: Some("Analysis complete.") }
AgentComplete
```

### 4.3 LLM 错误路径

```
AgentStart
  LLMCallStart { iteration: 1 }
    ThinkingStart → ThinkingDelta×N          ← 已经输出了一些 thinking
  LLMCallError { "Connection reset by peer" }
AgentAborted { "LLM failed: Connection reset" }
```

### 4.4 工具错误路径

```
AgentStart
  LLMCallStart { iteration: 1 }
    ... → LLMCallComplete
  ToolCallBegin { "bash", "docker restart xxx" }
  ToolCallError { "Command failed: docker not found" }
AgentAborted { "Tool execution failed: docker not found" }
```

### 4.5 HITL 拒绝路径

```
AgentStart
  ...
  ToolCallBegin { "bash", "rm -rf /" }
  ToolCallSkipped { reason: "User rejected" }
  ← 继续处理下一个 tool 或进入下一轮
```

---

## 5. 消费者映射

### 5.1 TUI

| 事件 | 渲染方式 |
|------|---------|
| `AgentStart` | 显示用户输入 |
| `ThinkingStart` | 显示 "Thinking..." 指示器 |
| `ThinkingDelta` | 实时打字机效果 (黄色) |
| `ThinkingComplete` | 折叠为 "Thinking completed" |
| `ContentStart` | 显示 "回答中..." 指示器 |
| `ContentDelta` | 实时打字机效果 (白色) |
| `ContentComplete` | 折叠为最终文本 |
| `ToolCallBegin` | 蓝色 "🔧 tool_name: command" |
| `ToolCallComplete` | 绿色 "✓ tool completed" + 截断结果 |
| `ToolCallError` | 红色 "✗ tool failed: error" |
| `ToolCallSkipped` | 灰色 "⊘ tool skipped: reason" |
| `LLMCallStart/Complete/Error` | 不显示 (meta event) |
| `IterationComplete` | 显示 final_answer 或迭代数 |
| `AgentComplete` | 空行 |
| `AgentAborted` | 红色 "✗ Aborted: reason" |

### 5.2 SessionListener (JSONL 持久化)

| 事件 | should_record | Session Message |
|------|:---:|----------------|
| `AgentStart` | ✓ | `User(input)` |
| `ThinkingComplete` | ✓ | `Assistant(thinking_content)` |
| `ContentComplete` | ✓ | `Assistant(content)` |
| `ToolCallBegin` | ✓ | `Assistant(tool_calls=...)` |
| `ToolCallComplete` | ✓ | `Tool(result)` |
| `ToolCallError` | ✓ | `Tool("Error: {error}")` |
| `ToolCallSkipped` | ✓ | `Tool("Skipped: {reason}")` |
| `IterationComplete` (有 final) | ✓ | `Assistant(final_answer)` |
| 其他所有事件 | ✗ | — |

**不记录 delta 事件** — session 需要的是完整对话消息，不是流式片段。

### 5.3 ObservabilityPlugin

| 事件 | 记录方式 | 用途 |
|------|---------|------|
| `AgentStart` | Run Start | 计时起点 |
| `LLMCallStart` | LLM Start | LLM 耗时测量 |
| `ThinkingStart` | TTFT 记录 | Time to first token |
| `ThinkingDelta` | 追加到 run log | 完整推理链 |
| `ThinkingComplete` | 记录完整 thinking | 推理长度 |
| `ContentDelta` | 追加到 run log | 完整输出 |
| `ContentComplete` | 记录完整 content | 输出长度 |
| `LLMCallComplete` | 记录 model + usage | Token 成本 |
| `LLMCallError` | 记录错误 | 错误率 |
| `ToolCallBegin` | 记录开始时间 | 工具耗时 |
| `ToolCallComplete` | 记录结果 | 工具成功率 |
| `ToolCallError` | 记录错误 | 工具错误率 |
| `ToolCallSkipped` | 记录跳过原因 | HITL 统计 |
| `IterationComplete` | 记录迭代信息 | 迭代次数 |
| `AgentComplete` | Run Complete | 计时终点 |
| `AgentAborted` | Run Aborted | 错误原因 |

---

## 6. 实现设计

### 6.1 文件变更

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/vol-llm-core/src/stream.rs` | 修改 | `AgentStreamEvent` 扩展到 18 种 |
| `crates/vol-llm-agent/src/react/agent.rs` | 修改 | emit 时序 + consume_llm_stream 改造 |
| `crates/vol-session/src/listener.rs` | 修改 | `should_record` + `event_to_message` 更新 |
| `crates/vol-llm-agent/src/observability/` | 修改 | ObservabilityPlugin 适配新事件 |
| `crates/vol-llm-tui/src/render.rs` | 修改 | 渲染新增事件 |
| `crates/vol-llm-provider/src/anthropic.rs` | 检查 | 确认 stream 事件格式兼容 |

### 6.2 broadcast 通道容量

```rust
// 旧
let (event_tx, _) = broadcast::channel(100);

// 新
let (event_tx, _) = broadcast::channel(1024);
```

容量计算: 单轮 LLM 调用 ~50-80 delta + 工具 ~10 + lifecycle ~10 ≈ 100/轮 × max 5 轮 = 500。1024 有 2× 余量。

### 6.3 consume_llm_stream 改造

```rust
async fn consume_llm_stream(
    stream: StreamReceiver,
    run_ctx: &RunContext,
) -> Result<(String, Vec<ToolCall>, String), AgentError>
```

- 接收 `run_ctx` 引用用于实时 emit 流式事件
- 在消费过程中 emit ThinkingStart/Delta/Complete、ContentStart/Delta/Complete
- LLMCallComplete 由调用方 (agent.rs) 在 consume 返回后 emit
- LLMCallError 在 consume 返回 Err 时由调用方 emit

### 6.4 agent.rs emit 时序

```rust
// AgentStart
emit(AgentStart { input });
intercept → Abort? → emit(AgentAborted) → return Err;

loop {
    // check max iterations → emit(AgentAborted) → return Err;

    // LLM
    emit(LLMCallStart { iteration });
    let stream = match llm.converse_stream().await {
        Err(e) => {
            emit(LLMCallError { error });
            emit(AgentAborted { reason });
            return Err;
        }
        Ok(s) => s,
    };
    match consume_llm_stream(stream, &run_ctx).await {
        Ok((thinking, tool_calls, content)) => {
            emit(LLMCallComplete { model, usage });
            // thinking/content/tool_calls 已由 consume 函数 emit delta 和 complete 事件
        }
        Err(e) => {
            emit(LLMCallError { error });
            emit(AgentAborted { reason });
            return Err;
        }
    };

    // Tools
    for call in &tool_calls {
        emit(ToolCallBegin { ... });
        intercept → Skip → emit(ToolCallSkipped) → continue;
        intercept → Abort → emit(AgentAborted) → return Err;
        match execute() {
            Err(e) => {
                emit(ToolCallError { ... });
                emit(AgentAborted { ... });
                return Err;
            }
            Ok(r) => emit(ToolCallComplete { ... }),
        }
    }

    if !tool_calls.is_empty() {
        emit(IterationComplete { tool_calls, final_answer: None });
        continue;
    }

    // Final answer
    emit(IterationComplete { final_answer: Some(content) });
    emit(AgentComplete);
    return Ok(finalize());
}
```

---

## 7. 向后兼容

`AgentStreamEvent` 是公共 API，现有消费者（TUI、SessionListener、ObservabilityPlugin、所有测试）都需要更新。

兼容性策略：
- `AgentStart`, `AgentComplete`, `AgentAborted` — 保持不变（向后兼容）
- `ThinkingComplete` — 保持，新增 `ThinkingStart`, `ThinkingDelta`
- `ToolCallBegin` — 保持不变
- `ToolCallComplete` — 保持不变，新增 `ToolCallError`, `ToolCallSkipped`
- `IterationComplete` — 保持不变
- `PluginEvent` — 保持不变

新增事件都是新 variant，现有 match 需要添加 `_ => {}` 或显式处理。

---

## 8. 后续扩展

### 8.1 流式工具输出

未来如果某个工具需要流式输出（如长运行 bash 命令实时输出），可引入：
```rust
ToolOutputDelta { tool_call_id: String, delta: String },
ToolOutputComplete { tool_call_id: String, result: String },
```
当前不实现。

### 8.2 性能指标

有了完整事件链，可以轻松计算：
- **TTFT**: `ThinkingStart.timestamp - LLMCallStart.timestamp`
- **LLM 总耗时**: `LLMCallComplete.timestamp - LLMCallStart.timestamp`
- **Agent 总耗时**: `AgentComplete.timestamp - AgentStart.timestamp`
- **工具平均耗时**: `ToolCallComplete.timestamp - ToolCallBegin.timestamp` 的平均
- **迭代效率**: tool_calls / iteration 比率

### 8.3 事件过滤

ObservabilityPlugin 可以按需过滤：
```toml
[agent.observability]
emit_level = "complete_only"  # 只记录 Complete 事件，跳过 delta
# 或
emit_level = "all"            # 记录所有事件（含 delta）
```
