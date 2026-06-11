---
type: entity
category: product
tags: [crate, agent, transport, rust, json-rpc, control-plane]
created: 2026-05-05
updated: 2026-06-10
source_count: 20
---

# vol-llm-agent-channel Crate

**Category:** Rust crate — Agent communication channel layer
**Related:** [[vol-llm-agent-crate]], [[react-pattern]], [[connection-trait]], [[connection-holder]], [[agent-dispatcher]], [[http-transport]], [[remote-agent-connection]], [[jsonrpc-transport]], [[agent-router]], [[task-5-jsonrpc-integration-tests]], [[jsonrpc-transport-refactoring]], [[vol-mcp-servers-crate]], [[vol-llm-ui-crate]], [[agentinput-multimodal-run]], [[agentinput-channel-unification]], [[task-database-store-implementation]], [[runtime-task-store-configuration]], [[task-4-quality-issues-cleanup]]

## Overview

The `vol-llm-agent-channel` crate provides the protocol, transport, and dispatch abstraction layer for agent-server communication. In the final control/data-plane architecture it owns all wire-level JSON-RPC definitions and transport abstractions, while concrete data-plane/control-plane server cores live in [[vol-agent-server-crate]].

## Key Facts

- `Connection` trait abstracts transport protocols with `protocol()`, `send()`, and `recv()` [[http-transport-impl]]
- `ConnectionHolder` now lives in [[vol-agent-server-crate]] data-plane code; channel keeps only the `Connection` trait boundary [[agent-server-data-plane-core-move]]
- `AgentDispatcher` and `AgentRouter` now live in [[vol-agent-server-crate]] data-plane code as concrete execution/routing components [[agent-server-data-plane-core-move]]
- `AgentPayload::Submit` carries `input: AgentInput` with `target` for routing — `run_id` and `metadata` live inside `AgentInput` [[agentinput-channel-unification]]
- `AgentDispatcher` calls `agent.run_input(AgentInput)` instead of the removed `run_with_id()` [[agent-dispatcher]]
- `transport::jsonrpc` module: `JsonRpcConnection` implements `Connection` trait, and `JsonRpcServer<S>` is generic over `JsonRpcMessageService` with an explicit mount path [[jsonrpc-transport]]
- All JSON-RPC transport code consolidated under `transport/jsonrpc/` — server, connection, codec, and serde helpers [[jsonrpc-transport-consolidation]]
- 12 JSON-RPC methods: `agent.submit` (with optional `target`), `cancel`, `subscribe`, `unsubscribe`, `approve`, `file.list`, `file.read`, `log.list`, `log.read`, `session.list`, `session.resume` [[jsonrpc-transport]]
- Current web backend startup path is owned by `vol-agent-server`; historical channel examples were removed during Task 4 cleanup [[agent-server-data-plane-core-move]]
- `AgentServerCoreBuilder` accepts optional `TaskStoreConfig` and forwards it into `AgentRuntimeBuilder`, while `TaskHandler` continues to use the single shared `runtime.task_store` [[task-database-store-implementation]]
- `AgentServerCoreBuilder` also forwards optional `SessionStoreConfig`; `SessionHandler` and `register_agent` use the runtime-owned `SessionManager` so file/database session backends stay consistent [[session-database-store-implementation]]
- `JsonRpcConnection::send` preserves structured `ErrorPayload` values and request IDs by routing error messages through the JSON-RPC codec [[session-database-store-implementation]]
- Final control/data-plane boundary: this crate owns `Operation`, `Payload`, `control.*` method/payload definitions, JSON-RPC codec, `Connection`, `DomainHandler`, `HandlerRegistry`, and a generic `JsonRpcMessageService`; concrete `DataPlaneServerCore` and `ControlPlaneServerCore` belong in [[vol-agent-server-crate]] [[agent-server-control-data-plane-architecture]]
- `JsonRpcMessageService` is now implemented and exported; `JsonRpcServer<S>` consumes any service implementing it and accepts an explicit WebSocket mount path, decoupling JSON-RPC transport from concrete `AgentServerCore` [[agent-server-control-data-plane-implementation-plan]]
- Addendum clarifies this crate should also own endpoint/domain error code vocabulary and JSON-RPC method allowlist protocol semantics, while server fills contextual details [[agent-server-control-data-plane-addendum]]
- `ControlPayload` uses default externally tagged enum serialization so `Payload::data_json()` strips the variant wrapper and emits flat `control.*` JSON-RPC params/results rather than internal `type`/`data` wrappers [[control-payload-flat-jsonrpc-encoding-fix]]
- Concrete `AgentServerCore` behavior moved out of this crate into `vol-agent-server::data_plane::DataPlaneServerCore`; channel no longer exports the concrete core/router/dispatcher/handler modules [[agent-server-data-plane-core-move]]
- 49 integration tests for JSON-RPC serialization and parsing [[task-5-jsonrpc-integration-tests]]
- Task 4 quality cleanup removed unused `uuid`/`tempfile`, kept `tokio-tungstenite` and `vol-llm-core` as test-only dev-dependencies, and neutralized comments that implied channel-owned routing/dispatch [[task-4-quality-issues-cleanup]]

## Transport Comparison

