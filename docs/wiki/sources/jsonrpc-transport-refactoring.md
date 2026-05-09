---
type: source
source_type: report
date: 2026-05-09
ingested: 2026-05-09
tags: [json-rpc, transport, connection-trait, refactoring, vol-llm-agent-channel]
---

# JSON-RPC Transport Refactoring

**Authors/Creators:** Claude Code with user guidance
**Date:** 2026-05-09
**Link:** `docs/superpowers/specs/2026-05-09-jsonrpc-transport-design.md`, `docs/superpowers/plans/2026-05-09-jsonrpc-transport.md`

## TL;DR

Replaced the duplicate EventBridgePlugin + JsonRpcHandler architecture with `JsonRpcConnection` implementing the `Connection` trait, plugging into the existing `ConnectionHolder` plugin system.

## Key Takeaways

- `EventBridgePlugin` was redundant with `ConnectionHolder` — both forwarded agent events, just with different output formats. Deleted.
- `JsonRpcHandler` + `JsonRpcContext` bypassed the `Connection` trait entirely. Replaced by `JsonRpcConnection` + `JsonRpcServer`.
- `JsonRpcConnection` implements `Connection` trait (`protocol() → "jsonrpc-ws"`, `send()`, `recv()`) and translates between `Message` and JSON-RPC wire format.
- Multi-agent support: `JsonRpcServer` accepts `Vec<AgentRegistration>` at startup, builds an `AgentRouter` internally, and all holders are attached at connection startup — no detach/attach switching needed.
- Wire format preserved — frontend receives identical JSON-RPC events with no code changes.
- 49 integration tests cover all `AgentStreamEvent` variants, all 12 JSON-RPC methods, and error handling.

## Detailed Summary

The previous `jsonrpc` module in `vol-llm-agent-channel` had its own `JsonRpcHandler`, `JsonRpcContext`, and `EventBridgePlugin` — completely separate from the `Connection` trait system that `WsConnection` and `MemoryConnection` used. This created two parallel event-bridging mechanisms:

1. `ConnectionHolder` (AgentPlugin) forwards events through `Connection` trait
2. `EventBridgePlugin` (AgentPlugin) forwards events through a separate `broadcast::Sender`

The refactoring unified these by making `JsonRpcConnection` implement the `Connection` trait. Now `ConnectionHolder` is the single event bridge for all transports. The JSON-RPC wire format (the envelope structure with `subscription`, `req_id`, `event_type`, `data`) is preserved through `serialize_agent_event()` and `to_jsonrpc_event()` in `serde_helpers.rs`.

### New Architecture

```
ReActAgent
  └── ConnectionHolder (AgentPlugin)
       └── Connection::send(Message::Event)
            ├── WsConnection (raw binary)
            ├── JsonRpcConnection (JSON-RPC 2.0 text)
            └── MemoryConnection (mpsc channel)
```

### Key Files Changed

- **Created:** `src/jsonrpc/connection.rs` — `JsonRpcConnection` with 11 handler methods
- **Created:** `src/jsonrpc/server.rs` — `JsonRpcServer` with `AgentRegistration`
- **Created:** `src/jsonrpc/serde_helpers.rs` — serialization helpers (moved from handler.rs)
- **Created:** `tests/jsonrpc_integration.rs` — 49 integration tests
- **Deleted:** `src/jsonrpc/handler.rs` — old `JsonRpcHandler`, `JsonRpcContext`, `EventBridgePlugin` (564 lines)
- **Modified:** `crates/vol-llm-core/src/stream.rs` — added `Deserialize` derive to `AgentStreamEvent`

### Design Decisions

- **All holders attached at startup:** Rather than detach/attach switching, the connection attaches to all registered agents' holders at startup. Events from all agents flow through the same WebSocket. The frontend distinguishes events by content.
- **Optional target field in agent.submit:** Clients can specify which agent to submit to via an optional `target` parameter. Falls back to first registered agent if omitted.
- **File/session/log operations handled in run() loop:** These are JSON-RPC-only methods handled directly by the connection, not through the `Connection` trait.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: The crate being refactored

## Concepts Covered

- [[connection-trait]]: JsonRpcConnection now implements this trait
- [[connection-holder]]: Single event bridge after EventBridgePlugin deletion
- [[agent-router]]: Used internally by JsonRpcServer for multi-agent dispatch
- [[agent-dispatcher]]: FIFO request queueing, used by JsonRpcServer
- [[json-rpc-websocket]]: Wire format preserved across refactoring
- [[agent-event-stream]]: AgentStreamEvent serialization moved to serde_helpers
