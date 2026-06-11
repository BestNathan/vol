# Architecture: Agent Server Control Plane / Data Plane

**Date**: 2026-06-10
**Status**: Draft
**Author**: Claude
**Source**: Codebase analysis; prior spec `docs/superpowers/specs/2026-05-02-agent-manager-control-plane-design.md`; current runtime/server specs `docs/superpowers/specs/2026-05-24-agent-teamwork-design.md`, `docs/superpowers/specs/2026-06-05-agent-server-design.md`

## Requirements

用户目标：给 agent server 设计一套控制面与数据面架构。

### Goals

- `vol-agent-server` 作为唯一 server binary crate，不新增独立 control-plane crate。
- `vol-llm-agent-channel` 只负责协议层、JSON-RPC over WebSocket transport、连接抽象、handler/dispatch 抽象。
- `vol-agent-server` 负责具体 server 实现：`DataPlaneServerCore` 和 `ControlPlaneServerCore`，并根据配置组织 standalone data-plane、standalone control-plane、combined control+data-plane 三种运行模式。
- 控制面和数据面的 wire protocol 统一为 JSON-RPC 2.0 over WebSocket，复用同一套 codec、method routing、response/error 和 notification 语义。
- 数据面节点可以通过 JSON-RPC WebSocket 向控制面注册，并持续上报 agents、tools、MCP servers/tools/resources/prompts、skills、运行状态、事件和指标。
- 保持 `AgentRuntime` 作为执行资源单一事实来源；不要让 `DataPlaneServerCore`、`ControlPlaneServerCore` 或 control-plane 状态重复组装 tools/skills/MCP/providers。

### Non-goals

- 不新增 `vol-agent-control-plane` crate；control-plane 实现在 `vol-agent-server::control_plane`。
- 不在 `vol-agent-server` 定义 wire-level protocol types；所有 JSON-RPC method、payload、error shape、codec 属于 `vol-llm-agent-channel`。
- 不让 `vol-llm-runtime` 感知控制面存在；runtime 只暴露可被 snapshot 的执行资源状态。
- 不在第一阶段实现多控制面 HA、Raft/etcd 一致性、完整 RBAC、多租户隔离或审计报表。
- 不要求替换现有 JSON-RPC WebSocket 客户端协议；`agent.*` / `tool.*` / `mcp.*` 方法继续兼容，并增加 `control.*` 方法。

## Current State

当前 agent server 是单进程形态：`vol-agent-server` 加载 TOML 配置，构建 `AgentServerCore`，调用 `discover_agents()`，再用 JSON-RPC WebSocket 暴露 `/ws`。

关键现状：

- `AgentRuntime` 是共享 agent 资源的权威所有者，字段包含 provider registry、`ToolRegistry`、`TaskStore`、`SessionManager`、`McpManager`、`SkillLoader`、agent definitions 和 agent status（`crates/vol-llm-runtime/src/lib.rs:61`）。
- `AgentRuntime::register_agent()` 构建 `ReActAgent`，注入 runtime-owned tool/session/MCP 资源（`crates/vol-llm-runtime/src/lib.rs:118`）。
- 当前 `AgentServerCore` 位于 `vol-llm-agent-channel`，但它已经是具体 data-plane server 实现：包装 runtime、注册 domain handlers、维护 local `AgentRouter`、`ConnectionHolder` 和 agent dispatchers（`crates/vol-llm-agent-channel/src/server_core.rs:67`）。
- `DomainHandler`/`HandlerRegistry` 是当前最自然的 RPC dispatch 抽象（`crates/vol-llm-agent-channel/src/domain/handler.rs:8`）。它们应保留在 channel crate。
- `Operation` 当前把控制面式目录/状态方法和数据面式执行方法混在同一协议命名空间里，例如 `agent.list`、`agent.status`、`agent.submit`、`tool.call`、`mcp.call_tool`（`crates/vol-llm-agent-channel/src/agent_server_protocol.rs:37`）。后续应扩展 `Operation::Control(ControlOperation)`。
- `JsonRpcServer` 当前绑定 `AgentServerCore`（`crates/vol-llm-agent-channel/src/transport/jsonrpc/server.rs`），后续应改成绑定通用 service trait，以便 data-plane core 和 control-plane core 都能复用同一 transport。

