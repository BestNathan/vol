---
type: entity
category: service
tags: [server, config, json-rpc, task-store, session-store, data-plane, control-plane]
created: 2026-06-09
updated: 2026-06-10
source_count: 16
---

# vol-agent-server Crate

## Overview
`vol-agent-server` is the standalone server crate that loads TOML configuration and launches the agent JSON-RPC backend service.

## Key Facts
- Config is loaded from an explicit path, `~/.vol/agent-server.toml`, or defaults.
- Runtime path settings include `working_dir` and `store_dir` with tilde expansion.
- The crate depends on [[vol-llm-runtime-crate]] for shared task store and session store config types.
- Active web backend ownership lives here; startup uses `config.control_plane.client_ws_path`, defaulting to `/ws`, when mounting the generic channel JSON-RPC server [[task-4-quality-issues-cleanup]].

## Runtime Task Store Config Parsing
Sources: [[task-store-config-parsing]], [[task-database-store-implementation]]

`RuntimeSection` now includes optional `task_store: Option<vol_llm_runtime::TaskStoreConfig>`. When omitted, config defaults preserve the existing file-backed task store path.

`ServerConfig::load` validates parsed config before returning it. `ServerConfig::validate` delegates to `TaskStoreConfig::validate`, giving early errors for invalid `[runtime.task_store]` TOML.

Covered test cases:
- Parses database config with `url = "sqlite:///tmp/vol-agent/tasks.db"`.
- Rejects `type = "database"` without `url`.
- Rejects `type = "file"` with `url`.
- Rejects unknown database scheme such as `oracle://`.

`vol-agent-server` logs whether it is using the default file task store or a configured store type, then passes `config.runtime.task_store.clone()` into `AgentServerCore::builder(...).with_task_store_config(...)`.

## Runtime Session Store Config Parsing
Source: [[session-database-store-implementation]]

`RuntimeSection` now includes optional `session_store: Option<vol_llm_runtime::SessionStoreConfig>`. Server validation delegates to `SessionStoreConfig::validate`, so invalid `[runtime.session_store]` TOML is rejected before server startup.

Covered test cases include parsing a SQLite database session-store URL and rejecting `type = "database"` when `url` is missing. Startup logging mirrors task-store logging and the builder chain passes `config.runtime.session_store.clone()` into `AgentServerCore::builder(...).with_session_store_config(...)`.

## Proposed Data/control-plane Server Cores
Source: [[agent-server-control-data-plane-architecture]]

In the final control/data-plane architecture, `vol-agent-server` is the single concrete server implementation crate. It owns both `DataPlaneServerCore` and `ControlPlaneServerCore` and composes them by config. `DataPlaneServerCore` is the final home for current `AgentServerCore` behavior: it builds [[vol-llm-runtime-crate]], discovers agents, registers data-plane handlers, and serves standalone `/ws` when control-plane mode is disabled. `ControlPlaneServerCore` owns node registry, capability index, lease management, distributed routing, and control-plane event fan-out.

`vol-agent-server` must not define wire-level protocol types. It uses [[vol-llm-agent-protocol-crate]] protocol models, JSON-RPC transport, `DomainHandler`, `HandlerRegistry`, and generic service abstractions.

Task 1 of the implementation plan updated current startup to construct a generic `JsonRpcServer` from `Arc::new(core)` plus a configured route path, while the channel transport became generic over any `JsonRpcMessageService`.

Task 3 added the first concrete server role configuration and route skeleton [[agent-server-role-config-route-skeleton]]. `ServerConfig` now parses `[server.roles]`, `[control_plane]`, and `[data_plane]`, defaults to standalone data-plane mode (`control_plane=false`, `data_plane=true`), and rejects configs where both roles are disabled. The crate also has a minimal Axum skeleton: `health::health`, `routes::base_router()` mounted at `/health`, and `app::run(ServerConfig)` that binds the base router without changing current `main.rs` startup behavior.

