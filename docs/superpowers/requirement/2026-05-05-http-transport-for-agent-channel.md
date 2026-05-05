---
name: http-transport-for-agent-channel
description: Add HTTP transport as a reusable wrapper that can be mounted on any axum endpoint, alongside existing WebSocket transport in vol-llm-agent-channel crate.
status: draft
created: 2026-05-05
---

# Requirements: HTTP Transport for Agent Channel

## Background

The `vol-llm-agent-channel` crate currently supports WebSocket (`WsServer`) and in-memory (`MemoryConnection`) transports. `WsServer` creates its own axum `Router` with a fixed `/ws` route. The HTTP transport should follow a more flexible pattern — provide reusable axum handlers that can be mounted on any path of an existing HTTP service, rather than creating a standalone server.

## Goals

1. Add an HTTP transport implementation in `crates/vol-llm-agent-channel/src/transport/http.rs` that provides reusable axum handlers for both blocking and SSE streaming modes.
2. Support two modes via `?stream=true` query parameter:
   - **Blocking mode** (`?stream` absent): POST request blocks until agent completes, returns final result as JSON.
   - **SSE streaming mode** (`?stream=true`): POST request returns SSE stream of `Message::Event` envelopes during agent execution, followed by a final `Message::Result` event.
3. The HTTP transport is a **wrapper/handler** — not a standalone server. Users receive an `axum::Router` or handler function they can mount on any path (`/api/chat`, `/agents/v1/run`, etc.) of their existing HTTP service.
4. SSE events use the same `Message::Event { sender, receiver, event }` envelope that WebSocket uses.

## Non-Goals

1. No bidirectional streaming over HTTP (client cannot send additional messages after the initial POST — one request per agent run).
2. No `Message::Cancel` support over HTTP.
3. No changes to existing WebSocket or memory transport implementations.
4. Not creating a standalone HTTP server — just providing mountable handlers.

## Scope

### Included
- New `transport/http.rs` file with `HttpTransport` struct (analogous to `WsServer` but returning handlers, not a fixed Router)
- An `into_axum_router` or similar method that returns an axum `Router` the user can merge/mount anywhere
- POST handler with `?stream=true` support
- Blocking mode: agent runs synchronously, returns JSON result
- SSE mode: streams `AgentStreamEvent` wrapped in `Message::Event` format, then sends `Message::Result`
- Export `HttpTransport` from `transport/mod.rs` and crate root
- Basic tests for HTTP transport

### Excluded
- CORS configuration
- Authentication middleware
- Request batching
- Fixed endpoint paths

## Constraints

- Use existing dependencies already in the crate (axum, tokio, serde_json, futures, etc.)
- Similar design to `WsServer.into_axum_router()` — returns a Router that the user can `.merge()` into their own router at any path
- SSE format: standard `text/event-stream` content type with `data:` lines
- HTTP is request-response (no `recv()` from client after POST), so the HTTP connection approach differs from WebSocket

## Success Criteria

1. `cargo check -p vol-llm-agent-channel` passes with no new errors or warnings from the HTTP transport code.
2. `cargo test -p vol-llm-agent-channel` passes, including new HTTP transport tests.
3. A POST request to a user-defined endpoint (e.g., mounted at `/api/chat`) returns the agent's final result as JSON in blocking mode.
4. A POST request with `?stream=true` returns an SSE stream containing `Message::Event` wrapped agent events followed by a `Message::Result` event.
5. The HTTP transport is exported from the crate root alongside `WsServer` and `MemoryConnection`.

## Edge Cases

| Edge Case | Handling |
|-----------|----------|
| Client disconnects during SSE stream | Stop streaming gracefully, no error propagated to server |
| Agent run fails with error | Return `Message::Error` in both blocking and SSE modes |
| Dispatcher busy (another request executing) | Queue the request — dispatcher already has FIFO queue |
| Empty input string | Agent should still execute (empty input is valid) |
| SSE client never disconnects | Server closes SSE stream after final result/error is sent |
| Concurrent POST requests | Each gets its own HTTP handler; dispatcher serializes execution |

## Open Questions

None.
