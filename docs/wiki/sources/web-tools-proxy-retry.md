---
type: source
source_type: code
date: 2026-08-12
ingested: 2026-08-12
tags: [web-tools, proxy, retry, web-fetch, web-search]
---

# Web Tools Proxy and Retry Support

**Authors/Creators:** BestNathan
**Date:** 2026-08-12
**Link:** crates/vol-llm-tool/src/web/, crates/vol-llm-tools-builtin/

## TL;DR

Round 1: Added three-tier proxy configuration and exponential-backoff retry logic to `web_fetch` and `web_search` tools. Proxy priority: tool parameter > agent config > environment variable. Retry uses exponential backoff (default 3 attempts, 1s base delay).

Round 2: Added response caching (JSON file-based, `.vol/cache/tools/web_fetch/`, 15min TTL for success, 5min for errors), cross-host redirect detection (manual redirect loop, only same-host followed), pre-processing body truncation, and structured status markers (`<fetch success>`, `<fetch from cache>`, `<fetch redirect>`, `<fetch error>`, `<fetch success truncated>`).

## Key Takeaways

- `ProxyConfig::resolve()` implements the priority chain: override → config → env (`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`)
- `RetryConfig` with `retry_async()` helper provides exponential backoff for transient network errors
- `WebSearchParams` and `WebFetchParams` now accept optional `proxy_url` from the LLM at call time
- Agent YAML configs can set per-tool `proxy` and `retry` under `tool_configs`
- `register_tool_by_name` in `vol-llm-yaml-agent` now reads `tool_configs` from YAML
- Providers (`DefaultFetchProvider`, `TavilySearchProvider`) build per-request clients with the resolved proxy

## Detailed Summary

### ProxyConfig Enhancement (`vol-llm-tool/src/web/proxy.rs`)

Added `from_env()` and `resolve()` methods:
- `from_env()`: reads `HTTPS_PROXY` > `http_proxy` > `HTTP_PROXY` > `http_proxy` > `ALL_PROXY` > `all_proxy`
- `resolve(override)`: tool param > self.proxy_url > env var, with empty-string check

### Retry Module (`vol-llm-tool/src/web/retry.rs`)

New module with:
- `RetryConfig`: `max_attempts` (default 3), `base_delay_ms` (default 1000)
- `retry_async()`: generic async retry helper with exponential backoff
- `should_retry()`: checks error string for timeout/connection/DNS/reset/TLS/EOF patterns

### Tool Parameter Changes

`WebSearchParams` and `WebFetchParams` gained optional `proxy_url` field. The tool's `parameters()` JSON schema now includes `proxy_url` with description.

### Tool Struct Changes

`WebSearchTool` and `WebFetchTool` now store `proxy_config: ProxyConfig` alongside the provider. New `with_proxy()` constructor. In `execute()`, the tool resolves the effective proxy and passes it via `opts.proxy_url` to the provider.

### Provider Changes

- `DefaultFetchProvider`: no longer stores a pre-built `Client`; builds per-request client via `build_client()` using resolved proxy; wraps fetch in `retry_async()`
- `TavilySearchProvider`: same pattern — per-request client construction, `retry_async()` wrapper
- Both `FetchProviderConfig` and `TavilyConfig` gained `retry: RetryConfig` field

### Config Wiring

- `WebSearchConfig` and `WebFetchConfig` in `vol-llm-tools-builtin/src/config.rs` now include `retry: RetryConfig`
- `register_web_all()` passes both `proxy` and `retry` configs, uses `with_proxy()` constructors
- `register_tool_by_name()` in `vol-llm-yaml-agent` now accepts `Option<&ToolConfig>` and reads tool configs from the YAML agent definition

## Entities Mentioned

- [[vol-llm-tool-crate]]: enhanced ProxyConfig, new RetryConfig and retry_async
- [[vol-llm-tools-builtin]]: updated config types, provider implementations
- [[vol-llm-yaml-agent-crate]]: register_tool_by_name now uses agent config

## Concepts Covered

- [[retry-with-backoff]]: new web-tool-specific retry module alongside the existing plugin retry
- [[tool-registry]]: tools now carry per-tool proxy config for execution-time resolution
- [[proxy-config-resolution]]: new concept — three-tier priority chain for proxy resolution

## Notes

- `tavily.rs` coverage remains 0% (requires real Tavily API key); integration tests use mock providers
- The previous plugin-based `RetryPlugin` (in agent plugin system) is separate from this web-tool-level retry
- Per-request client building adds overhead vs a shared client; future optimization could cache per-proxy clients
