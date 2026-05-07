---
type: source
category: implementation
tags: [examples, agent, channel, websocket, http, axum]
created: 2026-05-07
updated: 2026-05-07
---

# Agent Channel Examples: WS + HTTP Service

**Source Type:** Implementation
**Related:** [[vol-llm-agent-channel-crate]], [[agent-router]], [[connection-holder]], [[agent-dispatcher]], [[http-transport]]

## Summary

Two example applications in `crates/vol-llm-agent-channel/examples/` demonstrate how to build runnable agent services using `vol-llm-agent-channel` primitives with real LLM providers.

## Examples Created

### single_agent.rs

A single `ReActAgent` with dual transport (WebSocket + HTTP) behind one axum server.

- Endpoints: `/ws` (WS), `/api/chat` (HTTP POST), `/api/chat?stream=true` (SSE), `/health`
- Uses `WsServer` and `HttpTransport` with shared `AgentDispatcher` and `ConnectionHolder`
- LLM provider created from env vars (DashScope Anthropic endpoint)
- Port: 3000

### multi_agent.rs

Multiple `ReActAgent` instances registered with `AgentRouter`, each accessible via path parameter.

- Endpoints: `/ws/:agent_id` (WS), `/api/chat/:agent_id` (HTTP POST), `/api/agents` (list), `/health`
- Three agents: translator, summarizer, coder — each with different system prompts
- `AppState` struct with `router`, `dispatchers`, and `holders` maps
- Custom handlers extract `agent_id` from path for per-agent lookup
- Port: 3001

## Key Concepts Extracted

- [[agent-router]] — Multi-agent routing pattern with per-agent dispatchers
- [[connection-holder-clone-limitation]] — ConnectionHolder cannot be both a plugin and transport reference

## Known Limitations

**ConnectionHolder Clone Issue:** `ConnectionHolder` implements `AgentPlugin` but not `Clone`. The `plugin_registry.register()` method takes ownership and wraps in `Arc`, making it impossible to also hold an `Arc<ConnectionHolder>` for transport use. The examples document this with NOTE comments. A future refactor would need either `Clone` on `ConnectionHolder` or a separate event-forwarding mechanism.
