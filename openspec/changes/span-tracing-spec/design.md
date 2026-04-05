## Context

当前项目中 tracing 的使用存在以下问题：

1. **Span 创建不统一**：有的地方手动 `info_span!()`，有的地方用 `#[instrument]` 宏
2. **跨 await 传递不一致**：部分代码用 `.enter()` + guard，部分用 `.instrument()`
3. **TraceId 管理分散**：没有统一的生成和获取工具
4. **跨 channel 传播模式未标准化**：`WithSpan` wrapper 已存在但未统一使用

**约束条件**：
- Rust 项目，tokio 异步运行时
- 使用 OpenTelemetry + Jaeger 进行分布式追踪
- 数据流：DataSource → Rule Engine → Alert → Notification（跨多个 tokio channel）

## Goals / Non-Goals

**Goals:**
- 统一 span 创建和传递的编码规范
- 提供 trace_id 管理工具函数
- 确保跨 await、跨 channel 的 trace 链路完整
- 日志自动关联 trace_id

**Non-Goals:**
- 不改变现有 OpenTelemetry 配置
- 不修改 Jaeger 集成方式
- 不引入新的 tracing 依赖（除了 `uuid`）

## Decisions

### 1. Span 传递方式：`.instrument()` trait

| 选项 | 优点 | 缺点 | 选择 |
|------|------|------|------|
| `.enter()` + guard | 简单直观 | guard 作用域难控制，跨 await 易断裂 | ❌ |
| `.instrument()` trait | 自动跟随 future，跨 await 安全 | 需要理解 trait bound | ✅ |
| `#[instrument]` 宏 | 最简洁 | 灵活性低，难控制动态字段 | 可选 |

**Rationale**: `.instrument()` 提供最佳平衡 - 完全控制 + 跨 await 安全

### 2. TraceId 生成：UUID v4

- 使用 `uuid::Uuid::new_v4()` 生成标准 trace_id
- 格式：hyphenated（`550e8400-e29b-41d4-a716-446655440000`）
- 不手动拼接 `tr_` 前缀，由日志格式化层处理

### 3. 跨 channel 传播：`WithSpan<T>` + `follows_from()`

```rust
// 发送端 - 创建 span 时注入 trace_id
let trace_id = new_trace_id();
let span = info_span!("datasource_receive",
    source = "deribit",
    trace_id = %trace_id
);
let traced = WithSpan::new(event, span);
tx.send(traced).await?;

// 接收端 - trace_id 自动继承，无需手动传递
let traced = rx.recv().await?;
traced.enter_span(info_span!("rule_evaluate"), |span| {
    span.follows_from(parent_span.id());
    // 处理逻辑
});
```

### 4. Span 命名规范

- 使用 `snake_case`
- 格式：`{stage}_{action}` 如 `datasource_receive`, `rule_evaluate`, `notification_send`

### 5. 字段注入规范

**创建时已知字段** - 直接传入：
```rust
let span = info_span!("datasource_receive",
    source = "deribit",
    trace_id = %trace_id,
    iv = %vol_data.iv
);
```

**创建后才知道的字段** - 用 `record()`：
```rust
let span = info_span!("my_span",
    source = "deribit",
    iv = tracing::field::Empty  // 预声明
);
span.record("iv", &vol_data.iv);  // 运行时注入
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| `.instrument()` 需要理解 trait bound | 提供示例代码和模板 |
| 现有代码需要修改 | 渐进式重构，先测试后推广 |
| `uuid` 新增依赖 | 体积影响小（<50KB） |

## Migration Plan

1. **Step 1**: 增强 `vol-tracing`（已完成）
2. **Step 2**: 修改 `volatility.rs` 作为样板
3. **Step 3**: 推广到其他模块
4. **Step 4**: 更新文档和示例

## Open Questions

- 是否需要在根 span 中自动生成 trace_id？（当前手动创建）
