---
type: concept
category: framework
tags: [json-rpc, websocket, remote, jsonrpsee]
created: 2026-05-08
updated: 2026-05-08
source_count: 1
---

# JSON-RPC WebSocket

**Category:** Network protocol

**Related:** [[vol-llm-ui-crate]], [[vol-llm-agent-channel-crate]], [[remote-agent-connection]]

## Definition

JSON-RPC 2.0 over WebSocket as the protocol for remote agent communication. The client (`RemoteConnection` in `vol-llm-ui`) connects to a WebSocket server in `vol-llm-agent-channel` and issues JSON-RPC requests for agent operations.

## Key Points
- Uses jsonrpsee 0.26 with `ws-client` feature for client-side implementation [[remote-connection-impl]]
- Connection established via `WsClientBuilder::default().build(ws_url)` [[remote-connection-impl]]
- New WebSocket connection created per `rpc_call()` — no persistent connection state in `rpc_call` itself [[remote-connection-impl]]
- Named parameters use `ObjectParams` rather than positional `rpc_params!` macro [[remote-connection-impl]]
- `ClientT` trait provides the `.request(method, params)` async method [[remote-connection-impl]]

## Protocol Design
- JSON-RPC 2.0 standard with `jsonrpc`, `method`, `params`, `id` fields
- Server pushes events via notification methods (e.g., `ui.event`)
- Client initiates all operations via request-response pattern

## Comparison with Other Transports

| Aspect | WebSocket (JSON-RPC) | HTTP SSE | Memory |
|--------|---------------------|----------|--------|
| Direction | Bidirectional | Server-push only | Bidirectional |
| Protocol | JSON-RPC 2.0 | SSE text/event-stream | mpsc channels |
| Use Case | Remote agent service | HTTP streaming | Testing |
| Crate | `vol-llm-ui` (client) | `vol-llm-agent-channel` | `vol-llm-agent-channel` |
| Connection persistence | Per-call | Per-SSE-request | Direct handle |

## Auto-Reconnect

The `submit()` method implements exponential backoff on failure:
- Maximum 5 retries
- Delay: `min(1000 * 2^(retry-1), 30000)` milliseconds
- Range: 1s, 2s, 4s, 8s, 16s (capped at 30s)
- After all retries fail, sends `UiEvent::AgentError` to the receiver
