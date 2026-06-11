---
type: concept
category: framework
tags: [http, transport, sse, axum]
created: 2026-05-05
updated: 2026-06-10
source_count: 2
---

# HTTP Transport

**Category:** Network transport
**Related:** [[vol-llm-agent-channel-crate]], [[connection-trait]], [[agent-server-protocol]], [[agent-channel-server-protocol-transport-migration]], [[agent-channel-examples]], [[mcp-transport-pattern]]

## Definition

Historical HTTP transport implementation for `vol-llm-agent-channel` using axum 0.7. It was deleted during the Task 4 data-plane boundary cleanup; the active channel crate API now keeps WebSocket/JSON-RPC and memory transports only.

## Key Points
- Deleted from the active `vol-llm-agent-channel` source during Task 4 cleanup after concrete data-plane behavior moved into `vol-agent-server` [[agent-server-data-plane-core-move]]
- Former `HttpTransport` accepted `AgentServerMessage` JSON bodies and could return blocking JSON or SSE events [[http-transport-impl]]
- Current active remote transport for the channel crate is JSON-RPC 2.0 over WebSocket via `JsonRpcServer<S>` and `JsonRpcConnection` [[jsonrpc-transport]]
- Historical details below describe the removed implementation for archaeology only; do not treat them as current API

## Blocking Mode

Simple request-response: client POSTs JSON body with `input`, `req_id` (optional), `metadata` (optional). The handler submits to `AgentDispatcher`, awaits the oneshot result, and returns a JSON response with `success`, `response`/`error`, and metadata fields.

## SSE Mode

1. Checks `ConnectionHolder.is_connected()` — returns 409 if already active
2. Creates a `broadcast::channel::<Message>(100)` for event capture
3. Creates `HttpEventConnection` and attaches it to `ConnectionHolder`
4. Submits request to `AgentDispatcher`
5. Spawns a task to await the dispatcher result via oneshot
6. Uses `tokio::select!` to merge the broadcast event stream with the done signal
7. After receiving the final result, drops `event_tx` so `event_rx` drains buffer then returns `Closed`
8. On stream end (success, error, or dispatcher failure), detaches from `ConnectionHolder`

## HttpEventConnection

Implements the `Connection` trait for `ConnectionHolder` integration:
- `protocol()` returns `"http"`
- `recv()` always returns `None` (HTTP is request-response, no inbound after POST)
- `send()` forwards messages to the broadcast channel
- Minimal struct: only holds `broadcast::Sender<Message>` (no unused sender/receiver fields)

## Concurrency Safety

`ConnectionHolder` holds at most one active connection. If a second SSE request arrives while one is active, it receives a 409 Conflict response. This prevents events from one client being sent to another's broadcast channel.

## Design Decision: Router Ownership

Unlike `WsServer` which creates a fixed `/ws` route, `HttpTransport` takes `self` in `into_axum_router()` and returns ownership of the `Router`. This allows users to `.merge()` it at any path in their own service composition.