Task 4 completed the concrete data-plane move [[agent-server-data-plane-core-move]]. `vol-agent-server::data_plane` now owns `DataPlaneServerCore`, `DataPlaneServerCoreBuilder`, `AgentRouter`, `AgentDispatcher`, `ConnectionHolder`, and all concrete data-plane handlers (`agent`, `file`, `log`, `mcp`, `session`, `skill`, `system`, `task`, `tool`). The binary startup path builds `DataPlaneServerCore` and mounts it with `JsonRpcServer::new(Arc::new(core), config.control_plane.client_ws_path)`, where `client_ws_path` defaults to `/ws`, preserving standalone behavior while removing concrete execution ownership from [[vol-llm-agent-protocol-crate]].

Task 6 added the first concrete control-plane core and handler layer [[agent-server-control-plane-core-handlers]]. `vol-agent-server::control_plane::core::ControlPlaneServerCore` owns `Arc<ControlPlaneState>`, registers `ControlHandler`, `NodeHandler`, and `CapabilityHandler` in `HandlerRegistry`, dispatches messages through `handle(message)`, and implements channel-owned `JsonRpcMessageService`. The handlers cover node registration, heartbeat updates, capability snapshots, event fan-out, node list/get, and capability list with optional node filtering.

Task 7 composed active routes by role [[agent-server-role-route-composition]]. `routes::ws_owner(control_plane, data_plane)` documents and tests `/ws` ownership: standalone data-plane owns `/ws` only when control-plane is disabled, while control-plane owns `/ws` whenever enabled. `app::run` now expands runtime paths, builds role-specific cores, mounts `ControlPlaneServerCore` on configured client/node WebSocket paths, mounts `DataPlaneServerCore` on the client path for standalone data-plane mode, binds the listener, and serves the composed Axum router. `main.rs` preserves config/tracing setup and delegates runtime startup to `app::run`.

A Task 7 quality fix [[agent-server-health-route-collision-validation]] hardens `ServerConfig::validate` against Axum duplicate-route panics: enabled control-plane configs reject `client_ws_path` or `node_ws_path` equal to `/health`, and standalone data-plane configs reject `client_ws_path = "/health"` because `/health` is already registered by `routes::base_router()`.

Task 8 added data-plane reporting primitives [[agent-server-data-plane-snapshot-command]]. `data_plane::snapshot` defines the `RuntimeCapabilitySource` facade plus `StaticCapabilitySource`, currently returning revision-1 empty snapshots and zero load. `data_plane::command::accept_control_command` returns accepted `CommandAck` responses and synthesizes `run_{command_id}` for `SubmitAgent` commands while leaving other command types without a run id.

Task 9 added the control-plane router MVP [[agent-server-control-router-mvp]]. `control_plane::router::ControlRouter<'a>` routes `route_agent(target)` against `CapabilityIndex` snapshots, selecting only online nodes, and matching by `agent_id` or `name` when a target is provided. The error string for no route is `capability_not_found`.

Task 10 added boundary and role-mode verification [[agent-server-boundary-mode-verification]]. `crates/vol-agent-server/tests/role_modes.rs` verifies standalone data-plane `/ws` ownership, control-plane `/ws` priority in standalone-control and combined modes, and TOML validation rejection when both roles are disabled. `scripts/check-agent-boundaries.sh` verifies `vol-llm-agent-channel` and `vol-llm-runtime` do not depend on `vol-agent-server`.

The addendum [[agent-server-control-data-plane-addendum]] further specifies endpoint role allowlists, command/run record separation, node record/session separation, combined-mode lifecycle, and boundary verification tests that should be implemented in this crate.

## Related
- [[agent-server-control-data-plane]]
- [[vol-llm-agent-protocol-crate]]
- [[vol-llm-runtime-crate]]
- [[runtime-task-store-configuration]]
- [[runtime-session-store-configuration]]
- [[session-database-store-implementation]]
- [[task-store-config-parsing]]
- [[task-database-store-implementation]]
- [[task-4-quality-issues-cleanup]]
- [[agent-server-data-plane-snapshot-command]]
- [[agent-server-control-router-mvp]]
