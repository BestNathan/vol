# WebSearch & WebFetch Tools Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add `web_search` and `web_fetch` tools to the agent system, with abstract provider interfaces in `vol-llm-tool` and concrete implementations (Tavily for search, default HTTP→markdown for fetch) in `vol-llm-tools-builtin`.

**Architecture:** Provider trait pattern — `SearchFn`/`FetchFn` traits defined in `vol-llm-tool`, concrete providers implemented as sub-crates under `vol-llm-tools-builtin/`. Tools accept provider instances via constructor.

**Tech Stack:** Rust, reqwest 0.11 (workspace), serde/serde_json, async-trait, tokio

---

## Context

The project has 6 built-in tools (read, write, edit, glob, grep, bash) each as separate sub-crates under `crates/vol-llm-tools-builtin/`. The `vol-llm-tool` crate provides the `Tool`/`ExecutableTool` traits and `ToolRegistry`. This plan follows the same pattern.

- reqwest workspace dep: `version = "0.11"` with `rustls-tls`
- No existing proxy usage in any reqwest client
- Tavily API: POST `https://api.tavily.com/search` with JSON body `{ "query", "api_key" }`

---

## File Structure

```
crates/vol-llm-tool/src/web/
  mod.rs           — web module exports (SearchFn, FetchFn traits + all types)
  search.rs        — SearchFn trait, SearchOptions, SearchResult, SearchError
  fetch.rs         — FetchFn trait, FetchOptions, FetchResult, FetchError

crates/vol-llm-tools-builtin/web-search-tool/
  Cargo.toml
  src/lib.rs       — WebSearchTool + WebFetchTool (ExecutableTool impls)
  src/tavily.rs    — TavilySearchProvider: SearchFn implementation

crates/vol-llm-tools-builtin/web-fetch/
  Cargo.toml
  src/lib.rs       — DefaultFetchProvider: FetchFn implementation (HTTP→markdown)

crates/vol-llm-tools-builtin/src/lib.rs — add web_search, web_fetch re-exports + register_web_all()
```

---

## Implementation Steps

### Task 1: Define SearchFn trait and types in vol-llm-tool

- [ ] **Step 1: Create `crates/vol-llm-tool/src/web/search.rs`**

```rust
//! Web search provider trait.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Options for a web search request
#[derive(Debug, Clone, Default)]
pub struct SearchOptions {
    pub proxy_url: Option<String>,
    pub num_results: Option<usize>,
    pub allowed_domains: Option<Vec<String>>,
    pub blocked_domains: Option<Vec<String>>,
}

/// A single search result item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchItem {
    pub title: String,
    pub url: String,
    pub snippet: Option<String>,
}

/// Search result containing query and items
#[derive(Debug, Clone)]
pub struct SearchResult {
    pub query: String,
    pub results: Vec<SearchItem>,
}

/// Search error type
#[derive(Debug, thiserror::Error)]
pub enum SearchError {
    #[error("Search request failed: {0}")]
    RequestFailed(String),
    #[error("Failed to parse search response: {0}")]
    ParseError(String),
    #[error("Invalid API key")]
    InvalidApiKey,
}

/// Trait for search providers (Tavily, Google, etc.)
#[async_trait]
pub trait SearchFn: Send + Sync {
    /// Search the web for the given query
    async fn search(&self, query: &str, opts: SearchOptions) -> Result<SearchResult, SearchError>;
}
```

- [ ] **Step 2: Create `crates/vol-llm-tool/src/web/fetch.rs`**

```rust
//! Web fetch provider trait.

use async_trait::async_trait;
use std::collections::HashMap;

/// Options for a web fetch request
#[derive(Debug, Clone, Default)]
pub struct FetchOptions {
    pub proxy_url: Option<String>,
    pub prompt: Option<String>,
    pub max_length: Option<usize>,
}

/// Fetch result containing extracted content
#[derive(Debug, Clone)]
pub struct FetchResult {
    pub url: String,
    pub content: String,
    pub title: Option<String>,
}

/// Fetch error type
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("Request failed: {0}")]
    RequestFailed(String),
    #[error("URL not accessible: {0}")]
    NotAccessible(String),
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),
    #[error("Response too large (max {max} bytes, got {actual} bytes)")]
    TooLarge { max: usize, actual: usize },
}

/// Trait for fetch providers (default, readability, etc.)
#[async_trait]
pub trait FetchFn: Send + Sync {
    /// Fetch and extract content from a URL
    async fn fetch(&self, url: &str, opts: FetchOptions) -> Result<FetchResult, FetchError>;
}
```

- [ ] **Step 3: Create `crates/vol-llm-tool/src/web/mod.rs`**

