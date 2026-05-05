---
type: source
source_type: implementation
date: 2026-05-05
ingested: 2026-05-05
tags: [http, transport, sse, axum, vol-llm-agent-channel]
---

# HTTP Transport Implementation

**Authors/Creators:** vol-monitor team
**Date:** 2026-05-05
**Link:** `crates/vol-llm-agent-channel/src/transport/http.rs`

## TL;DR

Added HTTP transport to `vol-llm-agent-channel` crate. `HttpTransport` provides reusable axum handlers mountable at any path, supporting blocking (JSON request-response) and SSE (Server-Sent Events streaming) modes.

## Key Takeaways
- `HttpTransport` holds `Arc<AgentDispatcher>`, `Arc<ConnectionHolder>`, and `agent_id`
- `into_axum_router()` consumes self and returns an axum `Router` with POST `/` endpoint
- `?stream=true` query parameter enables SSE mode; default is blocking JSON response
- `HttpEventConnection` implements `Connection` trait, forwarding events to a broadcast channel
- SSE mode merges broadcast event stream with oneshot completion signal via `tokio::select!`
- `Message` enum gained `Clone` derive to support tokio broadcast channels
- Added `async-stream = "0.3"` and `axum` `query` feature as dependencies

## Detailed Summary

The HTTP transport fills the gap between WebSocket (bidirectional, long-lived) and memory (testing only) transports. It targets simple REST-style clients and SSE streaming use cases.

### Architecture

```
POST /?stream=true → handle_sse() → HttpEventConnection → broadcast::channel → Sse stream
POST /             → handle_blocking() → dispatcher.submit() → await → JSON response
```

### Blocking Mode
- Parses `HttpRequestBody` (input, req_id, metadata)
- Submits to `AgentDispatcher` via `submit()`
- Awaits oneshot receiver for `RunResult`
- Returns JSON with success/error and response data

### SSE Mode
- Checks `is_connected()` first — returns 409 Conflict if already active
- Creates broadcast channel (capacity 100) for event capture
- Creates `HttpEventConnection` and attaches to `ConnectionHolder`
- Submits request to dispatcher
- Spawns task to await dispatcher result, sends final `Message::Result` or `Message::Error` via oneshot
- Merges event stream with done signal using `tokio::select!` (biased toward done)
- On done: drops `event_tx` so `event_rx` drains buffer then returns `Closed` (no 100ms sleep needed)
- On stream end: explicitly detaches from `ConnectionHolder`
- On dispatcher error before submit: detaches and returns 500

### Tests
5 tests added covering:
1. Blocking mode returns JSON result with `success: true`
2. SSE mode returns `text/event-stream` content type
3. Invalid JSON body returns 400 Bad Request
4. Empty input string still succeeds
5. Concurrent SSE requests — second gets 409 Conflict (tested via TCP server with slow mock LLM)

### Design Decisions
- `HttpEventConnection.recv()` always returns `None` since HTTP is request-response
- Router takes ownership in `into_axum_router()` for flexible path composition
- `Message` derive changed from `Debug, Serialize, Deserialize` to `Debug, Clone, Serialize, Deserialize`

## Entities Mentioned
- [[vol-llm-agent-channel-crate]]: Crate where this is implemented
- [[vol-llm-agent-crate]]: ReActAgent used by the dispatcher

## Concepts Covered
- [[http-transport]]: The new HTTP transport implementation
- [[connection-trait]]: Abstraction that HttpEventConnection implements
- [[connection-holder]]: Plugin that forwards events to the connection
- [[agent-dispatcher]]: FIFO request queueing used by both modes
