## ADDED Requirements

### Requirement: TraceId 生成工具

`vol-tracing` 提供统一的 trace_id 生成函数，使用 UUID v4 标准。

#### Scenario: 生成新的 trace_id
- **WHEN** 代码需要创建新的 trace_id
- **THEN** 调用 `vol_tracing::new_trace_id()`
- **THEN** 返回格式为 hyphenated UUID（如 `550e8400-e29b-41d4-a716-446655440000`）

### Requirement: 当前 TraceId 获取

提供函数从当前 span context 中提取 trace_id，用于日志和调试。

#### Scenario: 获取当前 trace_id
- **WHEN** 代码需要获取当前活跃的 trace_id
- **THEN** 调用 `vol_tracing::current_trace_id()`
- **THEN** 无活跃 span 时返回空字符串
