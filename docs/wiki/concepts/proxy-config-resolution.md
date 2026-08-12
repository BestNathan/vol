---
type: concept
category: pattern
tags: [proxy, configuration, web-tools, priority-chain]
created: 2026-08-12
updated: 2026-08-12
source_count: 1
---

# Proxy Config Resolution

**Category:** Configuration pattern
**Related:** [[web-tools-proxy-retry]], [[retry-with-backoff]], [[vol-llm-tool-crate]]

## Definition

A three-tier priority chain for resolving the effective proxy URL for web tools (`web_fetch`, `web_search`). Each tier overrides the one below it.

## Priority Chain

```
1. Tool parameter (LLM-provided at call time)  ← highest priority
2. Agent config    (YAML/TOML tool_configs)     ← middle
3. Environment var (HTTPS_PROXY/HTTP_PROXY/ALL_PROXY) ← lowest
```

## Implementation

`ProxyConfig::resolve(override_url: Option<&str>) -> Option<String>` in `vol_llm_tool::web::proxy`:

```rust
// 1. Tool parameter check
if let Some(url) = override_url {
    if !url.is_empty() { return Some(url); }
}
// 2. Agent config check
if let Some(ref url) = self.proxy_url {
    if !url.is_empty() { return Some(url); }
}
// 3. Environment variable fallback
ProxyConfig::from_env().proxy_url
```

## Environment Variables Checked

`ProxyConfig::from_env()` checks in this order:
- `HTTPS_PROXY` / `https_proxy`
- `HTTP_PROXY` / `http_proxy`
- `ALL_PROXY` / `all_proxy`

## Configuration Examples

### Agent YAML

```yaml
tool_configs:
  web_fetch:
    proxy:
      proxy_url: "http://proxy.internal:8080"
    retry:
      max_attempts: 5
```

### Agent TOML

```toml
[tools.web_search.proxy]
proxy_url = "socks5://proxy.internal:1080"

[tools.web_search.retry]
max_attempts = 3
base_delay_ms = 1000
```

### Runtime (LLM parameter)

```json
{"url": "https://example.com", "proxy_url": "http://192.168.1.1:3128"}
```

## How Tools Use It

1. `WebSearchTool` / `WebFetchTool` store a `ProxyConfig` from the agent config
2. At execution time, `params.proxy_url` (from LLM) is passed to `proxy_config.resolve()`
3. The resolved URL is passed to the provider via `FetchOptions.proxy_url` / `SearchOptions.proxy_url`
4. The provider builds a per-request `reqwest::Client` with the effective proxy

## Related Concepts
- [[retry-with-backoff]]: Retry logic that accompanies proxy use in web tools
- [[web-tools-proxy-retry]]: Source document for the full implementation
