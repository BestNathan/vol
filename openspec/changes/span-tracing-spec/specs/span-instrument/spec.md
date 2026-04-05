## ADDED Requirements

### Requirement: 使用 `.instrument()` trait 传递 Span

跨异步边界的 span 传递统一使用 `tracing::Instrument` trait 的 `.instrument()` 方法，而非 `.enter()` + guard 模式。

#### Scenario: 异步操作使用 instrument
- **WHEN** 代码需要执行跨 `await` 的操作
- **THEN** 使用 `.instrument(span).await` 而非 `let _guard = span.enter()`

#### Scenario: Channel 发送使用 instrument
- **WHEN** 通过 mpsc channel 发送消息
- **THEN** 使用 `tx.send(msg).instrument(span).await`

#### Scenario: 函数返回值绑定 span
- **WHEN** 需要将 future 绑定到 span
- **THEN** 使用 `async { ... }.instrument(span)` 模式
