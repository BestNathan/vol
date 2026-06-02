---
type: concept
category: architecture
tags: [agent-server-protocol, transport, agent-channel, message-boundary]
created: 2026-05-21
updated: 2026-05-21
source_count: 1
---

# Agent Server Protocol

**Category:** Protocol architecture
**Related:** [[vol-llm-agent-channel-crate]], [[connection-trait]], [[http-transport]], [[jsonrpc-transport]], [[agent-channel-server-protocol-transport-migration]]

## Definition

Agent Server Protocol is the typed message boundary used by `vol-llm-agent-channel` to represent commands, acknowledgements, results, events, and errors across transports. Its central type is `AgentServerMessage`, which carries `protocol`, `message_id`, `sender`, `receiver`, `kind`, `operation`, `payload`, and metadata.

## Key Points

- Transports decode incoming wire frames or HTTP bodies into `AgentServerMessage` and encode outbound `AgentServerMessage` values back to the wire.
- `AgentServerCore` owns protocol dispatch through the handler registry; transports do not call `AgentDispatcher` directly.
- Operations are grouped by domain, such as agent, file, session, skill, log, MCP, system, and permission operations.
- The protocol replaced the legacy `vol_llm_agent_channel::protocol::Message` enum for WebSocket, HTTP, and manager protocol tests.

## How It Works

A client sends an `AgentServerMessage` with `kind = Command`, an `Operation`, and a matching typed `Payload`. The transport forwards it to `AgentServerCore::handle` or `AgentServerCore::serve`. The core dispatches by operation method name to the registered domain handler and returns zero or more protocol messages, usually ack/result/error messages with the original `message_id`.

## Examples

- WebSocket: text frames contain serialized `AgentServerMessage` values; `WsConnection` parses frames and `WsServer` calls `AgentServerCore::serve`.
- HTTP: POST body is one serialized `AgentServerMessage`; response body is `Vec<AgentServerMessage>` or SSE events containing serialized protocol messages.
- Agent submit: `Payload::Agent(AgentPayload::Submit { input, target, metadata })` uses `target` to choose a registered agent.

## Related Concepts

- [[connection-trait]]: connection abstraction that transports `AgentServerMessage` values.
- [[http-transport]]: HTTP boundary now built around this protocol.
- [[jsonrpc-transport]]: JSON-RPC adapter that maps JSON-RPC method calls into protocol messages.
