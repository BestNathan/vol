---
type: concept
category: limitation
tags: [connection-holder, clone, plugin, ownership]
created: 2026-05-07
updated: 2026-05-07
source_count: 1
---

# ConnectionHolder Clone Limitation

**Category:** Design constraint
**Related:** [[connection-holder]], [[agent-plugin-system]], [[vol-llm-agent-channel-crate]]

## Problem

`ConnectionHolder` implements `AgentPlugin` but not `Clone`. This creates an ownership conflict when the same holder needs to serve two roles:

1. **Plugin role:** Registered on a `ReActAgent` via `plugin_registry.register(holder)` — takes ownership, wraps in `Arc` internally
2. **Transport role:** Held as `Arc<ConnectionHolder>` by `WsServer`/`HttpTransport` for connection attach/detach

Since `register()` consumes the holder and there's no `Clone`, you cannot have both.

## Current Workaround

Examples create the holder independently of the agent (not registered as a plugin). The holder is used only by the transport layer for SSE event capture. Agent stream events (tool calls, thinking, content blocks) are NOT forwarded to the WebSocket connection in this configuration.

```rust
// Holder used for transport only
let holder = Arc::new(ConnectionHolder::new("my-agent".to_string(), "client".to_string()));
let dispatcher = Arc::new(AgentDispatcher::new(agent));
// Note: holder NOT registered as plugin on agent
```

## Potential Solutions

1. **Implement Clone for ConnectionHolder:** Requires the inner `Arc<Mutex<Option<Box<dyn Connection>>>>` to be cloneable, which would share the connection between clones rather than create independent copies.

2. **Arc<ConnectionHolder> in plugin registry:** Change `PluginRegistry::register()` to accept `Arc<dyn AgentPlugin>` instead of consuming `P: AgentPlugin`, allowing the caller to retain an `Arc<ConnectionHolder>` reference.

3. **Separate event forwarding mechanism:** Use a `broadcast::channel` that the agent writes to and transports subscribe to, decoupling event forwarding from the plugin system.

## Impact

Currently a documented limitation in example code. Does not block basic WS/HTTP functionality — only the real-time event forwarding (tool call events, thinking blocks) from agent to WebSocket.