## Architecture

最终架构是 **single server crate, split cores, shared channel protocol**：

- **`vol-llm-agent-channel`**：协议与通信抽象层。拥有 `AgentServerMessage`、`Operation`、`Payload`、`control.*` 方法、JSON-RPC codec、`JsonRpcConnection`、generic `JsonRpcServer<S>`、`DomainHandler`、`HandlerRegistry`、`JsonRpcMessageService`。
- **`vol-agent-server`**：具体 server 实现与进程编排层。拥有 `DataPlaneServerCore`、`ControlPlaneServerCore`、control-plane 状态（registry/index/router/lease/event）、data-plane reporter/client/snapshot/command executor、config-driven role composition。
- **`vol-llm-runtime`**：执行资源层。拥有 providers、tools、MCP、skills、task/session stores、agent definitions/status，不依赖 control plane。

```
┌──────────────────────────────────────────────────────────────────────┐
│                          vol-agent-server                            │
│                                                                      │
│  ┌──────────────────────────┐        ┌────────────────────────────┐  │
│  │ ControlPlaneServerCore   │        │ DataPlaneServerCore        │  │
│  │ - NodeRegistry           │        │ - AgentRuntime             │  │
│  │ - CapabilityIndex        │        │ - AgentRouter              │  │
│  │ - LeaseManager           │        │ - AgentDispatcher          │  │
│  │ - ControlRouter          │        │ - ConnectionHolder         │  │
│  │ - EventBus               │        │ - data-plane handlers      │  │
│  │ - control-plane handlers │        │ - DataPlaneReporter        │  │
│  └─────────────┬────────────┘        └──────────────┬─────────────┘  │
│                │                                    │                │
│                │ uses shared protocol/transport     │ uses runtime   │
└────────────────┼────────────────────────────────────┼────────────────┘
                 │                                    │
                 ▼                                    ▼
┌────────────────────────────────────┐    ┌───────────────────────────┐
│        vol-llm-agent-channel        │    │      vol-llm-runtime      │
│ - protocol::{Operation, Payload}    │    │ - ToolRegistry           │
│ - protocol::control::*              │    │ - McpManager             │
│ - transport::jsonrpc::*             │    │ - SkillLoader            │
│ - Connection trait                  │    │ - TaskStore              │
│ - DomainHandler / HandlerRegistry   │    │ - SessionManager         │
│ - JsonRpcMessageService trait       │    │ - Agent defs/status      │
└────────────────────────────────────┘    └───────────────────────────┘

Client / UI / CLI  ── JSON-RPC WS /ws ──► ControlPlaneServerCore
Data-plane node    ── JSON-RPC WS /control/v1/ws ──► ControlPlaneServerCore
Standalone client  ── JSON-RPC WS /ws ──► DataPlaneServerCore
```

### Design Principles

1. **Channel owns protocol, server owns implementation**: `vol-llm-agent-channel` defines all wire contracts and transport abstractions. `vol-agent-server` must not define JSON-RPC wire payloads.
2. **Server core is concrete, not protocol**: `DataPlaneServerCore` and `ControlPlaneServerCore` belong in `vol-agent-server` because they own runtime/control state and process behavior.
3. **Runtime stays authoritative**: data-plane capability snapshots are derived from `AgentRuntime`; control plane stores observations, not runtime objects.
4. **JSON-RPC is the only application protocol**: client↔control-plane and control-plane↔data-plane both use JSON-RPC 2.0 over WebSocket. HTTP is reserved for `/health` and `/metrics` only.
5. **Config chooses role composition**: the same `vol-agent-server` binary can run as standalone data-plane, standalone control-plane, or combined control+data-plane.
6. **Migration can be staged**: first make JSON-RPC server generic, then move concrete data-plane core into `vol-agent-server`, then add `ControlPlaneServerCore`.

## Component Breakdown

