---
type: concept
category: framework
tags: [transport, connection, abstraction, trait]
created: 2026-05-05
updated: 2026-05-05
source_count: 1
---

# Connection Trait

**Category:** Transport abstraction
**Related:** [[vol-llm-agent-channel-crate]], [[http-transport]], [[connection-holder]], [[remote-agent-connection]], [[tui-frontend-ratatui]]

## Definition

The `Connection` trait in `vol-llm-agent-channel` abstracts transport protocols behind a uniform interface, allowing the same agent event forwarding logic to work with WebSocket, HTTP, and in-memory transports.

## Key Points
- Trait requires `Send + Sync + 'static` for safe concurrent use [[http-transport-impl]]
- `protocol(&self) -> &str` returns a protocol identifier (e.g., "ws", "http", "memory") [[http-transport-impl]]
- `recv(&mut self) -> Option<Result<Message, ConnectionError>>` receives inbound messages [[http-transport-impl]]
- `send(&self, msg: Message) -> Result<(), ConnectionError>` sends outbound messages [[http-transport-impl]]

## Implementations

| Type | Protocol | recv() behavior | send() behavior |
|------|----------|-----------------|-----------------|
| `WsConnection` | "ws" | Reads from WebSocket stream, parses JSON | Writes JSON text to WebSocket |
| `HttpEventConnection` | "http" | Always returns `None` | Forwards to broadcast channel (minimal: only holds sender) |
| `MemoryConnection` | "memory" | Receives from `mpsc::UnboundedReceiver` | Sends to `mpsc::UnboundedSender` |

## Design Rationale

The trait enables `ConnectionHolder` to work uniformly across all transports. When a `ConnectionHolder` is registered as an `AgentPlugin`, its `listen()` hook serializes each `AgentStreamEvent` to JSON and sends it as a `Message::Event` to whichever connection is currently attached.
