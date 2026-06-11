# Requirements: Sandbox Protocol for Agent Server

## Background

`vol-llm-sandbox` 已经定义了完整的 `Sandbox` trait 和四种实现（Local、SSH、Firecracker、Wasm），agent server 内部可以直接使用这些 sandbox 执行命令和文件操作。但缺少一个远程协议，使得**其他 agent 无法通过 agent server 的网络接口在 server 所在的进程环境中执行沙箱命令**。

当前需求：让 agent server 本身成为一个 sandbox 端点——其他 agent 通过 JSON-RPC 协议连接 agent server，将命令执行委托给 server 的本地环境。

## Goals

1. **协议定义**：在 `vol-llm-agent-protocol` 中定义 sandbox 协议的 wire 类型（Operation、Payload 枚举扩展），对齐已有 `Sandbox` trait 的全部操作
2. **服务端处理**：在 `vol-agent-server` 的 data-plane 和 control-plane 中实现 `SandboxHandler`，接收 sandbox 协议请求并在 server 本地进程环境执行
3. **远程代理类型**：在 `vol-agent-server` 中实现一个实现 `Sandbox` trait 的远程代理类型，通过 JSON-RPC 协议调用远端 agent server 的 sandbox 能力
4. **共享沙箱模式**：所有连接到同一 agent server 的 agent 共享同一个沙箱工作区（server 进程环境）

## Non-Goals

- **不搞多层转发**：agent server 不将 sandbox 请求转发到内部挂载的其他 sandbox 实例（如 SSH/Firecracker），直接用 server 的进程环境执行
- **不实现 per-connection 或 per-session 隔离**：不做临时子目录或命名空间隔离
- **不暴露 sandbox 生命周期管理**：协议不需要 `sandbox.start` / `sandbox.cleanup` 操作，生命周期由 agent server 启动时统一管理

## Scope

### Included

| 组件 | 说明 | 位置 |
|------|------|------|
| Sandbox wire 类型 | `SandboxOperation`、`SandboxPayload` 枚举 + JSON-RPC 编解码 | `vol-llm-agent-protocol` |
| SandboxHandler | data-plane + control-plane 的 DomainHandler 实现 | `vol-agent-server` |
| 远程 Sandbox 代理 | 实现 `Sandbox` trait，内部走 JSON-RPC | `vol-agent-server` |
| 集成测试 | 协议 + handler + 远程代理的集成测试 | `vol-agent-server` tests/ |
| 端到端测试 | coding agent 通过远程 sandbox 执行命令的场景 | tests/ 或 examples/ |

### Excluded

- Sandbox trait 本身的修改（已有实现保持不变）
- 新增 sandbox 类型（Local/SSH/Firecracker/Wasm 已足够）
- Sandbox 资源配额/限制管理
- Sandbox 事件流/推送

## Constraints

- **协议 crate 依赖**：`vol-llm-agent-protocol` 新增对 `vol-llm-sandbox` 的依赖（用于引用 `CommandRequest`、`CommandOutput` 等基础类型）
- **无循环依赖**：`vol-llm-sandbox` ← `vol-llm-agent-protocol` ← `vol-agent-server`，单向无环
- **复用现有模式**：遵循 `DomainHandler` trait 和 `HandlerRegistry` 模式注册 handler，不引入新的路由机制
- **JSON-RPC 兼容**：sandbox 操作遵循已有 JSON-RPC 2.0 编解码规范

## Protocol Operations

对齐 `Sandbox` trait 的完整方法集合：

| method_name | 对应 trait 方法 | 描述 |
|-------------|----------------|------|
| `sandbox.list` | - | 列出 agent server 上可用的 sandbox |
| `sandbox.exec` | `execute(CommandRequest) → CommandOutput` | 在 sandbox 中执行命令 |
| `sandbox.read_file` | `read_file(path, offset, limit) → Vec<u8>` | 读取文件内容 |
| `sandbox.write_file` | `write_file(path, content)` | 写入文件 |
| `sandbox.create_dir` | `create_dir_all(path)` | 递归创建目录 |
| `sandbox.read_dir` | `read_dir(path) → Vec<DirEntry>` | 列出目录内容 |
| `sandbox.metadata` | `metadata(path) → FileMetadata` | 获取文件/目录元信息 |

Sandbox 生命周期（start/cleanup）不暴露为协议操作，由 agent server 启动时管理。

## Remote Sandbox Proxy

`AgentServerSandbox`（或类似命名）实现 `Sandbox` trait。其内部：

1. 连接 agent server 的 WebSocket/JSON-RPC 端点
2. `execute()` 等方法内部构造 `AgentServerMessage`，通过 JSON-RPC 发送
3. 将返回的 `Payload` 解析为 `Sandbox` trait 的返回类型
4. 错误映射：`ProtocolError` / 网络错误 → `SandboxError`

```
Agent B 侧                                     Agent Server A 侧
──────────────────────────                    ───────────────────────
AgentServerSandbox                             SandboxHandler
  impl Sandbox trait                                 │
  │                                                  │
  │ execute() → JSON-RPC ───────────────►     收到 sandbox.exec
  │            sandbox.exec                            
  │                                          用 LocalSandbox 执行
  │                                                  │
  │ ◄──────── JSON-RPC ──────────────────     返回 CommandOutput
  │          sandbox.exec 结果
```

## Success Criteria

1. `cargo build -p vol-agent-server -p vol-llm-agent-protocol` 编译通过
2. `cargo test -p vol-agent-server` 中 sandbox 相关测试全部通过
3. 集成测试验证：启动 agent server → `AgentServerSandbox` 连接 → 调用 `execute("echo hello")` → 得到 `exit_code=0, stdout="hello"`
4. 集成测试验证：`read_file` / `write_file` / `create_dir_all` / `read_dir` / `metadata` 全部通过远程协议正确工作
5. 端到端测试：coding agent 通过远程 sandbox 执行 `cargo build`（或等效命令），验证完整链路
6. Control-plane 模式也通过集成测试
7. 无循环依赖检查通过：`./scripts/check-agent-boundaries.sh` 不变红（如有新增依赖需更新脚本）

## Edge Cases

| 边界情况 | 处理策略 |
|----------|----------|
| 命令执行超时 | 使用 `CommandRequest.timeout`，超时后 kill 进程组返回 `SandboxError::Timeout` |
| 大文件读写 | `read_file` 已有 offset/limit 分段能力；`write_file` 全量写入，不做流式分片 |
| 并发命令执行 | 共享沙箱模式：多个 agent 并发执行命令由 OS 进程隔离自然管理 |
| Path traversal 攻击 | 复用 `Sandbox::resolve_path()` 的 traversal 检测，拒绝 `..` 和绝对路径 |
| Agent server 未启动 | 远程代理连接失败返回 `SandboxError::NotStarted` 或 IO error |
| 网络断连 | JSON-RPC 连接断开后，后续调用返回 IO error；需要上层重连逻辑 |
| 无效 sandbox 名称 | `sandbox.list` 返回实际可用列表；指定不存在的 sandbox 返回错误 |
| 空 stdin / 空命令 | 空命令由 `Sandbox::execute()` 底层处理（spawn 失败返回 OS error） |

## Open Questions

无。所有设计决策已在澄清阶段确定。

## Dependencies

- `vol-llm-sandbox` — Sandbox trait + 基础类型（CommandRequest, CommandOutput, SandboxError, etc.）
- `vol-llm-agent-protocol` — 新增 sandbox wire 类型，新增对 sandbox crate 的依赖
- `vol-agent-server` — SandboxHandler + 远程代理类型，已有 sandbox + protocol 依赖