```rust
//! Web tools: abstract provider traits for search and fetch.

pub mod fetch;
pub mod search;

pub use fetch::{FetchError, FetchFn, FetchOptions, FetchResult};
pub use search::{SearchError, SearchFn, SearchItem, SearchResult, SearchOptions};
```

- [ ] **Step 4: Update `crates/vol-llm-tool/src/lib.rs`**

Add `pub mod web;` to the existing module list and add the necessary dependency.

- [ ] **Step 5: Update `crates/vol-llm-tool/Cargo.toml`**

Add dependencies:
```toml
reqwest = { workspace = true }
async-trait = { workspace = true }
thiserror = { workspace = true }
```

---

### Task 2: Implement Tavily search provider + WebSearch/WebFetch tools

- [ ] **Step 1: Create `crates/vol-llm-tools-builtin/web-search-tool/Cargo.toml`**

```toml
[package]
name = "vol-llm-tools-builtin-web-search"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tokio = { workspace = true }
reqwest = { workspace = true }
```

- [ ] **Step 2: Create `crates/vol-llm-tools-builtin/web-search-tool/src/tavily.rs`**

```rust
//! Tavily API search provider.

use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use vol_llm_tool::web::search::{SearchError, SearchFn, SearchItem, SearchOptions, SearchResult};

/// Tavily API search response
#[derive(Debug, Deserialize)]
struct TavilyResponse {
    results: Vec<TavilyResult>,
}

#[derive(Debug, Deserialize)]
struct TavilyResult {
    title: String,
    url: String,
    content: Option<String>,
}

/// Provider that searches using the Tavily API
pub struct TavilySearchProvider {
    api_key: String,
    client: Client,
}

impl TavilySearchProvider {
    pub fn new(api_key: String, proxy_url: Option<String>) -> Result<Self, SearchError> {
        let client = build_client(&proxy_url).map_err(|e| {
            SearchError::RequestFailed(format!("Failed to create HTTP client: {}", e))
        })?;

        Ok(Self { api_key, client })
    }

    /// Build reqwest::Client with optional proxy
    fn build_client(proxy_url: &Option<String>) -> Result<Client, Box<dyn std::error::Error + Send + Sync>> {
        let mut builder = Client::builder();
        if let Some(url) = proxy_url {
            let proxy = reqwest::Proxy::all(url)?;
            builder = builder.proxy(proxy);
        }
        Ok(builder.build()?)
    }
}

#[async_trait]
impl SearchFn for TavilySearchProvider {
    async fn search(&self, query: &str, opts: SearchOptions) -> Result<SearchResult, SearchError> {
        let num_results = opts.num_results.unwrap_or(5);
        let mut body = serde_json::json!({
            "query": query,
            "api_key": self.api_key,
            "max_results": num_results,
        });

        if let Some(ref domains) = opts.allowed_domains {
            body["include_domains"] = serde_json::Value::Array(
                domains.iter().map(|d| serde_json::Value::String(d.clone())).collect()
            );
        }
        if let Some(ref domains) = opts.blocked_domains {
            body["exclude_domains"] = serde_json::Value::Array(
                domains.iter().map(|d| serde_json::Value::String(d.clone())).collect()
            );
        }

        let response = self.client
            .post("https://api.tavily.com/search")
            .json(&body)
            .send()
            .await
            .map_err(|e| SearchError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await.unwrap_or_default();
            if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                return Err(SearchError::InvalidApiKey);
            }
            return Err(SearchError::RequestFailed(
                format!("Tavily API error: {} — {}", status, text)
            ));
        }

        let resp: TavilyResponse = response
            .json()
            .await
            .map_err(|e| SearchError::ParseError(e.to_string()))?;

        let results = resp
            .results
            .into_iter()
            .map(|r| SearchItem {
                title: r.title,
                url: r.url,
                snippet: r.content,
            })
            .collect();

        Ok(SearchResult {
            query: query.to_string(),
            results,
        })
    }
}
```

- [ ] **Step 3: Create `crates/vol-llm-tools-builtin/web-search-tool/src/lib.rs`**

