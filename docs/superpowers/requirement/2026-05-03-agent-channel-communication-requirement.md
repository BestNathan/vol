# Requirements: Agent Channel Communication

## Background

The current `ReActAgent.run()` is a direct call: the caller blocks until the agent loop completes. This means:
- No way to queue multiple concurrent requests
- No way to cancel a pending request
- No structured request metadata
- No reusable communication pattern for agents

We need a channel-based communication layer that adds request queuing, cancellation, and reply routing on top of `ReActAgent`.

## Goals

1. **Submit-and-await pattern** — Caller calls `submit(request)` and receives a `oneshot::Receiver<AgentResponse>`, allowing the caller to await the final response asynchronously
2. **Structured request** — Each request carries a `run_id`, `user_input`, optional `session_id`, and extensible metadata map
3. **Single-threaded FIFO queue** — Only one execution runs at a time; new requests queue and wait for their turn
4. **Queue-only cancellation** — A queued request (not yet started) can be cancelled by the sender via `cancel(run_id)`. The currently executing request cannot be cancelled and runs to completion
5. **Reply routing** — Each sender receives only its own response, identified by `run_id`

## Non-Goals

1. **Concurrent execution** — Multiple `ReActAgent` runs in parallel is out of scope
2. **Mid-execution cancellation** — Cancelling a currently running agent loop is out of scope
3. **Streaming events to sender** — The channel returns only the final `AgentResponse`. Streaming events (`AgentStreamEvent`) remain available through the existing broadcast channel on `RunContext`
4. **Generic agent abstraction** — The dispatcher is designed specifically for `ReActAgent`. Other agents that wrap `ReActAgent` inherit this layer
5. **Persistent queue** — The queue is in-memory; if the process restarts, queued requests are lost

## Scope

### Included

- New crate `vol-llm-agent-channel` in the workspace
- `AgentRequest` struct: structured request type
- `AgentDispatcher` struct: manages the request queue and executes requests one at a time
- `submit(request) -> (run_id, oneshot::Receiver<AgentResponse>)`: submit a request
- `cancel(run_id) -> bool`: cancel a queued request (returns false if already executing or completed)
- FIFO execution order
- Wraps an existing `ReActAgent` instance, calling `run(user_input)` for each request

### Excluded

- Changes to `ReActAgent` internals
- WebSocket/HTTP API layer
- CLI or TUI changes
- Cross-process communication

## Constraints

1. **Async Rust** — Uses tokio async primitives (`tokio::sync::mpsc`, `tokio::sync::oneshot`)
2. **Existing types** — Reuses `ReActAgent`, `AgentResponse`, `AgentError` from `vol-llm-agent`
3. **Thread-safe** — `AgentDispatcher` must be `Send + Sync` and clonable/shareable across tasks
4. **No blocking** — All operations are async; no `block_on` or synchronous waits

## Success Criteria

1. `submit()` returns immediately with a `oneshot::Receiver`; the sender can `.await` the result
2. Multiple `submit()` calls queue correctly; responses are delivered in FIFO order
3. `cancel(run_id)` on a queued request prevents it from executing; the sender's receiver returns an error (channel closed)
4. `cancel(run_id)` on a currently executing request returns `false` and does not affect the running execution
5. Each sender receives the correct `AgentResponse` matching its `run_id`
6. The dispatcher handles agent errors correctly — the `AgentError` propagates to the sender's receiver

## Edge Cases

| Edge Case | Behavior |
|-----------|----------|
| Dispatcher receives request while agent is running | Request is queued, `oneshot::Receiver` returned immediately |
| Multiple requests queued | Executed in FIFO order, one at a time |
| Cancel a queued request | Request removed from queue, sender's channel closed with error |
| Cancel a currently executing request | Returns `false`, no effect on execution |
| Cancel an already completed request | Returns `false`, request no longer in queue |
| Agent run fails with `AgentError` | Error returned to sender via `oneshot::Receiver` |
| Dispatcher dropped while requests are queued | All pending senders receive channel errors |
| Duplicate `run_id` submitted | System assigns new `run_id` internally; original `run_id` preserved in response |

## Open Questions

None at this time.
