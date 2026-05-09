---
type: entity
category: product
tags: [crate, agent, transport, rust, json-rpc]
created: 2026-05-05
updated: 2026-05-09
source_count: 3
---

# vol-llm-agent-channel Crate

**Category:** Rust crate — Agent communication channel layer
**Related:** [[vol-llm-agent-crate]], [[react-pattern]], [[connection-trait]], [[connection-holder]], [[agent-dispatcher]], [[http-transport]], [[remote-agent-connection]], [[jsonrpc-server-handler]], [[task-5-jsonrpc-integration-tests]]

## Overview

The `vol-llm-agent-channel` crate provides the communication layer between external clients and ReActAgent instances. It offers multiple transport protocols (WebSocket, HTTP, in-memory) unified through the `Connection` trait, and a FIFO request dispatcher for single-agent queueing.

## Key Facts
- `Connection` trait abstracts transport protocols with `recv()` and `send()` [[http-transport-impl]]
- `ConnectionHolder` implements `AgentPlugin` to forward agent events to the active connection [[http-transport-impl]]
- `AgentDispatcher` provides FIFO request queueing with `submit()` returning oneshot receivers [[http-transport-impl]]
- Three transport implementations: `WsServer` (WebSocket), `HttpTransport` (HTTP POST + SSE), `MemoryConnection` (in-memory testing) [[http-transport-impl]]
- `AgentRouter` provides multi-agent routing (separate from dispatcher) [[vol-llm-agent-crate]]
- `Message` enum unifies all communication: Submit, Cancel, Connected, Event, Result, Error [[http-transport-impl]]
- `jsonrpc` module exports `JsonRpcHandler` and `JsonRpcContext` for JSON-RPC 2.0 server [[task-9-jsonrpc-server]]
- 9 JSON-RPC methods: `agent.submit/cancel/approve`, `file.list/read`, `log.list/read`, `session.list/resume` [[task-9-jsonrpc-server]]

## Transport Comparison

| Transport | Protocol | Bidirectional | Mount Style | Use Case |
|-----------|----------|---------------|-------------|----------|
| `WsServer` | WebSocket | Yes | Fixed `/ws` | Real-time, long-lived connections |
| `HttpTransport` | HTTP POST | Request-response | Any path via `.merge()` | Simple REST, SSE streaming |
| `MemoryConnection` | mpsc channel | Yes | Direct handle | Testing, inter-process |

## Architecture

```
Client → Transport (WS/HTTP/Memory) → Connection → ConnectionHolder (AgentPlugin)
                                                   ↕ events
                                            ReActAgent ← AgentDispatcher (FIFO queue)
```

## Timeline
- **2026-04**: Initial implementation with WebSocket transport and memory transport
- **2026-05-05**: HTTP transport added with blocking and SSE modes [[http-transport-impl]]
- **2026-05-05**: HTTP transport quality improvements — concurrent request protection, clean stream termination, holder detach, and test suite (5 tests) [[http-transport-impl]]
- **2026-05-07**: Example applications added — `single_agent.rs` (dual transport) and `multi_agent.rs` (agent router) [[agent-channel-examples]]
- **2026-05-08**: `jsonrpc` module added with `JsonRpcHandler`/`JsonRpcContext`, 9 RPC methods, and `jsonrpc_agent_service.rs` example [[task-9-jsonrpc-server]]
- **2026-05-08**: Final verification passed — 16 tests, all targets compile [[task-10-final-verification]]
- **2026-05-09**: JSON-RPC serialization integration tests added — 44 tests covering all AgentStreamEvent variants, all JSON-RPC request methods, and error handling for malformed input [[task-5-jsonrpc-integration-tests]]