```rust
//! vol-llm-tools-builtin-web-search: Web search and fetch tools.

pub mod tavily;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType};
use vol_llm_tool::web::search::SearchFn;
use vol_llm_tool::web::fetch::FetchFn;
use std::sync::Arc;

// ==================== WebSearchTool ====================

/// Parameters for web_search tool
#[derive(Debug, Deserialize)]
pub struct WebSearchParams {
    pub query: String,
    pub num_results: Option<usize>,
    pub allowed_domains: Option<Vec<String>>,
    pub blocked_domains: Option<Vec<String>>,
}

/// Web search tool — searches the web via a SearchFn provider
pub struct WebSearchTool {
    provider: Arc<dyn SearchFn>,
}

impl WebSearchTool {
    pub fn new(provider: impl SearchFn + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }
}

#[async_trait]
impl ExecutableTool for WebSearchTool {
    fn name(&self) -> &'static str {
        "web_search"
    }

    fn description(&self) -> &'static str {
        "Search the web for up-to-date information. Returns search results with titles, URLs, and snippets. Use for accessing current events, documentation, or recent data."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query"
                },
                "num_results": {
                    "type": "integer",
                    "description": "Number of results to return (default: 5)",
                    "default": 5
                },
                "allowed_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Only include results from these domains"
                },
                "blocked_domains": {
                    "type": "array",
                    "items": {"type": "string"},
                    "description": "Exclude results from these domains"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: WebSearchParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let opts = vol_llm_tool::web::search::SearchOptions {
            num_results: params.num_results,
            allowed_domains: params.allowed_domains,
            blocked_domains: params.blocked_domains,
            proxy_url: None, // Proxy configured at provider creation time
        };

        let result = self.provider.search(&params.query, opts).await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        // Format results as readable text
        let mut content = String::new();
        content.push_str(&format!("Search results for: {}\n\n", result.query));
        for (i, item) in result.results.iter().enumerate() {
            content.push_str(&format!(
                "[{}] {}\n{}\n{}\n\n",
                i + 1,
                item.title,
                item.url,
                item.snippet.as_deref().unwrap_or("")
            ));
        }

        Ok(ToolResult::success(content))
    }
}

// ==================== WebFetchTool ====================

/// Parameters for web_fetch tool
#[derive(Debug, Deserialize)]
pub struct WebFetchParams {
    pub url: String,
    pub prompt: Option<String>,
}

/// Web fetch tool — extracts content from a URL via a FetchFn provider
pub struct WebFetchTool {
    provider: Arc<dyn FetchFn>,
}

impl WebFetchTool {
    pub fn new(provider: impl FetchFn + 'static) -> Self {
        Self {
            provider: Arc::new(provider),
        }
    }
}

#[async_trait]
impl ExecutableTool for WebFetchTool {
    fn name(&self) -> &'static str {
        "web_fetch"
    }

    fn description(&self) -> &'static str {
        "Fetch and extract content from a URL. Converts HTML to readable markdown. Use for reading documentation, articles, or web content."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL to fetch content from"
                },
                "prompt": {
                    "type": "string",
                    "description": "Optional prompt to filter/extract specific content"
                }
            },
            "required": ["url"]
        })
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: WebFetchParams = serde_json::from_value(args.clone()).map_err(|e| {
            ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e))
        })?;

        let opts = vol_llm_tool::web::fetch::FetchOptions {
            prompt: params.prompt,
            proxy_url: None, // Proxy configured at provider creation time
            max_length: None,
        };

        let result = self.provider.fetch(&params.url, opts).await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let mut content = String::new();
        if let Some(title) = &result.title {
            content.push_str(&format!("Title: {}\n\n", title));
        }
        content.push_str(&format!("URL: {}\n\n", result.url));
        content.push_str(&result.content);

        Ok(ToolResult::success(content))
    }
}
```

---

### Task 3: Implement DefaultFetchProvider in web-fetch

- [ ] **Step 1: Create `crates/vol-llm-tools-builtin/web-fetch/Cargo.toml`**

```toml
[package]
name = "vol-llm-tools-builtin-web-fetch"
version.workspace = true
edition.workspace = true

[dependencies]
vol-llm-tool = { workspace = true }
vol-llm-core = { workspace = true }
async-trait = { workspace = true }
reqwest = { workspace = true }
thiserror = { workspace = true }
readability = "0.3"
url = "2.5"
```

- [ ] **Step 2: Create `crates/vol-llm-tools-builtin/web-fetch/src/lib.rs`**