### 1. `vol-llm-agent-channel` Protocol Layer

Purpose: provide reusable protocol and transport primitives for both data-plane and control-plane services.

Responsibilities:

- Define all JSON-RPC method names: existing `agent.*`, `tool.*`, `mcp.*`, `task.*`, `session.*`, plus new `control.*`.
- Define `Operation`, `Payload`, `AgentServerMessage`, error payloads, params/results for all domains.
- Decode/encode JSON-RPC frames into/from `AgentServerMessage`.
- Provide transport abstractions: `Connection`, `JsonRpcConnection`, generic `JsonRpcServer<S>`.
- Provide dispatch abstractions: `DomainHandler`, `HandlerRegistry`, and `JsonRpcMessageService`.
- Not own runtime, node registry, capability index, placement policy, or concrete agent execution.

### 2. `DataPlaneServerCore` (`vol-agent-server::data_plane`)

Concrete server core for local agent execution. This is the final home for the current `AgentServerCore` behavior.

Responsibilities:

- Build and own an `AgentRuntime` using `AgentRuntimeBuilder`.
- Register local data-plane handlers: agent, tool, MCP, skill, task, session, file, log, system.
- Own local `AgentRouter`, `AgentDispatcher`, and `ConnectionHolder` because these are execution-layer concepts, not protocol concepts.
- Discover/register agents from `.agents/agents`.
- Serve standalone JSON-RPC `/ws` when control-plane mode is disabled.
- Execute `ControlCommand`s received from a control plane in data-plane mode.
- Feed `DataPlaneReporter` capability snapshots and agent stream events.

### 3. `ControlPlaneServerCore` (`vol-agent-server::control_plane`)

Concrete server core for cluster coordination.

Responsibilities:

- Own `ControlPlaneState`: `NodeRegistry`, `CapabilityIndex`, `LeaseManager`, `ControlRouter`, `EventBus`, `RunStore`.
- Register control-plane handlers for `control.register`, `control.heartbeat`, `control.capability_snapshot`, `control.capability_delta`, `control.event`, `control.command_result`, and client-facing catalog/run methods.
- Serve client JSON-RPC `/ws` and node JSON-RPC `/control/v1/ws` through the generic channel JSON-RPC transport.
- Track connected node sessions and dispatch `control.command` JSON-RPC requests to selected nodes.
- Publish node/run/capability events as JSON-RPC notifications.

### 4. `DataPlaneReporter` and `ControlPlaneClient` (`vol-agent-server::data_plane`)

Embedded data-plane components used when `[data_plane.control_plane]` is enabled.

Responsibilities:

- Connect to control-plane `/control/v1/ws` using channel JSON-RPC client/connection primitives.
- Send `control.register`, `control.heartbeat`, `control.capability_snapshot`, `control.capability_delta`, and `control.event`.
- Receive `control.command` requests, execute them against `DataPlaneServerCore`, and return JSON-RPC result/error plus terminal notifications where appropriate.
- Deduplicate by JSON-RPC request id and `command_id`.

### 5. `ServerApp` / role composition (`vol-agent-server::app`)

Process-level composition based on config.

Responsibilities:

- Parse config and decide which role(s) to start.
- Build `DataPlaneServerCore` when data-plane mode is enabled.
- Build `ControlPlaneServerCore` when control-plane mode is enabled.
- Mount `/ws` to control-plane core when control-plane is enabled; otherwise mount `/ws` to data-plane core for standalone compatibility.
- Mount `/control/v1/ws` only when control-plane is enabled.
- Start background lease scanner and data-plane reporter tasks.

## Crate / File Structure

