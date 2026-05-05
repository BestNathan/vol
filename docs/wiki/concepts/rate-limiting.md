---
type: concept
category: pattern
tags: [rate-limiting, concurrency, semaphore]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Rate Limiting

**Category:** Concurrency control pattern
**Related:** [[agent-plugin-system]], [[plugin-actions]], [[built-in-plugins]]

## Definition

A plugin that controls concurrent agent execution using a semaphore, preventing resource exhaustion.

## Key Points
- Semaphore-based concurrency control [[react-agent-docs]]
- Priority 5: executes first before all other plugins [[react-agent-docs]]
- Acquires permit on `on_start`, releases on drop (complete or error) [[react-agent-docs]]

## How It Works

```rust
let plugin = RateLimiterPlugin::new(10); // max 10 concurrent agent runs
```

On `on_start`, the plugin attempts to acquire a semaphore permit. If no permits are available, it returns `Abort(rate_limit_exceeded)`. The permit is automatically released when the agent completes or errors, via the `Drop` implementation.

## Examples / Applications

- **API protection**: Prevent too many concurrent LLM calls that would exhaust rate limits
- **Resource management**: Limit concurrent database connections
- **Cost control**: Cap concurrent agent runs to control token spending

## Related Concepts
- [[agent-plugin-system]]: Where the plugin is registered
- [[plugin-actions]]: Uses Abort when limit is exceeded
- [[built-in-plugins]]: First plugin in the execution chain
