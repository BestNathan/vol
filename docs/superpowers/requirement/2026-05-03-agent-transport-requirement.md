# Requirements: Abstract Transport Layer for Agent Communication

## Background

The `vol-llm-agent-channel` crate provides `AgentDispatcher` (FIFO queue) and `AgentRouter` (multi-agent routing), but currently only supports in-process communication. We need an abstract transport mechanism that allows clients to connect to agents via various protocols (WebSocket, HTTP, etc.) while supporting bidirectional streaming of agent events.

## Goals

1. **Transport trait** — Define an abstract `Transport` trait that protocols (WebSocket, HTTP, in-memory) implement
2. **Transport is an AgentPlugin** — Each transport instance implements `AgentPlugin`, registered to the agent's `PluginRegistry`, receiving all `AgentStreamEvent`s via `listen()` for real-time streaming to clients
3. **Bidirectional streaming** — Clients can send requests and receive both streaming events (thinking, tool calls) and the final `RunResult`
4. **In-memory transport** — Provide a built-in in-memory transport for local testing and inter-process communication
5. **WebSocket transport** — Provide a WebSocket transport implementation for remote access
6. **Per-connection transport** — Each client connection (WS connection, in-memory session) gets its own `Transport` instance bound to that connection
7. **Agent-to-agent over network** — Transports enable remote agent-to-agent communication (Agent A's transport sends request to Agent B's transport)

## Non-Goals

1. **Mid-execution cancellation over transport** — Only queue-level cancellation supported (same as dispatcher)
2. **Concurrent execution** — Transport does not change the single-threaded FIFO behavior
3. **Authentication / authorization** — Transport passes connections through as-is; auth is out of scope
4. **Message persistence** — Transport does not store or replay messages
5. **gRPC transport** — Not in initial scope; trait design should not preclude it

## Scope

### Included

- `Transport` trait definition in `vol-llm-agent-channel`
- `Transport` implements `AgentPlugin` — registered to agent, receives events via `listen()`
- `Transport` can also call `dispatcher.submit()` to execute requests from clients
- In-memory transport implementation
- WebSocket transport implementation (uses `axum` + `tokio-tungstenite` already in workspace)
- JSON message protocol for transport communication (request, event, result, cancel)

### Excluded

- Changes to `ReActAgent` internals
- Changes to existing `AgentDispatcher` core logic (transport integrates via plugin, not by modifying dispatcher)
- CLI or TUI transport client
- HTTP REST API transport

## Constraints

1. **Async Rust** — tokio-based async transport implementations
2. **Existing deps** — Use `axum` (already has `ws` feature), `tokio-tungstenite`, `serde_json` from workspace
3. **No blocking** — All transport operations are async
4. **Clone-friendly** — Transport instances must be `Send + Sync` and cloneable

## Success Criteria

1. A new `Transport` trait can be implemented for any protocol
2. A `Transport` instance can be registered as a plugin on a `ReActAgent`
3. When the agent runs, the transport receives all `AgentStreamEvent`s via `listen()`
4. A client connected via WebSocket can send a request and receive:
   - Streaming events (thinking deltas, tool call progress) in real-time
   - The final `RunResult` when execution completes
5. The in-memory transport allows local code to communicate with an agent without network overhead
6. Multiple simultaneous WebSocket connections can each communicate with different agents
7. The WebSocket server can be started with minimal boilerplate (< 20 lines)

## Edge Cases

| Edge Case | Behavior |
|-----------|----------|
| Client disconnects during agent execution | Transport plugin still receives events but forwarding silently fails (no error) |
| Client sends invalid JSON message | Transport sends error response back |
| Client sends request while agent is busy | Request queued, client receives confirmation of queue position |
| Client sends cancel for already-executing request | Returns false, no effect |
| WebSocket server receives connection on unknown route | Rejects with 404 |
| Transport plugin not registered | Agent runs normally, no transport events emitted |

## Open Questions

None at this time.