```
crates/
├── vol-llm-agent-channel/
│   └── src/
│       ├── protocol/
│       │   ├── mod.rs
│       │   ├── operation.rs              # Operation enum incl. Control(ControlOperation)
│       │   ├── payload.rs                # Payload enum incl. Control(ControlPayload)
│       │   ├── agent.rs                  # agent.* operations/payloads
│       │   ├── tool.rs                   # tool.* operations/payloads
│       │   ├── mcp.rs                    # mcp.* operations/payloads
│       │   ├── task.rs                   # task.* operations/payloads
│       │   ├── session.rs                # session.* operations/payloads
│       │   ├── control.rs                # control.* operations/payloads
│       │   └── error.rs                  # JSON-RPC-compatible error payloads
│       ├── transport/
│       │   └── jsonrpc/
│       │       ├── mod.rs
│       │       ├── codec.rs              # JSON-RPC <-> AgentServerMessage
│       │       ├── connection.rs         # JsonRpcConnection implements Connection
│       │       └── server.rs             # generic JsonRpcServer<S>
│       ├── domain/
│       │   ├── handler.rs                # DomainHandler trait
│       │   └── registry.rs               # HandlerRegistry
│       ├── connection.rs                 # Connection trait
│       ├── service.rs                    # JsonRpcMessageService trait
│       └── lib.rs
│
├── vol-agent-server/
│   └── src/
│       ├── main.rs                       # binary entrypoint
│       ├── config.rs                     # roles + runtime + control/data-plane config
│       ├── app.rs                        # build role cores and background tasks
│       ├── routes.rs                     # /ws, /control/v1/ws, /health, /metrics
│       ├── health.rs
│       ├── data_plane/
│       │   ├── mod.rs
│       │   ├── core.rs                   # DataPlaneServerCore (current AgentServerCore home)
│       │   ├── builder.rs                # builds AgentRuntime + handlers
│       │   ├── router.rs                 # local AgentRouter
│       │   ├── dispatcher.rs             # AgentDispatcher
│       │   ├── connection_holder.rs      # AgentPlugin event bridge
│       │   ├── handlers/                 # concrete data-plane DomainHandlers
│       │   │   ├── agent.rs
│       │   │   ├── tool.rs
│       │   │   ├── mcp.rs
│       │   │   ├── skill.rs
│       │   │   ├── task.rs
│       │   │   ├── session.rs
│       │   │   ├── file.rs
│       │   │   ├── log.rs
│       │   │   └── system.rs
│       │   ├── reporter.rs               # reports runtime state to control plane
│       │   ├── client.rs                 # JSON-RPC control-plane client
│       │   ├── snapshot.rs               # AgentRuntime -> CapabilitySnapshot
│       │   └── command.rs                # ControlCommand -> local execution
│       └── control_plane/
│           ├── mod.rs
│           ├── core.rs                   # ControlPlaneServerCore
│           ├── builder.rs
│           ├── state.rs                  # ControlPlaneState
│           ├── registry.rs               # NodeRegistry
│           ├── capability.rs             # CapabilityIndex
│           ├── lease.rs                  # LeaseManager
│           ├── router.rs                 # ControlRouter
│           ├── event.rs                  # EventBus
│           ├── store.rs                  # in-memory now, DB later
│           └── handlers/                 # concrete control-plane DomainHandlers
│               ├── control.rs
│               ├── node.rs
│               ├── capability.rs
│               └── run.rs
│
└── vol-llm-runtime/
    └── src/lib.rs                        # AgentRuntime remains execution resource owner
```

## Key Types

### `Operation` and `ControlOperation` (`vol-llm-agent-channel`)

```rust
/// Protocol operation namespace for every JSON-RPC method supported by the channel.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Operation {
    Agent(AgentOperation),
    File(FileOperation),
    Session(SessionOperation),
    Mcp(McpOperation),
    Skill(SkillOperation),
    Tool(ToolOperation),
    Log(LogOperation),
    System(SystemOperation),
    Task(TaskOperation),
    Control(ControlOperation),
}

/// Control-plane/data-plane JSON-RPC methods.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ControlOperation {
    Register,
    Heartbeat,
    CapabilitySnapshot,
    CapabilityDelta,
    Event,
    Command,
    CommandAck,
    CommandResult,
    NodeList,
    NodeGet,
    CapabilityList,
    RunStatus,
}
```

### `ControlPayload` (`vol-llm-agent-channel`)