| Transport | Protocol | Bidirectional | Mount Style | Use Case |
|-----------|----------|---------------|-------------|----------|
| `WsConnection` | WebSocket binary | Yes | Fixed `/ws` | Real-time, native protocol |
| `JsonRpcConnection` | JSON-RPC 2.0 text | Yes | Configured path via `JsonRpcServer::new` | Web frontend, browser-compatible |
| `MemoryConnection` | mpsc channel | Yes | Direct handle | Testing, inter-process |
| `HttpTransport` | HTTP POST + SSE | Request-response | Deleted | Historical only; removed after Task 4 cleanup |

## Architecture

```
Client → Transport (WS/JSON-RPC/Memory) → Connection → ConnectionHolder (AgentPlugin)
                                                        ↕ events
                                                 ReActAgent ← AgentDispatcher (FIFO queue)
                                                              ↕ requests
                                                        AgentRouter (multi-agent)
```

## Module Structure

Current modules after the Task 4 boundary refactor:

- `connection.rs` — `Connection` trait only; concrete `ConnectionHolder` moved to [[vol-agent-server-crate]]
- `domain/handler.rs` — `DomainHandler` abstraction
- `domain/registry.rs` — `HandlerRegistry` abstraction
- `request.rs` — transport-independent request/result queue models shared with data-plane dispatcher
- `service.rs` — `JsonRpcMessageService` abstraction for JSON-RPC transports
- `transport/ws.rs` — generic `WsConnection`/`WsServer<S>` over `JsonRpcMessageService`
- `transport/memory.rs` — `MemoryConnection` (in-memory testing)
- `transport/jsonrpc/codec.rs` — JSON-RPC frame encode/decode
- `transport/jsonrpc/connection.rs` — `JsonRpcConnection` implementing `Connection`
- `transport/jsonrpc/server.rs` — generic `JsonRpcServer<S>` with configured mount path
- `transport/jsonrpc/serde_helpers.rs` — legacy JSON-RPC event/response helper functions

Concrete execution pieces (`AgentDispatcher`, `AgentRouter`, `ConnectionHolder`, data-plane handlers, and previous `AgentServerCore`) now live in [[vol-agent-server-crate]] under `data_plane` [[agent-server-data-plane-core-move]].

## Timeline

- **2026-04**: Initial implementation with WebSocket transport and memory transport
- **2026-05-05**: HTTP transport added with blocking and SSE modes [[http-transport-impl]]
- **2026-05-05**: HTTP transport quality improvements — concurrent request protection, clean stream termination, holder detach, and test suite (5 tests) [[http-transport-impl]]
- **2026-05-07**: Historical example applications added — `single_agent.rs` (dual transport) and `multi_agent.rs` (agent router); later deleted during Task 4 cleanup [[agent-channel-examples]]
- **2026-05-09**: JSON-RPC transport refactoring — `JsonRpcConnection` implements `Connection` trait, `EventBridgePlugin` deleted, `JsonRpcServer` with multi-agent support, 49 integration tests [[jsonrpc-transport-refactoring]]
- **2026-05-23**: Agent directory discovery — `discover_agents()` replaces manual registration, `agent.list` returns type/description/scope metadata [[agent-directory-discovery]]
- **2026-05-29**: Obsolete `vol-agent-manager` service removed; later Task 4 cleanup moved active server ownership to `vol-agent-server` and removed channel examples [[remove-vol-agent-manager]]
- **2026-06-09**: `AgentServerCoreBuilder` forwards optional runtime task-store config into `AgentRuntimeBuilder`; task JSON-RPC handling still reads the single shared `runtime.task_store` [[task-database-store-implementation]]
- **2026-06-10**: Session-domain JSON-RPC operations and registered agent session creation were rewired to use `runtime.session_manager`, with database-backed SQLite coverage and JSON-RPC error payload preservation [[session-database-store-implementation]]
- **2026-06-10**: Final control/data-plane architecture set this crate's boundary to protocol/JSON-RPC transport/abstractions only; concrete `AgentServerCore` behavior should move to `vol-agent-server::data_plane` as `DataPlaneServerCore`, with `ControlPlaneServerCore` also implemented in `vol-agent-server` [[agent-server-control-data-plane-architecture]]
- **2026-06-10**: Task 1 implemented `JsonRpcMessageService`, made `JsonRpcServer<S>` generic over service plus route path, and adapted current `AgentServerCore` through `serve_dyn(Arc<dyn Connection>)` [[agent-server-control-data-plane-implementation-plan]]
- **2026-06-10**: Task 2 code-quality fix removed internal serde tagging from `ControlPayload` and added JSON-RPC codec tests for flat `control.register` params and `RegisterAck` results [[control-payload-flat-jsonrpc-encoding-fix]]
- **2026-06-10**: Task 4 moved concrete data-plane core/router/dispatcher/handlers and `ConnectionHolder` into `vol-agent-server::data_plane`; channel now keeps protocol, connection, handler registry, request, service, and generic transport abstractions [[agent-server-data-plane-core-move]]
- **2026-06-10**: Task 4 cleanup removed stale HTTP/example API references and pruned channel dependencies to active protocol/transport needs [[agent-server-data-plane-core-move]]
- **2026-06-10**: Follow-up quality cleanup removed unused/mis-scoped channel dependencies, refreshed generic JSON-RPC documentation, and neutralized moved-router/dispatcher comments [[task-4-quality-issues-cleanup]]
