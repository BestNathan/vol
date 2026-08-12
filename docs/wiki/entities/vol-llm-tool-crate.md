---
type: entity
category: product
tags: [crate, tools, rust, registry, web, proxy, retry]
created: 2026-05-04
updated: 2026-08-12
source_count: 3
---

# vol-llm-tool Crate

**Category:** Rust crate — Tool definition and execution framework
**Related:** [[tool-registry]], [[vol-llm-agent-crate]], [[vol-llm-mcp-crate]], [[mcp-client-integration]], [[retry-with-backoff]]

## Overview

Provides the `Tool` trait, `ToolRegistry`, and execution framework for agent tools. Also contains the `web` module with abstract provider traits (`FetchFn`, `SearchFn`), reusable proxy configuration, and retry logic for web tools.

## Key Facts
- `Tool` trait: name, description, parameters schema, async execute
- `ToolRegistry`: HashMap-based registration and dispatch
- `ToolContext`: provides alert info, message history, and custom metadata
- `ToolResult`: structured result with content, error, and optional structured data
- `ProxyConfig`: three-tier proxy resolution (tool param > agent config > env var)
- `RetryConfig`: exponential backoff with `retry_async()` helper for transient network errors

### Web module (`src/web/`)

| Module | Purpose |
|--------|---------|
| `fetch.rs` | `FetchFn` trait, `FetchOptions`, `FetchResult` |
| `search.rs` | `SearchFn` trait, `SearchOptions`, `SearchResult`, `SearchItem` |
| `proxy.rs` | `ProxyConfig` with `from_env()` and `resolve()` |
| `retry.rs` | `RetryConfig` and `retry_async()` generic retry helper |

## Timeline
- **2026-04**: Tool framework implemented
- **2026-05-21**: `McpTool` compile blocker fixed
- **2026-08-12**: Proxy priority chain and retry module added to web tools [[web-tools-proxy-retry]]
