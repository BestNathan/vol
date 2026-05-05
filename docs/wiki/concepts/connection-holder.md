---
type: concept
category: framework
tags: [connection, plugin, lifecycle, agent]
created: 2026-05-05
updated: 2026-05-05
source_count: 1
---

# Connection Holder

**Category:** Plugin lifecycle
**Related:** [[vol-llm-agent-channel-crate]], [[connection-trait]], [[agent-plugin-system]], [[http-transport]]

## Definition

`ConnectionHolder` is a struct that manages at most one active `Connection` at a time and implements the `AgentPlugin` trait to forward agent events to the attached connection.

## Key Points
- Created with `ConnectionHolder::new(sender, receiver)` for sender/receiver identity [[http-transport-impl]]
- `attach(conn)` replaces any existing connection after detaching it [[http-transport-impl]]
- `detach()` clears the current connection [[http-transport-impl]]
- `is_connected()` checks if a connection is active [[http-transport-impl]]
- Implements `AgentPlugin` with `id()` returning `"connection_holder"` [[http-transport-impl]]

## How It Works

When registered as an `AgentPlugin` on a ReActAgent, the `ConnectionHolder`'s `listen()` hook is called for every `AgentStreamEvent` the agent produces. It serializes the event to JSON and wraps it in a `Message::Event`, then calls `send()` on the currently attached `Connection`.

This creates a bridge between the agent's internal event stream and the external transport layer:

```
AgentStreamEvent → ConnectionHolder::listen() → Message::Event → Connection::send() → Transport
```

## HTTP SSE Integration

In the HTTP transport, each SSE request creates a fresh `HttpEventConnection` with a new broadcast channel, attaches it to the holder, and streams events until the agent run completes. Before attaching, the handler checks `is_connected()` and returns 409 Conflict if a connection is already active — preventing concurrent SSE requests from clobbering each other's event channels. After the SSE stream ends (success, error, or client disconnect), the connection is explicitly detached via `holder.detach()`.
