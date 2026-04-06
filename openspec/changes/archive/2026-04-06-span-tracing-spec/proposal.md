## Why

当前项目中 span + tracing 的使用方式不够统一：部分代码使用 `.enter()` + guard 模式，部分场景没有正确使用 `.instrument()` trait 跨 await 传递 span。这导致 trace 链路在异步边界可能断裂，日志关联不一致。

## What Changes

- **新增**：`vol-tracing` 提供统一的 span 工具函数（`new_trace_id()`, `current_trace_id()`）
- **新增**：Re-export `tracing::Instrument` trait 和 `tracing::instrument` 宏
- **修改**：跨 await 操作统一使用 `.instrument()` trait 而非 `.enter()`
- **新增**：Span 命名和字段注入规范
- **新增**：跨 channel span 传播标准模式（`WithSpan` + `follows_from()`）

## Capabilities

### New Capabilities
- `span-instrument`: 统一使用 `.instrument()` trait 传递 span 的规范
- `trace-id-management`: TraceId 生成、获取、注入的统一工具函数
- `span-lifecycle`: Span 创建、命名、字段注入、作用域管理的编码规范

### Modified Capabilities
- `tracing`: 补充 span 传播和生命周期的具体实现要求

## Impact

- **受影响模块**: `vol-datasource`, `vol-engine`, `vol-notification`, `vol-rules`
- **API 变更**: 无 Breaking Change，只增加工具函数和 re-export
- **依赖**: `vol-tracing` 新增 `uuid` 依赖