```rust
//! vol-llm-tools-builtin-web-fetch: Default HTTP→markdown fetch provider.

use async_trait::async_trait;
use readability::extractor;
use reqwest::Client;
use std::io::Cursor;
use vol_llm_tool::web::fetch::{FetchError, FetchFn, FetchOptions, FetchResult};

const MAX_CONTENT_LENGTH: usize = 2 * 1024 * 1024; // 2MB
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default fetch provider that fetches URLs and extracts readable content
pub struct DefaultFetchProvider {
    client: Client,
}

impl DefaultFetchProvider {
    pub fn new(proxy_url: Option<String>) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS))
            .user_agent("Mozilla/5.0 (compatible; Agent/1.0)");

        if let Some(url) = proxy_url {
            let proxy = reqwest::Proxy::all(url)?;
            builder = builder.proxy(proxy);
        }

        let client = builder.build()?;
        Ok(Self { client })
    }
}

#[async_trait]
impl FetchFn for DefaultFetchProvider {
    async fn fetch(&self, url: &str, opts: FetchOptions) -> Result<FetchResult, FetchError> {
        // Validate URL
        let parsed = url::Url::parse(url)
            .map_err(|e| FetchError::InvalidUrl(e.to_string()))?;

        // Fetch URL
        let response = self.client
            .get(url)
            .send()
            .await
            .map_err(|e| FetchError::RequestFailed(e.to_string()))?;

        if !response.status().is_success() {
            return Err(FetchError::NotAccessible(format!(
                "HTTP {}", response.status()
            )));
        }

        // Check content length
        if let Some(len) = response.content_length() {
            if len > MAX_CONTENT_LENGTH as u64 {
                return Err(FetchError::TooLarge {
                    max: MAX_CONTENT_LENGTH,
                    actual: len as usize,
                });
            }
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|e| FetchError::RequestFailed(e.to_string()))?;

        if bytes.len() > MAX_CONTENT_LENGTH {
            return Err(FetchError::TooLarge {
                max: MAX_CONTENT_LENGTH,
                actual: bytes.len(),
            });
        }

        // Extract readable content using readability
        let dom = readability::Document::from(Cursor::new(&bytes), Some(parsed));
        let content = dom
            .text()
            .map_err(|e| FetchError::NotAccessible(format!(
                "Failed to extract content: {}", e
            )))?;

        // Apply prompt-based filtering if provided (just truncate to max_length)
        let max_length = opts.max_length.unwrap_or(MAX_CONTENT_LENGTH / 2);
        let content = if content.len() > max_length {
            format!("{}...\n\n[Content truncated at {} characters]",
                &content[..max_length], max_length)
        } else {
            content
        };

        let title = dom.title().ok();

        Ok(FetchResult {
            url: url.to_string(),
            content,
            title,
        })
    }
}
```

---

### Task 4: Register tools in vol-llm-tools-builtin

- [ ] **Step 1: Update `crates/vol-llm-tools-builtin/Cargo.toml`**

Add new dependencies:
```toml
vol-llm-tools-builtin-web-search = { path = "web-search-tool" }
vol-llm-tools-builtin-web-fetch = { path = "web-fetch" }
```

- [ ] **Step 2: Update `crates/vol-llm-tools-builtin/src/lib.rs`**

Add new module re-exports and a `register_web_all()` function:

```rust
pub mod web_search_tool {
    pub use vol_llm_tools_builtin_web_search::*;
}

pub mod web_fetch {
    pub use vol_llm_tools_builtin_web_fetch::*;
}

pub use web_search_tool::{WebSearchTool, WebFetchTool};
pub use web_search_tool::tavily::TavilySearchProvider;
pub use web_fetch::DefaultFetchProvider;

/// Register web tools to a ToolRegistry
/// Provider instances are created at call site so caller can configure API keys and proxy
pub fn register_web_all(
    registry: &mut vol_llm_tool::ToolRegistry,
    tavily_api_key: &str,
    proxy_url: Option<String>,
) {
    let tavily = TavilySearchProvider::new(tavily_api_key.to_string(), proxy_url.clone())
        .expect("Failed to create Tavily provider");
    registry.register(WebSearchTool::new(tavily));

    let fetcher = DefaultFetchProvider::new(proxy_url)
        .expect("Failed to create fetch provider");
    registry.register(WebFetchTool::new(fetcher));
}
```

- [ ] **Step 3: Run `cargo check --workspace`**

Expected: Compiles successfully

- [ ] **Step 4: Run `cargo test --workspace`**

Expected: All existing tests pass + new web tool tests

---

## Design Decisions

### Why provider instances at construction time (not per-call)?

Proxy URL is a stable infrastructure setting — it doesn't change per tool invocation. Creating the HTTP client once at provider construction is more efficient than building it per call. If per-call proxy is needed later, it can be added to `SearchOptions`/`FetchOptions`.

### Why `Arc<dyn SearchFn>` in tool struct?

The tool is cloned by the registry or shared across threads. `Arc` avoids `Clone` bounds on the provider trait and allows cheap sharing. This matches the pattern used for other tools with external service dependencies.

### Why readability crate for HTML→markdown?

The workspace already has HTML processing needs. `readability` is lightweight, extracts readable text from HTML, and avoids the heavy `readability` + `html2md` dependency chain. An alternative is `html2md` but `readability` extracts the main content better (removes nav, ads, etc.).

---

## Potential Issues

| Issue | Mitigation |
|-------|------------|
| Tavily API requires API key | `register_web_all()` takes key as parameter — caller provides from env/config |
| `readability` crate version | Pin to 0.3, test with real HTML first |
| reqwest proxy config | Use `reqwest::Proxy::all(url)` which handles HTTP/HTTPS/SOCKS |
| WebFetch with non-HTML content | readability returns empty for non-HTML; add content-type check |

---

## Verification

After implementation:

```bash
# Compile check
cargo check --workspace

# Tests
cargo test --workspace

# Build release
cargo build --release
```
