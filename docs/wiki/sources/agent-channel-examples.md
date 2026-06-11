---
type: source
category: implementation
tags: [examples, agent, channel, websocket, http, axum]
created: 2026-05-07
updated: 2026-06-10
---

# Agent Channel Examples: Historical WS + HTTP Service

**Source Type:** Implementation
**Related:** [[vol-llm-agent-channel-crate]], [[agent-server-protocol]], [[http-transport]], [[agent-channel-server-protocol-transport-migration]]

## Summary

Historical example applications once lived in `crates/vol-llm-agent-channel/examples/` to demonstrate WS + HTTP services. They were deleted during the Task 4 channel/data-plane cleanup; current runnable server behavior lives in `vol-agent-server`.

## Historical Examples Deleted

### single_agent.rs

A single `ReActAgent` with dual transport (WebSocket + HTTP) behind one axum server.

- Endpoints: `/ws` (WS), `/api/chat` (HTTP POST), `/api/chat?stream=true` (SSE), `/health`
- Uses `WsServer` and `HttpTransport` backed by shared `AgentServerCore`
- LLM provider created from env vars (DashScope Anthropic endpoint)
- Port: 3000

### multi_agent.rs

Multiple `ReActAgent` instances registered with `AgentRouter`, each accessible via path parameter.

- Endpoints: `/ws` (Agent Server Protocol WS), `/api/chat/:agent_id` (HTTP POST), `/api/agents` (list), `/health`
- Three agents: translator, summarizer, coder — each registered with `AgentServerCore::register_agent`
- HTTP chat builds `AgentPayload::Submit` with `target: Some(agent_id)` and delegates to `AgentServerCore::handle`
- WebSocket traffic uses core-backed `WsServer` instead of per-agent transport state
- Port: 3001

## Key Concepts Extracted

- [[agent-router]] — Multi-agent routing pattern with per-agent dispatchers
- [[connection-holder-clone-limitation]] — ConnectionHolder cannot be both a plugin and transport reference

## Current Status

These examples are no longer active source files. After Task 4, the channel crate keeps protocol and generic transport abstractions only; concrete data-plane registration, routing, event holders, sessions, tools, and provider setup live in `vol-agent-server::data_plane` [[agent-server-data-plane-core-move]].
