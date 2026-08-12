# Web Fetch: Cache, Redirect Detection & Structured Output

**Date:** 2026-08-12
**Status:** approved

## Overview

Add response caching (all result types), cross-host redirect detection, and structured status markers to the `DefaultFetchProvider` in `vol-llm-tools-builtin-web-fetch`.

## Design

### 1. Output Status Markers

Extend `web::fetch` with a `FetchStatus` enum covering all outcomes. Every response includes a status tag.

| Status | Tag | Cached? | TTL |
|--------|-----|---------|-----|
| Fresh success | `<fetch success>` | Yes | 15 min |
| Fresh success (body truncated before extraction) | `<fetch success truncated>` | Yes | 15 min |
| Cross-host redirect | `<fetch redirect>` | Yes | 15 min |
| HTTP/network error | `<fetch error>` | Yes | 5 min |
| Cache hit (any type) | `<fetch from cache>` | — | — |

Cache hits prepend `<fetch from cache>\n` before the cached status tag.

### 2. Cache Storage

- **Location:** `.vol/cache/tools/web_fetch/{sha256(url)}.json`
- **Format:** JSON file

```json
{
  "url": "<original URL>",
  "status": "success" | "truncated" | "redirect" | "error",
  "title": "<page title or null>",
  "content": "<extracted text, redirect notice, or error message>",
  "redirect_target": "<target URL or null>",
  "cached_at": "2026-08-12T10:30:00Z"
}
```

- **TTL check:** on read, if `now - cached_at > config.cache_ttl_secs` → delete file → cache miss
- **Write:** after any fetch result (success/truncated/redirect/error), serialize and write to cache dir
- **Create dir:** `std::fs::create_dir_all` on first write, best-effort (non-fatal if it fails — cache degrades gracefully)

### 3. Cross-Host Redirect Detection

- Build `reqwest::Client` with `.redirect(reqwest::redirect::Policy::none())`
- On fetch response:
  - Status 301/302/303/307/308 → check `Location` header
  - Parse redirect target URL → compare host against original URL host
  - **Same host:** follow redirect manually (build new request to target URL)
  - **Different host:** return `<fetch redirect>` with target URL, do NOT follow
  - Max redirect chain: 5 (same-host only)

### 4. Pre-processing Truncation

**Current behavior (kept):** download up to `max_content_length`, run readability, truncate result.

No change. The "pre-processing truncation" gap is left as-is per user decision ("2=保持").

### 5. HTTPS Upgrade

No change. Per user decision ("1=保持").

### 6. Config Changes

No new `FetchProviderConfig` fields. All cache settings are hardcoded defaults:

```rust
const CACHE_TTL_SUCCESS_SECS: u64 = 900;   // 15 min
const CACHE_TTL_ERROR_SECS: u64 = 300;     // 5 min
const MAX_REDIRECT_HOPS: u32 = 5;
const CACHE_DIR: &str = ".vol/cache/tools/web_fetch";
```

Cache dir resolved relative to current working directory. If `.vol` doesn't exist, skip caching (non-fatal).

### 7. Implementation Files

| File | Changes |
|------|---------|
| `crates/vol-llm-tool/src/web/fetch.rs` | Add `FetchStatus` enum, add `status` field to `FetchResult`, add `FetchStatus` to `FetchOptions`/`FetchResult` |
| `crates/vol-llm-tools-builtin/web-fetch/src/lib.rs` | Cache read/write helpers, redirect detection loop, status-aware output formatting |
| `crates/vol-llm-tools-builtin/web-search-tool/src/lib.rs` | `WebFetchTool::execute()` — propagate `FetchStatus` into output text |

### 8. FetchResult Changes

```rust
pub enum FetchStatus {
    Success,
    SuccessTruncated { original_bytes: usize, truncated_bytes: usize },
    Redirect { target_url: String },
    Error { message: String },
}

pub struct FetchResult {
    pub url: String,
    pub status: FetchStatus,
    pub content: String,
    pub title: Option<String>,
}
```

### 9. Output Format

```
<fetch from cache>           ←  only when cache hit; prepended before status tag below
<fetch success>              ←  or <fetch success truncated> / <fetch redirect> / <fetch error>
Title: ...
URL: ...

[content]
```

### 10. Error Handling

- Cache dir creation failure → log warning, continue without caching
- Cache read failure (corrupt file) → delete file, treat as cache miss
- Cache write failure → log warning, return fetch result anyway (cache is best-effort)
- Redirect loop (same-host) → return `<fetch error>` after MAX_REDIRECT_HOPS
- Missing `Location` header → return `<fetch error>`

## Test Plan

- Cache hit returns `<fetch from cache>` prefix
- Cache miss writes to disk after fetch
- Same-host redirect is followed (up to 5 hops)
- Cross-host redirect returns `<fetch redirect>` with target URL
- 404/500 error is cached with `<fetch error>` (TTL 5 min)
- Cache TTL expiry: cached file older than TTL is deleted and treated as miss
- No regression: existing fetch tests still pass
