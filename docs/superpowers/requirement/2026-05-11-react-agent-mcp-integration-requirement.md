# Requirements: ReAct Agent MCP Tool Integration

## Background

ReAct agent 当前只能使用项目内硬编码的 Rust 工具（`ExecutableTool` trait）。用户希望通过行业标准方式（`~/.mcp.json` / `.mcp.json`）配置外部 MCP server，让 agent 能够发现、连接并调用 MCP server 提供的 tools。

参考 Claude Desktop 的 `.mcp.json` 配置格式，新建 `vol-llm-mcp` crate 实现 MCP Client 协议层，将 MCP tools 代理为 agent 可使用的 `ExecutableTool`。

## Goals

1. **新建 `vol-llm-mcp` crate**：基于 `rmcp` 实现 MCP Client 协议层，包含配置解析、session 管理、tool 发现与执行
2. **符合 `~/.mcp.json` 标准 schema**：严格遵循 Claude Desktop 配置格式（`mcpServers` → `{server_name}: {command, args, env}`），不做扩展
3. **MCP tools 自动注册到 `ToolRegistry`**：agent 启动时自动连接所有配置的 MCP servers，每个 MCP tool 以 `mcp__{server_name}_{tool_name}` 格式注册为 `Box<dyn ExecutableTool>`
4. **连接失败不阻塞 agent**：MCP server 连接失败时通过 `AgentStreamEvent` 抛出错误事件（tracing 同步记录 error），该 server 的 tools 不注册，其他 tools 正常可用
5. **项目级配置优先**：`.mcp.json`（项目级）覆盖 `~/.mcp.json`（用户级），同名 server 以项目级为准
6. **首期只做 tools 能力**：`tools/list` 和 `tools/call` 两个 MCP method；`resources` / `prompts` 后续扩展

## Non-Goals

1. **不修改 MCP server 配置 schema**：严格遵循 `~/.mcp.json` 格式，不添加自定义字段
2. **不实现 resources / prompts 能力**：首期只支持 tools，后续扩展
3. **不修改 `vol-mcp-servers` crate**：本项目作为独立的 MCP Client 实现，与现有的 MCP Server 集合无关
4. **不实现重试/降级**：MCP 连接或调用失败时不重试，通过 `AgentStreamEvent` 抛出错误事件（tracing 同步记录 error）。失败不阻塞其他 server 或 tool 的正常执行

## Scope

### Included

- **`vol-llm-mcp` crate**：
  - MCP 配置解析（`~/.mcp.json` + `.mcp.json` 合并逻辑，项目级优先）
  - MCP session 生命周期管理（agent 启动连接、agent 停止断开）
  - Tool discovery（`tools/list`）与执行（`tools/call`），基于 `rmcp` crate
  - 支持 STDIO transport（由 `rmcp` 提供）；SSE/Streamable HTTP transport 依赖 `rmcp` client-side 支持情况确认后添加

- **与 ReAct agent 集成**：
  - `McpTool` 结构体实现 `ExecutableTool` trait，代理到 `McpSession`
  - `McpTool` 名称格式：`mcp__{server_name}_{tool_name}`（agent 可感知工具来源，正确路由）
  - `McpSession` 在 agent 构建时初始化，通过 `Arc` 共享，供所有 `McpTool` 实例引用

- **事件系统**：
  - MCP 连接失败时通过 `AgentStreamEvent` 抛出错误事件
  - MCP tool 调用成功/失败的事件记录复用现有 `ToolCallBegin` / `ToolCallComplete` / `ToolCallError` 事件

- **配置加载优先级**：
  1. `.mcp.json`（项目根目录，优先）
  2. `~/.mcp.json`（用户级，fallback）
  3. 两个文件都存在时按 key 合并（per-key replacement），同名 server key 以项目级为准

### Excluded

- MCP server 的动态增删（运行时）
- MCP resources / prompts 能力
- MCP tool 的动态过滤（enable/disable per session）
- MCP tool 调用重试机制

## Constraints

- **依赖 `rmcp` crate**：项目已使用 `rmcp = "1.6"`，直接复用其 transport 层能力（`transport-io` 等 features）
- **错误类型定义**：`vol-llm-mcp` 暴露 `McpError` 枚举，至少包含 `ConnectionFailed`、`ToolCallFailed`、`ServerNotRunning` 变体
- **工具命名规范**：`mcp__{server_name}_{tool_name}`，双下划线前缀区分 MCP tools 与原生 tools；`server_name` 需 sanitize（只保留字母、数字、下划线、连字符）
- **不引入循环依赖**：方向为 `vol-llm-mcp → vol-llm-tool`（McpTool 依赖 ExecutableTool trait），MCP 层不被 tool 层依赖
- **agent 架构兼容**：MCP tools 通过标准 `ToolRegistry::register_boxed()` 注册，不修改 ReAct 循环逻辑
- **McpSession 生命周期**：session 在 `AgentConfig` 构建时创建，绑定到 `RunContext` 生命周期；agent `run()` 结束时 session 断开

## Success Criteria

1. **配置解析正确**：从 `~/.mcp.json` 和 `.mcp.json` 加载配置后，合并逻辑正确（项目级覆盖全局级）
2. **MCP tools 可调用**：agent 能发现 MCP server 的 tools，以 `mcp__{server_name}_{tool_name}` 格式注册到 `ToolRegistry`，agent 在 ReAct 循环中能正常调用并获取结果
3. **连接失败隔离**：某个 MCP server 连接失败时，仅该 server 的 tools 不可用，其他 server 的 tools 和原生 tools 正常
4. **命名唯一性**：多个 server 提供同名工具时，通过 `mcp__{server_name}_{tool_name}` 格式自动区分，不会冲突
5. **`vol-llm-mcp` 可独立编译**：新增 crate 不破坏现有 workspace 编译

## Edge Cases

| Edge Case | Handling |
|-----------|----------|
| `~/.mcp.json` 和 `.mcp.json` 均不存在 | 不连接任何 MCP server，agent 正常运行 |
| 配置文件 JSON 格式错误 | 解析失败，记录 error，不加载任何 MCP server，agent 继续 |
| MCP server `command` 不存在 | 启动失败，抛出 `McpConnectionFailed` 事件，该 server 的 tools 不注册，agent 继续 |
| MCP server 运行中崩溃 | 下次 tool 调用时返回 error，不影响其他 tools |
| MCP server 不提供任何 tools | 视为正常，不报错，该 server 无工具可注册 |
| MCP tool 调用超时 | 超时后返回 `ToolCallError`，agent 继续下一轮推理 |
| 两个 server 同名 | 项目级覆盖全局级，只保留项目级的定义 |

## Architecture Sketch

```
vol-llm-mcp (crate)
├── config.rs        → 配置解析（~/.mcp.json + .mcp.json 合并）
├── session.rs       → McpSession（MCP 协议层，基于 rmcp）
├── tool.rs          → McpTool 实现 ExecutableTool trait
└── transport.rs     → Transport 封装（STDIO / SSE）

依赖方向:
  vol-llm-tool → vol-llm-mcp (McpTool 实现 ExecutableTool)
  vol-llm-mcp  → rmcp (MCP 协议实现)
  vol-llm-agent → vol-llm-mcp (构建时初始化 McpSession)
```

## Open Questions

- `mcp__{server_name}_{tool_name}` 命名中 `server_name` sanitize 的具体规则？（建议：`[^a-zA-Z0-9_-]` → `_`，连续下划线合并为一个）
