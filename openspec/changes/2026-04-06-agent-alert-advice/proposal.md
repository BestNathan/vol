# Proposal: Agent Alert Advice

## Context

当前的波动率预警系统工作流程为：
```
DataSource → Rule → AlertManager → Notification → 飞书
```

预警通知直接发送，缺少以下能力：
1. **智能分析**：无法结合历史数据判断预警的严重性
2. **决策建议**：用户收到预警后不知道该如何行动
3. **上下文关联**：单一预警无法与历史趋势关联

**约束条件**：
- Rust 项目，tokio 异步运行时
- 已有 vol-llm-agent、vol-llm-tool、vol-llm-provider crate
- 已有 TDengine 存储历史数据（192.168.2.106:6041）
- 已有飞书通知集成
- **不影响现有代码逻辑和结构**

## Goals / Non-Goals

**Goals:**
- 在预警发送后，异步推送 AI 分析建议
- 结合 TDengine 历史数据做趋势分析
- 通过飞书发送结构化建议（风险等级 + 分析 + 建议）
- 独立的频率限制，避免过度分析
- 完全解耦，不影响现有引擎和通知逻辑

**Non-Goals:**
- 不自动执行交易操作（仅建议）
- 不修改现有 AlertManager 的 cooldown 逻辑
- 不改变现有通知渠道
- 不引入新的 LLM Provider

## Decisions

### 1. 架构模式：独立 Agent Service

| 选项 | 优点 | 缺点 | 选择 |
|------|------|------|------|
| NotificationHandler 扩展 | 代码改动小 | 耦合度高，LLM 调用可能阻塞通知 | ❌ |
| 独立 Agent Service | 完全解耦，可独立启停，可扩展 | 新增 crate | ✅ |
| Sidecar 外部进程 | 完全独立部署 | 增加网络延迟，部署复杂 | ❌ |

**Rationale**: 独立 Service 提供最佳解耦，符合现有插件架构

### 2. 集成点：AlertManager 之后，独立订阅

```
AlertManager → broadcast → [NotificationHandler, AgentAdviceService]
```

- AgentAdviceService 独立订阅 alert broadcast channel
- 不影响现有 NotificationHandler 流程
- 频率限制独立管理

### 3. 频率限制策略

**维度**: `symbol:alert_type` 组合

**限制**:
- 同一组合每 5 分钟最多分析 1 次
- 每小时最多 20 次全局分析

**配置**:
```toml
[agent_advice]
enabled = true
cooldown_secs = 300        # 5 分钟
max_analyses_per_hour = 20
```

### 4. 数据上下文

**查询范围**:
- Alert 本身数据（IV、symbol、tenor 等）
- TDengine 查询过去 1 小时、24 小时历史 IV
- TDengine 查询当前市场价格

**不查询**:
- 复杂聚合（如 skew、term structure）- 避免过度延迟

### 5. Agent Prompt 设计

**角色**: 衍生品市场风险分析师

**输出格式**:
```
🔔 预警分析建议

预警：{alert_type} - {symbol}
当前 IV: {iv} (阈值：{threshold})

📊 历史数据分析:
- 过去 1 小时 IV 变化 {change_1h}%
- 过去 24 小时 IV 分位数：{percentile_24h}%

⚠️ 风险等级：{高/中/低}

💡 建议:
{具体操作建议}
```

## Risks / Trade-offs

| Risk | Mitigation |
|------|------------|
| LLM API 调用延迟 | 异步处理，不阻塞主流程 |
| LLM API 不可用 | 优雅降级，记录日志，不影响预警发送 |
| 频率限制过严 | 可配置，支持动态调整 |
| 历史数据查询失败 |  fallback 到仅基于 Alert 的简单分析 |
| 飞书消息过长 | 结构化输出，控制在 500 字内 |

## Migration Plan

1. **Step 1**: 创建 `vol-llm-bridge` crate
2. **Step 2**: 实现 `FrequencyLimiter`
3. **Step 3**: 实现 `AgentAdviceService`
4. **Step 4**: 集成到 `vol-monitor` 主程序
5. **Step 5**: 配置和测试

## Open Questions

- 是否需要支持多种 LLM 模型配置？
- 是否需要将分析建议持久化到数据库？
