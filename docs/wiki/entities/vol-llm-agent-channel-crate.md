---
type: entity
category: product
tags: [crate, agent, transport, rust, json-rpc]
created: 2026-05-05
updated: 2026-05-21
source_count: 6
---

# vol-llm-agent-channel Crate

**Category:** Rust crate — Agent communication channel layer
**Related:** [[vol-llm-agent-crate]], [[react-pattern]], [[connection-trait]], [[connection-holder]], [[agent-dispatcher]], [[http-transport]], [[remote-agent-connection]], [[jsonrpc-transport]], [[agent-router]], [[agent-server-protocol]], [[agent-channel-server-protocol-transport-migration]], [[task-5-jsonrpc-integration-tests]], [[jsonrpc-transport-refactoring]], [[vol-mcp-servers-crate]], [[vol-llm-ui-crate]], [[skill-system]], [[skills-panel-json-rpc]]

## Overview

The `vol-llm-agent-channel` crate provides the communication layer between external clients and ReActAgent instances. It offers multiple transport protocols (WebSocket, HTTP, in-memory, JSON-RPC WebSocket) unified around [[agent-server-protocol]] messages, with `AgentServerCore` owning domain dispatch and transport code limited to wire-level decode/encode responsibilities.

## Key Facts

- `Connection` trait abstracts transport protocols with `protocol()`, `send()`, and `recv()` [[http-transport-impl]]
- `ConnectionHolder` implements `AgentPlugin` to forward agent events to the attached connection — single event bridge for all transports [[jsonrpc-transport-refactoring]]
- `AgentDispatcher` provides FIFO request queueing with `submit()` returning oneshot receivers [[http-transport-impl]]
- `AgentRouter` provides multi-agent request routing via `HashMap<String, AgentDispatcher>` [[agent-router]]
- `AgentServerMessage` is the protocol boundary for WebSocket, HTTP, memory, JSON-RPC adapters, and manager integration; the legacy `protocol::Message` enum was removed [[agent-channel-server-protocol-transport-migration]]
- `jsonrpc` module: `JsonRpcConnection` implements `Connection` trait, `JsonRpcServer` accepts `Vec<AgentRegistration>` for multi-agent, optional `Arc<SkillLoader>` for skill discovery [[jsonrpc-transport]], [[skills-panel-json-rpc]]
- 14 JSON-RPC methods: 12 existing agent/file/log/session methods plus `skill.list`, `skill.get` [[jsonrpc-transport]], [[skills-panel-json-rpc]]
- `run_id` is the business identifier for one agent inference run; `message_id` stays protocol correlation only [[run-id-unification]]
- `AgentRequest`, `RunResult`, dispatcher cancellation, and agent-domain submit/cancel handling all now propagate `run_id` [[run-id-unification]]
- 49 integration tests for JSON-RPC serialization and parsing [[task-5-jsonrpc-integration-tests]]

## Transport Comparison

| Transport | Protocol | Bidirectional | Mount Style | Use Case |
|-----------|----------|---------------|-------------|----------|
| `WsConnection` | WebSocket binary | Yes | Fixed `/ws` | Real-time, native protocol |
| `JsonRpcConnection` | JSON-RPC 2.0 text | Yes | Fixed `/ws` | Web frontend, browser-compatible |
| `HttpTransport` | HTTP POST + SSE | Request-response | Any path via `.merge()` | Simple REST, SSE streaming |
| `MemoryConnection` | mpsc channel | Yes | Direct handle | Testing, inter-process |

## Architecture

```
Client → Transport (WS/HTTP/JSON-RPC/Memory) → Connection → ConnectionHolder (AgentPlugin)
                                                        ↕ events
                                                 ReActAgent ← AgentDispatcher (FIFO queue)
                                                              ↕ requests
                                                        AgentRouter (multi-agent)
```

## Module Structure

- `connection.rs` — `Connection` trait and `ConnectionHolder` plugin
- `dispatcher.rs` — `AgentDispatcher` with FIFO queueing
- `router.rs` — `AgentRouter` for multi-agent dispatch
- `transport/ws.rs` — `WsConnection` (raw WebSocket)
- `transport/http.rs` — `HttpTransport` (HTTP POST + SSE)
- `transport/memory.rs` — `MemoryConnection` (in-memory testing)
- `jsonrpc/connection.rs` — `JsonRpcConnection` implementing `Connection`
- `jsonrpc/server.rs` — `JsonRpcServer` with `AgentRegistration`
- `jsonrpc/serde_helpers.rs` — JSON-RPC serialization helpers

## Timeline

- **2026-05-21:** WebSocket and HTTP transports migrated to `AgentServerCore` + `AgentServerMessage`; legacy `protocol::Message` deleted; examples and manager protocol tests updated [[agent-channel-server-protocol-transport-migration]]

- **2026-04**: Initial implementation with WebSocket transport and memory transport
- **2026-05-05**: HTTP transport added with blocking and SSE modes [[http-transport-impl]]
- **2026-05-05**: HTTP transport quality improvements — concurrent request protection, clean stream termination, holder detach, and test suite (5 tests) [[http-transport-impl]]
- **2026-05-07**: Example applications added — `single_agent.rs` (dual transport) and `multi_agent.rs` (agent router) [[agent-channel-examples]]
- **2026-05-09**: JSON-RPC transport refactoring — `JsonRpcConnection` implements `Connection` trait, `EventBridgePlugin` deleted, `JsonRpcServer` with multi-agent support, 49 integration tests [[jsonrpc-transport-refactoring]]
- **2026-05-16**: `skill.list` and `skill.get` RPC methods added — `JsonRpcServer` gains `Option<Arc<SkillLoader>>`, `handle_skill_list()` returns metadata array, `handle_skill_get()` returns full `SkillDetail` or not-found error; `SkillLoader` wired into `jsonrpc_agent_service.rs` at startup [[skills-panel-json-rpc]]
- **2026-05-21**: Run identity unified around `run_id` — protocol submit/cancel payloads, `AgentRequest`, `RunResult`, dispatcher/router cancellation, and agent-domain handler propagation now use run id as the business lifecycle id while `message_id` remains transport correlation [[run-id-unification]]