```rust
/// Params/results for `control.*` JSON-RPC methods.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ControlPayload {
    Register(NodeRegistration),
    RegisterAck(RegisterAck),
    Heartbeat(NodeHeartbeat),
    CapabilitySnapshot(CapabilitySnapshot),
    CapabilityDelta(CapabilityDelta),
    Event(DataPlaneEvent),
    Command(ControlCommand),
    CommandAck(CommandAck),
    CommandResult(CommandResult),
    NodeList(NodeListRequest),
    NodeListResult(NodeListResult),
    CapabilityList(CapabilityListRequest),
    CapabilityListResult(CapabilityListResult),
    RunStatus(RunStatusRequest),
    RunStatusResult(RunStatusResult),
}
```

### `JsonRpcMessageService` (`vol-llm-agent-channel`)

```rust
/// Generic service abstraction consumed by JSON-RPC WebSocket transport.
///
/// Concrete implementations live in `vol-agent-server`:
/// - `DataPlaneServerCore`
/// - `ControlPlaneServerCore` / role-specific endpoint wrappers
#[async_trait::async_trait]
pub trait JsonRpcMessageService: Send + Sync + 'static {
    /// Serve one already-upgraded JSON-RPC connection.
    async fn serve_connection(&self, conn: std::sync::Arc<dyn Connection>);
}
```

### `JsonRpcServer<S>` (`vol-llm-agent-channel`)

```rust
/// Generic JSON-RPC over WebSocket server.
///
/// This type owns transport only. It must not know whether the service is a
/// data-plane server, a control-plane client endpoint, or a control-plane node endpoint.
pub struct JsonRpcServer<S> {
    service: std::sync::Arc<S>,
    path: &'static str,
}

impl<S> JsonRpcServer<S>
where
    S: JsonRpcMessageService,
{
    pub fn new(service: std::sync::Arc<S>, path: &'static str) -> Self {
        Self { service, path }
    }

    pub fn into_axum_router(self) -> axum::Router {
        // Mount `self.path`, upgrade WebSocket, create JsonRpcConnection,
        // delegate to `service.serve_connection(conn)`.
        todo!()
    }
}
```

### `DataPlaneServerCore` (`vol-agent-server`)

```rust
/// Concrete data-plane server core for local agent execution.
///
/// Owns AgentRuntime and local execution machinery. Replaces the current
/// channel-crate `AgentServerCore` as the concrete server implementation.
pub struct DataPlaneServerCore {
    pub runtime: vol_llm_runtime::AgentRuntime,
    handler_registry: vol_llm_agent_channel::domain::HandlerRegistry,
    router: AgentRouter,
    holders: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, std::sync::Arc<ConnectionHolder>>>>,
}

#[async_trait::async_trait]
impl vol_llm_agent_channel::JsonRpcMessageService for DataPlaneServerCore {
    async fn serve_connection(&self, conn: std::sync::Arc<dyn vol_llm_agent_channel::Connection>) {
        // Attach connection holders, receive messages, dispatch to data-plane handlers,
        // send JSON-RPC responses/notifications through the channel transport.
    }
}
```

### `ControlPlaneServerCore` (`vol-agent-server`)

```rust
/// Concrete control-plane server core for node registry, capability index,
/// routing, leases, and event fan-out.
pub struct ControlPlaneServerCore {
    state: std::sync::Arc<ControlPlaneState>,
    handler_registry: vol_llm_agent_channel::domain::HandlerRegistry,
}

/// Role wrapper for the same control-plane core mounted at different endpoints.
pub enum ControlConnectionRole {
    Client,
    DataPlaneNode,
}

pub struct ControlPlaneEndpoint {
    core: std::sync::Arc<ControlPlaneServerCore>,
    role: ControlConnectionRole,
}

#[async_trait::async_trait]
impl vol_llm_agent_channel::JsonRpcMessageService for ControlPlaneEndpoint {
    async fn serve_connection(&self, conn: std::sync::Arc<dyn vol_llm_agent_channel::Connection>) {
        self.core.serve_connection_with_role(self.role, conn).await;
    }
}
```

### `ServerRoles` (`vol-agent-server`)

