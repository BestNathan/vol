---
type: source
source_type: code
date: 2026-06-10
ingested: 2026-06-10
tags: [agent-server, data-plane, json-rpc, channel-boundary, rust]
---

# Agent Server Data-Plane Core Move

**Authors/Creators:** Claude Code
**Date:** 2026-06-10
**Link:** /Users/admin/Documents/learn/vol-agent/crates/vol-agent-server/src/data_plane

## TL;DR
Task 4 moved concrete standalone data-plane execution behavior out of [[vol-llm-agent-channel-crate]] and into [[vol-agent-server-crate]] as `vol_agent_server::data_plane::DataPlaneServerCore`. The channel crate now keeps protocol, connection, handler registry, and generic JSON-RPC transport abstractions, while server owns concrete runtime construction, local handlers, router, dispatcher, and connection holder behavior.

## Key Takeaways
- Created `crates/vol-agent-server/src/data_plane/` with `builder`, `connection_holder`, `core`, `dispatcher`, `handlers`, and `router` modules; empty `command`/`snapshot` placeholders were removed in cleanup for later Task 8 implementation.
- Created `crates/vol-agent-server/src/data_plane/handlers/` with `agent`, `file`, `log`, `mcp`, `session`, `skill`, `system`, `task`, and `tool` handlers.
- Renamed `AgentServerCore` to `DataPlaneServerCore` and `AgentServerCoreBuilder` to `DataPlaneServerCoreBuilder` in the server crate.
- Moved `ConnectionHolder`, `AgentDispatcher`, `AgentRouter`, and concrete data-plane domain handlers into `vol-agent-server`.
- Left `Connection`, `DomainHandler`, `HandlerRegistry`, protocol models, `JsonRpcMessageService`, and generic JSON-RPC/WebSocket transports in `vol-llm-agent-channel`.
- Removed old concrete channel modules/exports and moved data-plane integration tests to `vol-agent-server`.
- Updated `vol-agent-server` startup to build `DataPlaneServerCore` and mount `JsonRpcServer::new(Arc::new(core), config.control_plane.client_ws_path.clone())`.

## Detailed Summary
The migration implements the Task 4 boundary from [[agent-server-control-data-plane-implementation-plan]]. `vol-agent-server` now exposes a library surface and a `data_plane` module tree. `DataPlaneServerCore` owns the same standalone behavior that previously lived in the channel crate: it builds [[vol-llm-runtime-crate]], derives LLM/MCP/skill/tool resources, discovers and registers local agents, owns the local `AgentRouter`, attaches `ConnectionHolder` plugins, and dispatches `AgentServerMessage` values through `HandlerRegistry`.

`vol-llm-agent-channel` was reduced to protocol/transport abstractions for this boundary. It still owns `Connection`, `DomainHandler`, `HandlerRegistry`, `AgentServerMessage`, `Operation`, `Payload`, JSON-RPC codec/server/connection code, and `JsonRpcMessageService`. The legacy `WsServer` was made generic over `JsonRpcMessageService`; obsolete concrete HTTP transport and channel examples that directly constructed the old core were removed.

The concrete data-plane tests that previously compiled against `vol-llm-agent-channel::AgentServerCore` now live under `crates/vol-agent-server/tests/` and target `DataPlaneServerCore`. Channel tests now focus on protocol, codec, connection trait, and generic transport behavior.

Task 4 cleanup removed reviewed-crate compile warnings, pruned unused channel dependencies after the move, made `JsonRpcServer<S>` own a configurable `String` path, and marked the deleted HTTP transport/examples as historical wiki content. `McpHandler` now logs reconnect status rather than computing an unused status string.

Verification passed:
- `cargo fmt --all --check`
- `cargo check -p vol-llm-agent-channel --tests`
- `cargo check -p vol-agent-server --tests`
- `cargo test -p vol-llm-agent-channel --tests` — 61 passed
- `cargo test -p vol-agent-server --tests` — 37 passed
- `cargo clippy -p vol-llm-agent-channel --all-targets --no-deps -- -D warnings`
- `cargo clippy -p vol-agent-server --all-targets --no-deps -- -D warnings`

## Entities Mentioned
- [[vol-agent-server-crate]]: now owns concrete standalone data-plane server execution behavior.
- [[vol-llm-agent-channel-crate]]: now keeps channel/protocol/transport abstractions and no longer exports concrete data-plane core/router/dispatcher/handlers.
- [[vol-llm-runtime-crate]]: remains the runtime resource owner used by `DataPlaneServerCore`.

## Concepts Covered
- [[agent-server-control-data-plane]]: Task 4 completes the concrete data-plane half of the ownership split.
- [[agent-router]]: moved from channel crate into server-owned data-plane as node-local routing.
- [[connection-holder]]: moved from channel crate into server-owned data-plane as concrete agent event bridge.

## Notes
Control-plane state, handlers, distributed routing, data-plane reporter loopback, and command/snapshot implementations remain later tasks; the empty `data_plane::command` and `data_plane::snapshot` placeholder modules were intentionally removed until real code is added.
