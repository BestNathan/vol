---
name: http-transport-design
description: Design for HTTP transport in vol-llm-agent-channel — blocking and SSE modes via reusable axum handlers.
status: draft
created: 2026-05-05
---

# HTTP Transport Design

## Architecture

The HTTP transport provides reusable axum handlers that can be mounted on any path of an existing HTTP service. Unlike `WsServer` which creates its own fixed `/ws` route, `HttpTransport` returns an axum `Router` that users can `.merge()` at any path.

```
HttpTransport (struct)
├── dispatcher: Arc<AgentDispatcher>
├── holder: Arc<ConnectionHolder>
└── agent_id: String

into_axum_router(self) -> axum::Router
├── POST /          (blocking: wait for RunResult, return JSON)
└── POST /?stream=true  (SSE: stream events, then final result)
```

## Connection Model

HTTP is request-response, not persistent bidirectional like WebSocket. Therefore:

- **No `Connection` trait implementation** — HTTP doesn't fit the `recv()/send()` model.
- **Blocking mode**: directly calls `dispatcher.submit()`, awaits `oneshot::Receiver<RunResult>`, returns JSON.
- **SSE mode**: creates an in-memory `mpsc` channel, captures events from `ConnectionHolder.listen()` during the agent run, streams them as `Message::Event` via SSE, then sends `Message::Result` and closes.

## SSE Event Flow

1. Client POSTs with `?stream=true` and an input JSON body
2. Create a broadcast channel (`tokio::sync::broadcast`) to capture events
3. Create a temporary `HttpEventConnection` — a thin wrapper that sends `Message::Event` to the broadcast channel, attached to the `ConnectionHolder`
4. Submit the request to `dispatcher.submit()` — returns `oneshot::Receiver<RunResult>`
5. Concurrently:
   - **SSE stream**: reads from the broadcast channel, formats each event as `data: {json}\n\n`
   - **Await result**: waits for `RunResult` from the dispatcher
6. When result arrives, stream it as a final SSE event, then terminate the stream

## Blocking Flow

1. Client POSTs with input JSON body
2. Submit to `dispatcher.submit()`, await `oneshot::Receiver<RunResult>`
3. Return `RunResult` as JSON body with appropriate status code

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| No `Connection` impl for HTTP | HTTP is inherently request-response; forcing a connection abstraction adds complexity with no benefit |
| SSE uses `Message::Event` envelope | Consistency with WebSocket transport; clients can use the same parser |
| `into_axum_router()` returns a `Router` | Users can `.merge()` it at any path of their existing axum service |
| `HttpEventConnection` is a thin wrapper | Implements `Connection` trait purely for `ConnectionHolder` integration; `recv()` always returns `None` |

## Error Handling

| Scenario | Blocking Mode | SSE Mode |
|----------|---------------|----------|
| Agent run fails | JSON error body (500) | Final SSE event with `Message::Error`, then stream ends |
| Dispatcher error | JSON error body (500) | SSE error event, then stream ends |
| Client disconnect | Request cancelled by framework | SSE stream terminates (axum handles this automatically) |
| Invalid request body | 400 Bad Request | 400 Bad Request |

## Files Changed

| File | Change |
|------|--------|
| `transport/http.rs` | New — `HttpTransport` struct + `HttpEventConnection` + axum handlers |
| `transport/mod.rs` | Add `mod http` + re-export `HttpTransport` |
| `lib.rs` | Re-export `HttpTransport` |
| `Cargo.toml` | May need `axum-extra` for SSE support (or use `axum::response::sse`) |

## Testing Strategy

1. **Unit tests** for `HttpTransport::into_axum_router` — mount on test router, send POST requests via `axum::test_utils`
2. **Blocking mode test** — verify JSON response contains `RunResult`
3. **SSE mode test** — verify SSE stream contains `Message::Event` entries followed by `Message::Result`
4. **Error test** — verify error handling for invalid input and agent failures
