---
type: concept
category: pattern
tags: [retry, backoff, error-handling, web-tools]
created: 2026-05-04
updated: 2026-08-12
source_count: 2
---

# Retry with Backoff

**Category:** Error recovery pattern
**Related:** [[agent-plugin-system]], [[built-in-plugins]], [[web-tools-proxy-retry]], [[proxy-config-resolution]]

## Definition

Two implementations of exponential backoff retry exist in the codebase:

1. **Agent plugin** (`RetryPlugin`): retries entire agent operations at the plugin level
2. **Web tool retry** (`vol_llm_tool::web::retry`): retries individual HTTP requests within web_fetch/web_search tools

## Agent Plugin Retry

### Key Points
- Exponential backoff with configurable initial delay, max delay, and multiplier
- Default: 3 retries, 100ms initial delay, 5s max delay, 2x multiplier
- Runs at priority 30 (last) to catch errors from all upstream plugins and agent

### How It Works

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

## Web Tool Retry (`vol_llm_tool::web::retry`)

Added 2026-08-12 [[web-tools-proxy-retry]].

### Key Points
- `RetryConfig` with `max_attempts` (default 3) and `base_delay_ms` (default 1000ms)
- `retry_async()`: generic async helper that works with any `Future<Output = Result<T, E>>`
- Retries only on transient errors (timeout, connection refused, DNS, reset, TLS, EOF, broken pipe)
- 4xx errors do NOT trigger retry
- Configurable per-tool via agent YAML `tool_configs`

### Example Config (TOML)

```toml
[tools.web_fetch.retry]
max_attempts = 5
base_delay_ms = 2000
```

## Related Concepts
- [[agent-plugin-system]]: How the plugin integrates
- [[built-in-plugins]]: Its place in the plugin set
- [[proxy-config-resolution]]: Proxy priority chain used alongside retry in web tools
- [[web-tools-proxy-retry]]: Source document for the web tool retry implementation