```rust
/// Process role selection for the single `vol-agent-server` binary.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ServerRoles {
    /// Enable client-facing control-plane APIs and data-plane node endpoint.
    #[serde(default)]
    pub control_plane: bool,

    /// Enable local data-plane runtime and agent execution.
    #[serde(default = "default_true")]
    pub data_plane: bool,
}
```

## Protocol Mapping

All rows below are JSON-RPC methods over WebSocket. Client-facing and data-plane-facing APIs share framing, error semantics, request ids, and notification behavior through `vol-llm-agent-channel`.

| JSON-RPC method | Direction | Control-plane view | Concrete owner |
|-----------------|-----------|-------------------|----------------|
| `control.register` | node → control request | create/update `NodeRecord`; response is `RegisterAck` | `ControlPlaneServerCore` handler |
| `control.heartbeat` | node → control notification | update lease/load/status | `ControlPlaneServerCore` handler |
| `control.capability_snapshot` | node → control notification/request | replace indexed capabilities for `(node_id, revision)` | `ControlPlaneServerCore` handler |
| `control.capability_delta` | node → control notification | patch indexed capabilities for one node | `ControlPlaneServerCore` handler |
| `control.event` | node → control notification | append to `EventBus` / update `RunStore` | `ControlPlaneServerCore` handler |
| `control.command` | control → node request | dispatch selected operation to connected node | `DataPlaneServerCore` command executor |
| `agent.list` | client → control request or standalone data-plane request | query `CapabilityIndex` in control mode; local list in standalone mode | control/data handler by route mode |
| `agent.submit` | client → control request or standalone data-plane request | route to node in control mode; execute local in standalone mode | `ControlRouter` or data-plane `AgentHandler` |
| `tool.call` | client → control request or standalone data-plane request | route `control.command(CallTool)` in control mode; execute local in standalone mode | `ControlRouter` or data-plane `ToolHandler` |
| `mcp.call_tool` | client → control request or standalone data-plane request | route `control.command(CallMcpTool)` in control mode; execute local in standalone mode | `ControlRouter` or data-plane `McpHandler` |

## Runtime Modes and Routes

| Mode | Config | Routes |
|------|--------|--------|
| Standalone data-plane | `control_plane=false`, `data_plane=true` | `/ws -> DataPlaneServerCore`, `/health`, `/metrics` |
| Standalone control-plane | `control_plane=true`, `data_plane=false` | `/ws -> ControlPlaneEndpoint(Client)`, `/control/v1/ws -> ControlPlaneEndpoint(DataPlaneNode)`, `/health`, `/metrics` |
| Combined | `control_plane=true`, `data_plane=true` | `/ws -> ControlPlaneEndpoint(Client)`, `/control/v1/ws -> ControlPlaneEndpoint(DataPlaneNode)`, local `DataPlaneReporter` connects to `/control/v1/ws` or registers in-process |

For the MVP, combined mode should prefer loopback JSON-RPC registration (`ws://127.0.0.1:{port}/control/v1/ws`) because it verifies the same protocol path used by remote data-plane nodes. An in-process shortcut can be added later as an optimization.

## Data Flow

### Primary Flow: Data-plane node startup and registration

1. `vol-agent-server` loads `[server.roles]`, `[runtime]`, `[control_plane]`, and `[data_plane]` config.
2. If data-plane is enabled, it builds `DataPlaneServerCore`, which builds `AgentRuntime` and discovers local agents.
3. If control-plane registration is enabled for the data plane, `ControlPlaneClient` opens JSON-RPC WebSocket to `/control/v1/ws`.
4. Data plane sends JSON-RPC request `control.register` with `NodeRegistration` params.
5. `ControlPlaneServerCore` validates auth, stores/updates `NodeRecord`, and returns `RegisterAck`.
6. `DataPlaneReporter` sends `control.capability_snapshot` derived from `AgentRuntime` and `DataPlaneServerCore` state.
7. Control plane indexes the snapshot in `CapabilityIndex` and marks node `Online`.
8. Data plane sends periodic `control.heartbeat` notifications.

### Primary Flow: Control plane submits an agent run

