---
type: concept
category: pattern
tags: [retry, backoff, error-handling]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Retry with Backoff

**Category:** Error recovery pattern
**Related:** [[agent-plugin-system]], [[built-in-plugins]]

## Definition

A plugin that automatically retries failed operations using exponential backoff with configurable parameters.

## Key Points
- Exponential backoff with configurable initial delay, max delay, and multiplier [[react-agent-docs]]
- Default: 3 retries, 100ms initial delay, 5s max delay, 2x multiplier [[react-agent-docs]]
- Runs at priority 30 (last) to catch errors from all upstream plugins and agent [[react-agent-docs]]

## How It Works

```rust
let config = RetryConfig {
    max_retries: 5,
    initial_delay_ms: 200,
    max_delay_ms: 10000,
    multiplier: 1.5,
};
let plugin = RetryPlugin::new(config);
```

On error, the plugin:
1. Checks if retries remain
2. Calculates delay: `min(initial_delay * multiplier^retry_count, max_delay)`
3. Sleeps for the calculated delay
4. Retries the operation

## Related Concepts
- [[agent-plugin-system]]: How the plugin integrates
- [[built-in-plugins]]: Its place in the plugin set
- [[plugin-actions]]: Uses Continue for retry, Abort when exhausted
