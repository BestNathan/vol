---
type: concept
category: framework
tags: [router, multi-agent, dispatch, channel]
created: 2026-05-07
updated: 2026-05-07
source_count: 1
---

# Agent Router

**Category:** Multi-agent routing
**Related:** [[vol-llm-agent-channel-crate]], [[agent-dispatcher]], [[connection-holder]]

## Definition

`AgentRouter` maps agent IDs to their corresponding `AgentDispatcher` instances, enabling multi-agent services behind a single entry point.

## Key Points
- Created with `AgentRouter::new()` [[agent-channel-examples]]
- `register(agent_id, dispatcher)` adds a dispatcher for a given agent ID [[agent-channel-examples]]
- `send(agent_id, request)` routes a request to the appropriate dispatcher and returns a oneshot receiver [[agent-channel-examples]]
- `list_agents()` returns the set of registered agent IDs [[agent-channel-examples]]
- Returns `ChannelError::AgentNotFound` for unknown agent IDs [[agent-channel-examples]]

## How It Works

The router maintains an internal map of `agent_id → Arc<AgentDispatcher>`. When `send()` is called, it looks up the dispatcher by ID and forwards the request. Each dispatcher runs its agent independently via its own FIFO queue.

## Usage Pattern

```rust
let router = AgentRouter::new();
let dispatcher = Arc::new(AgentDispatcher::new(agent));
router.register("my-agent".to_string(), dispatcher.clone()).await;
let rx = router.send("my-agent", request).await?;
let result = rx.await?;
```

## Relationship to AgentDispatcher

`AgentRouter` is a higher-level abstraction than `AgentDispatcher`. A dispatcher handles a single agent's request queue; the router distributes requests across multiple dispatchers based on agent ID. They compose: each agent gets its own dispatcher, and the router selects which dispatcher to use.

## Multi-Agent Service Architecture

In a multi-agent service, each agent needs its own `ConnectionHolder` (for transport integration) and `AgentDispatcher` (for request queueing). The router holds references to dispatchers, while a separate `HashMap` holds holders for WebSocket/HTTP handler lookup. This is necessary because `ConnectionHolder` does not implement `Clone` ([[connection-holder-clone-limitation]]).