1. Client calls control-plane `/ws` JSON-RPC method `agent.submit` with optional `node_id`/`target`.
2. `ControlPlaneServerCore` dispatches to the run handler; `ControlRouter` resolves placement using `CapabilityIndex`, node health, and policy.
3. Control plane creates/updates a `RunRecord` and sends JSON-RPC request `control.command` to the selected node session.
4. Data-plane node receives `control.command`, maps it to local execution through `DataPlaneServerCore::execute_control_command`.
5. Local execution reuses data-plane handlers/`AgentRouter`/`AgentDispatcher`.
6. Data plane returns the `control.command` response when accepted, then sends `control.event` and `control.command_result` notifications.
7. Control plane updates `RunStore` and republishes lifecycle events to subscribed clients as JSON-RPC notifications.

### Secondary Flow: Standalone data-plane request

1. If `control_plane=false`, `/ws` is mounted directly to `DataPlaneServerCore`.
2. Client sends existing `agent.submit`, `tool.call`, or `mcp.call_tool` JSON-RPC request.
3. `DataPlaneServerCore` dispatches the request to local data-plane handlers exactly as the current server does.
4. No control-plane registry, placement, or capability index is involved.

## Edge Cases

| Edge Case | Behavior |
|-----------|----------|
| Both roles disabled | Config validation fails before binding sockets. |
| Control-plane and data-plane both enabled | `/ws` belongs to `ControlPlaneServerCore`; local data-plane registers through `/control/v1/ws` loopback by default. |
| Existing standalone clients expect `/ws` data-plane semantics | Preserve when `control_plane=false`; when `control_plane=true`, `/ws` is a control-plane endpoint and may route to local/remote nodes. |
| Node connects but does not send `control.register` | Close connection after registration timeout; no `NodeRecord` is created. |
| Node registers before MCP/skills finish loading | Accept registration; later `control.capability_snapshot` or `control.capability_delta` updates index. |
| Duplicate `node_id` while old connection is alive | Accept only if auth identity matches and generation is newer; close old connection with superseded reason. |
| Heartbeat timeout | `LeaseManager` marks node `Dead`, removes it from placement candidates, keeps capabilities as stale for inspection. |
| Duplicate JSON-RPC request id / command delivery after reconnect | Data plane uses JSON-RPC `id` plus `command_id` cache and returns the existing result where available. |
| Protocol type needed by both cores | Add it to `vol-llm-agent-channel::protocol`, never to `vol-agent-server`. |
| Data-plane execution type depends on `ReActAgent` | Keep it in `vol-agent-server::data_plane`, not in channel protocol. |

## Configuration

```toml
[server]
host = "0.0.0.0"
port = 3001

[server.roles]
control_plane = true
data_plane = true

[runtime]
working_dir = "."
store_dir = "~/.vol"

[control_plane]
auth_token = "dev-token"
client_ws_path = "/ws"
node_ws_path = "/control/v1/ws"
lease_timeout_secs = 90
lease_scan_secs = 15

[data_plane]
node_id = "local-dev-node"
name = "Local Development Node"
control_url = "ws://127.0.0.1:3001/control/v1/ws"
heartbeat_secs = 15
snapshot_on_connect = true
```

## Persistence Strategy

### MVP

- `ControlPlaneState`: in-memory `NodeRegistry`, `CapabilityIndex`, `RunStore`, and command-result cache inside `vol-agent-server::control_plane`.
- Durable task/session data remains in data-plane runtime stores (`TaskStore`, `SessionManager`).
- Protocol state is not persisted by `vol-llm-agent-channel`.

### Future

- Add optional SeaORM-backed control-plane store under `vol-agent-server::control_plane::store` for nodes, capabilities, run records, and event history.
- Reuse existing database configuration style from runtime store configs where possible.
- Keep control-plane persistence independent from data-plane `TaskStore`/`SessionManager`.

## Security and Policy

MVP security:

- Bearer token during JSON-RPC WebSocket upgrade for client-facing `/ws` and data-plane `/control/v1/ws` connections.
- Node identity is `node_id + auth identity`; reconnect with same node id but different auth identity is rejected.
- Control commands carry JSON-RPC `id`, `command_id`, target `node_id`, and optional deadline.
- Control plane records command issuer metadata when client auth is available.

