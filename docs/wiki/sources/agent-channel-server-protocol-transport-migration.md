---
type: source
source_type: report
date: 2026-05-21
ingested: 2026-05-21
tags: [agent-channel, transport, agent-server-protocol, websocket, http, refactoring]
---

# Agent Channel Transport Migration to Agent Server Protocol

**Authors/Creators:** Claude Code and BestNathan
**Date:** 2026-05-21
**Link:** `docs/superpowers/plans/2026-05-21-agent-channel-server-protocol.md`, commits `01c4c19`, `e26a126`, `8ed0c94`

## TL;DR

`vol-llm-agent-channel` transports were refactored so WebSocket and HTTP are transport-only boundaries that decode and encode `AgentServerMessage` values and delegate all business dispatch to `AgentServerCore`. The legacy `protocol::Message` surface was removed, examples now register agents through `AgentServerCore`, and `vol-agent-manager` WebSocket protocol tests were migrated to Agent Server Protocol messages.

## Key Takeaways

- `WsServer::new` now accepts `Arc<AgentServerCore>` and serves raw Agent Server Protocol over `/ws`.
- `HttpTransport::new` now accepts `Arc<AgentServerCore>` and accepts/returns Agent Server Protocol messages over HTTP, with blocking and SSE response modes.
- Transport code no longer constructs `AgentRequest`, calls `AgentDispatcher`, depends on `ConnectionHolder`, or shapes domain-specific response bodies.
- `crates/vol-llm-agent-channel/src/protocol.rs` and the public `Message` export were deleted.
- `single_agent.rs` and `multi_agent.rs` examples now use `AgentServerCore::register_agent` and core-backed transports.
- `vol-agent-manager` WebSocket handling now serializes and matches `AgentServerMessage` instead of the deleted legacy `Message` enum.

## Detailed Summary

The migration aligns WebSocket and HTTP transport with the JSON-RPC pattern: each transport is responsible only for wire-level decoding/encoding and forwards protocol messages to `AgentServerCore`. `WsConnection` implements the `Connection` trait by parsing WebSocket text frames into `AgentServerMessage` and serializing outbound protocol messages back to text. `WsServer` owns `Arc<AgentServerCore>` and calls `core.serve(conn)` on upgrade.

`HttpTransport` now exposes a POST route that deserializes an `AgentServerMessage`, calls `core.handle(message)`, and returns a JSON array of `AgentServerMessage` responses. `?stream=true` serializes the same response messages as SSE events rather than attaching a `ConnectionHolder` or running transport-level dispatcher logic.

The cleanup removed `vol_llm_agent_channel::protocol::Message` entirely. Remaining protocol users were migrated to `AgentServerMessage`, including `vol-agent-manager/src/ws/handler.rs` and the protocol roundtrip tests in `vol-agent-manager/tests/integration.rs`. The examples were updated to use `AgentServerCore` as the only registration and dispatch layer.

Verification passed with `cargo test -p vol-llm-agent-channel`, `cargo test -p vol-agent-manager --test integration`, and grep checks confirming no `AgentDispatcher`, `AgentRequest`, `RunResult`, `ConnectionHolder`, `protocol::Message`, or `Message::Submit` references remain in `crates/vol-llm-agent-channel/src/transport`.

## Entities Mentioned

- [[vol-llm-agent-channel-crate]]: crate whose transport layer was migrated to Agent Server Protocol.
- [[vol-agent-manager-crate]]: manager WebSocket handler and integration tests migrated away from legacy `Message`.

## Concepts Covered

- [[agent-server-protocol]]: protocol boundary now used by transports and manager WebSocket messages.
- [[connection-trait]]: transport abstraction now carries `AgentServerMessage` with `recv(&self)`.
- [[http-transport]]: HTTP transport now delegates to `AgentServerCore` and returns protocol messages.
- [[jsonrpc-transport]]: reference architecture for decode/encode-at-boundary plus core dispatch.

## Notes

The migration intentionally keeps `AgentDispatcher`, `AgentRouter`, `ConnectionHolder`, `AgentRequest`, and `RunResult` available for the core runtime and non-transport internals, but removes them from WebSocket and HTTP transport responsibilities.
