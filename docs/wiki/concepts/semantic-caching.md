---
type: concept
category: pattern
tags: [caching, semantic-search, ttl]
created: 2026-05-04
updated: 2026-05-04
source_count: 1
---

# Semantic Caching

**Category:** Caching pattern
**Related:** [[agent-plugin-system]], [[plugin-actions]], [[built-in-plugins]]

## Definition

A plugin that caches agent responses using semantic similarity matching, returning cached results for semantically similar queries to avoid redundant LLM calls.

## Key Points
- Uses semantic similarity (not exact match) for cache lookups [[react-agent-docs]]
- Configurable TTL for cache entries [[react-agent-docs]]
- `on_start`: checks cache for user_input, short-circuits on hit [[react-agent-docs]]
- `on_complete`: caches final response if not a cache hit [[react-agent-docs]]
- Supports shared cache instance across multiple agents [[react-agent-docs]]

## How It Works

1. On agent start, the plugin computes a semantic embedding of the user input
2. If a semantically similar query exists in cache (within threshold), returns `ShortCircuit(cached_response)`
3. On agent complete, the final response is stored in cache with TTL
4. Cache entries expire after TTL, preventing stale responses

Usage:
```rust
let plugin = CachingPlugin::new(300); // 5-minute TTL
let shared_cache = SemanticCache::new();
let plugin = CachingPlugin::new(300).with_cache(shared_cache.clone());
```

## Related Concepts
- [[agent-plugin-system]]: Where the plugin is registered
- [[plugin-actions]]: Uses ShortCircuit for cache hits
- [[built-in-plugins]]: Priority 20, runs before expensive LLM calls