Future policy seams live in `vol-agent-server::control_plane`:

- `PlacementPolicy`: which nodes can run which agents/tools/MCP calls.
- `ApprovalPolicy`: whether sensitive `tool.call`/`mcp.call_tool` requires approval.
- `AuditSink`: append-only command/event audit log.
- `NodeAdmissionPolicy`: validate node labels/version/capabilities before accepting it into the routing pool.

## Migration Plan

1. In `vol-llm-agent-channel`, add `JsonRpcMessageService` and make `JsonRpcServer` generic over that trait instead of depending on `AgentServerCore`.
2. Move protocol definitions into `vol-llm-agent-channel::protocol`, then add `ControlOperation`, `ControlPayload`, and `control.*` JSON-RPC method mapping.
3. Move concrete data-plane implementation from `vol-llm-agent-channel` to `vol-agent-server::data_plane`: current `AgentServerCore`, data-plane domain handlers, local router/dispatcher, and connection-holder plugin. Keep temporary type aliases/re-exports if needed during migration.
4. Add `ControlPlaneServerCore` under `vol-agent-server::control_plane` with in-memory registry, capability index, router, lease manager, event bus, and concrete control-plane handlers.
5. Extend `vol-agent-server` config with `[server.roles]`, `[control_plane]`, and `[data_plane]` sections.
6. Update `routes.rs` to mount `/ws` to either control-plane or data-plane core based on role config, and mount `/control/v1/ws` only when control-plane is enabled.
7. Implement `DataPlaneReporter`, `ControlPlaneClient`, capability snapshotting, and `control.command` execution in `vol-agent-server::data_plane`.
8. Add persistent control-plane store only after in-memory semantics stabilize.

## Out of Scope

- New `vol-agent-control-plane` crate.
- Wire protocol definitions inside `vol-agent-server`.
- Control-plane state inside `vol-llm-agent-channel`.
- Control-plane awareness inside `vol-llm-runtime`.
- Multi-control-plane HA and distributed consensus.
- Cross-node shared memory/session migration during a running agent run.
- Exactly-once command execution across process crashes. MVP provides at-least-once JSON-RPC request delivery with `id` / `command_id` deduplication during node process lifetime.

## Testing Strategy

### Unit tests

- `vol-llm-agent-channel`: JSON-RPC frame decode/encode for `control.*` methods, request/notification/response/error cases, and `Operation::Control` method mapping.
- `vol-llm-agent-channel`: generic `JsonRpcServer<S>` delegates upgraded connections to a mock `JsonRpcMessageService`.
- `vol-agent-server::control_plane`: `NodeRegistry` state transitions, `CapabilityIndex` replacement semantics, `LeaseManager` timeout behavior, and `ControlRouter` placement policy.
- `vol-agent-server::data_plane`: `CapabilitySnapshotter` projects `AgentRuntime` state; `control.command` maps to local handlers; command deduplication cache behavior.

### Integration tests

- Standalone data-plane mode: `/ws` serves existing `agent.*`, `tool.*`, `mcp.*` behavior.
- Standalone control-plane mode: `/ws` accepts client JSON-RPC, `/control/v1/ws` accepts node JSON-RPC; verify `control.register` → `control.capability_snapshot` → `control.heartbeat`.
- Combined mode: local data-plane registers to local control-plane endpoint; `agent.submit` on `/ws` routes to the local node via `control.command`.
- Reconnect same `node_id` and assert capability snapshot replaces stale entries.
- Register two nodes with the same tool name and verify deterministic route selection or required disambiguation.

### Compatibility tests

- Existing local `vol-agent-server` behavior remains unchanged when `[server.roles].control_plane = false`.
- Existing JSON-RPC methods preserve request/response/error shape.
- `vol-llm-agent-channel` has no dependency on `vol-agent-server` and does not reference `DataPlaneServerCore` or `ControlPlaneServerCore` concrete types.
