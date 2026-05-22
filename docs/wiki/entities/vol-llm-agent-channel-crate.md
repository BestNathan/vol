---
type: entity
category: product
tags: [crate, agent, transport, rust, json-rpc]
created: 2026-05-05
updated: 2026-05-22
source_count: 6
---

# vol-llm-agent-channel Crate

**Category:** Rust crate — Agent communication channel layer
**Related:** [[vol-llm-agent-crate]], [[react-pattern]], [[connection-trait]], [[connection-holder]], [[agent-dispatcher]], [[http-transport]], [[remote-agent-connection]], [[jsonrpc-transport]], [[agent-router]], [[task-5-jsonrpc-integration-tests]], [[jsonrpc-transport-refactoring]], [[vol-mcp-servers-crate]], [[vol-llm-ui-crate]], [[agentinput-multimodal-run]], [[agentinput-channel-unification]]

## Overview

The `vol-llm-agent-channel` crate provides the communication layer between external clients and ReActAgent instances. It offers multiple transport protocols (WebSocket, HTTP, in-memory, JSON-RPC WebSocket) unified through the `Connection` trait, with FIFO request dispatching and multi-agent routing support.

## Key Facts

- `Connection` trait abstracts transport protocols with `protocol()`, `send()`, and `recv()` [[http-transport-impl]]
- `ConnectionHolder` implements `AgentPlugin` to forward agent events to the attached connection — single event bridge for all transports [[jsonrpc-transport-refactoring]]
- `AgentDispatcher` provides FIFO request queueing with `submit()` returning oneshot receivers [[http-transport-impl]]
- `AgentRouter` provides multi-agent request routing via `HashMap<String, AgentDispatcher>` [[agent-router]]
- `AgentPayload::Submit` carries `input: AgentInput` with `target` for routing — `run_id` and `metadata` live inside `AgentInput` [[agentinput-channel-unification]]
- `AgentDispatcher` calls `agent.run_input(AgentInput)` instead of the removed `run_with_id()` [[agent-dispatcher]]
- `jsonrpc` module: `JsonRpcConnection` implements `Connection` trait, `JsonRpcServer` accepts `Vec<AgentRegistration>` for multi-agent [[jsonrpc-transport]]
- 12 JSON-RPC methods: `agent.submit` (with optional `target`), `cancel`, `subscribe`, `unsubscribe`, `approve`, `file.list`, `file.read`, `log.list`, `log.read`, `session.list`, `session.resume` [[jsonrpc-transport]]
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

- **2026-04**: Initial implementation with WebSocket transport and memory transport
- **2026-05-05**: HTTP transport added with blocking and SSE modes [[http-transport-impl]]
- **2026-05-05**: HTTP transport quality improvements — concurrent request protection, clean stream termination, holder detach, and test suite (5 tests) [[http-transport-impl]]
- **2026-05-07**: Example applications added — `single_agent.rs` (dual transport) and `multi_agent.rs` (agent router) [[agent-channel-examples]]
- **2026-05-09**: JSON-RPC transport refactoring — `JsonRpcConnection` implements `Connection` trait, `EventBridgePlugin` deleted, `JsonRpcServer` with multi-agent support, 49 integration tests [[jsonrpc-transport-refactoring]]
- **2026-05-22**: `AgentPayload::Submit`, `AgentRequest`, and dispatcher unified to use `AgentInput` directly — dropped redundant `run_id`/`metadata` fields, dispatcher switched from `run_with_id` to `run_input` [[agentinput-channel-unification]]
