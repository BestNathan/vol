# vol-mcp-servers: docs-rs MCP Server Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `vol-mcp-servers` crate with a `docs-rs-mcp` binary that implements 4 docs.rs/crates.io tools using `rmcp`, supporting stdio (default), HTTP, and SSE transports.

**Architecture:** Single crate with modular source layout: `transport/` handles stdio/http/sse startup, `docs_rs/` contains tool implementations. The binary (`src/bin/docs_rs.rs`) parses CLI args and assembles the two pieces.

**Tech Stack:** Rust 2021, `rmcp 1.6.0` (MCP protocol), `scraper` (HTML parsing), `html2md` (HTML→Markdown), `reqwest` (HTTP client), `tokio` (async runtime), `clap` (CLI parsing).

---

### Task 1: Create crate scaffold with workspace integration

**Files:**
- Create: `crates/vol-mcp-servers/Cargo.toml`
- Create: `crates/vol-mcp-servers/src/lib.rs`
- Modify: `Cargo.toml` (workspace root — add member + workspace dependency)

- [ ] **Step 1: Add workspace member and dependency**

Add `"crates/vol-mcp-servers"` to workspace `members` in `Cargo.toml`:

```toml
# In workspace.members array, add:
"crates/vol-mcp-servers",
```

Add to `[workspace.dependencies]`:

```toml
vol-mcp-servers = { path = "crates/vol-mcp-servers" }
```

- [ ] **Step 2: Create Cargo.toml**

```toml
[package]
name = "vol-mcp-servers"
version.workspace = true
edition.workspace = true

[[bin]]
name = "docs-rs-mcp"
path = "src/bin/docs_rs.rs"

[dependencies]
rmcp = { version = "1.6", features = ["server", "macros", "schemars", "transport-io", "transport-streamable-http-server"] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
reqwest = { workspace = true }
scraper = "0.26"
html2md = "0.2"
clap = { version = "4", features = ["derive"] }
anyhow = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
tower = { version = "0.5", features = ["util"] }
tower-http = { version = "0.5", features = ["cors"] }
axum = { version = "0.7", features = ["query"] }
```

- [ ] **Step 3: Create lib.rs**

```rust
pub mod docs_rs;
pub mod transport;
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-mcp-servers
```

Expected: errors for missing `docs_rs` and `transport` modules — that's expected, we create them next.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml crates/vol-mcp-servers/Cargo.toml crates/vol-mcp-servers/src/lib.rs
git commit -m "feat: add vol-mcp-servers crate scaffold"
```

---

### Task 2: Implement transport module

**Files:**
- Create: `crates/vol-mcp-servers/src/transport/mod.rs`
- Create: `crates/vol-mcp-servers/src/transport/http_sse.rs`

- [ ] **Step 1: Create transport/http_sse.rs**

```rust
use std::net::SocketAddr;

use axum::Router;
use rmcp::transport::streamable_http_server::tower::StreamableHttpService;
use rmcp::ServerHandler;
use tokio_util::sync::CancellationToken;
use tower::ServiceExt as _;

pub async fn serve_http_sse<S>(server: S, addr: SocketAddr, ct: CancellationToken) -> anyhow::Result<()>
where
    S: ServerHandler + 'static,
{
    let service = StreamableHttpService::new_server(server, Default::default())?;
    let app = Router::new().merge(service);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("docs-rs-mcp listening on http://{addr}");
    axum::serve(listener, app)
        .with_graceful_shutdown(async move { ct.cancelled().await })
        .await?;
    Ok(())
}
```

> **Note:** The exact `StreamableHttpService` API (constructor name, how to convert to axum `Router`) may need compiler-guided adjustment. The key pattern is: create service → mount on axum Router → serve. If `StreamableHttpService` directly implements `tower::Service`, use `Router::new().route_service("/", service)` or `.merge()`. If there's an explicit `.into_axum()` or `.into_router()` method, use that. Let the compiler errors guide you.

- [ ] **Step 2: Create transport/mod.rs**

```rust
use std::net::SocketAddr;

use clap::Parser;
use rmcp::transport::stdio;
use rmcp::ServiceExt;
use tokio_util::sync::CancellationToken;

mod http_sse;

#[derive(Parser, Debug)]
pub struct TransportArgs {
    /// Listen address for HTTP/SSE transport (e.g. 0.0.0.0:8080)
    #[arg(long)]
    pub http: Option<SocketAddr>,
}

pub enum TransportMode {
    Stdio,
    HttpSse(SocketAddr),
}

impl TransportArgs {
    pub fn mode(&self) -> TransportMode {
        if let Some(addr) = self.http {
            TransportMode::HttpSse(addr)
        } else {
            TransportMode::Stdio
        }
    }
}

pub async fn run_server<S>(mode: TransportMode, server: S, ct: CancellationToken) -> anyhow::Result<()>
where
    S: rmcp::ServerHandler + Clone + 'static,
{
    match mode {
        TransportMode::Stdio => {
            tracing::info!("docs-rs-mcp running on stdio");
            let service = server.serve(stdio()).await?;
            tokio::select! {
                _ = ct.cancelled() => {}
                _ = service.waiting() => {}
            }
        }
        TransportMode::HttpSse(addr) => {
            http_sse::serve_http_sse(server, addr, ct).await?;
        }
    }
    Ok(())
}
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-mcp-servers
```

Expected: `rmcp::transport::streamable_http_server::tower::StreamableHttpService` method names may differ slightly — adjust based on compile errors. The key API is `StreamableHttpService::new_server` and `.into_axum()`. If `into_axum` doesn't exist, check the actual method name from docs (it may be `into_router` or similar).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-mcp-servers/src/transport/mod.rs crates/vol-mcp-servers/src/transport/http_sse.rs
git commit -m "feat: add transport module with stdio and HTTP/SSE support"
```

---

### Task 3: Implement docs_rs tool module (mod + shared helpers)

**Files:**
- Create: `crates/vol-mcp-servers/src/docs_rs/mod.rs`

- [ ] **Step 1: Create docs_rs/mod.rs with server definition**

```rust
use rmcp::handler::server::wrapper::Parameters;
use rmcp::schemars;
use rmcp::tool;
use rmcp::tool_router;

mod get_item;
mod readme;
mod search_crates;
mod search_in_crate;

use get_item::*;
use readme::*;
use search_crates::*;
use search_in_crate::*;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchCratesParams {
    /// Search keywords for finding relevant crates. Keywords should be in English.
    pub query: String,
    /// Number of results per page (default: 10, max: 100)
    pub per_page: Option<usize>,
    /// Sort order: 'relevance', 'downloads', 'recent-downloads', 'recent-updates', 'new'
    pub sort: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ReadMeParams {
    /// Name of the crate to get README for
    pub crate_name: String,
    /// Specific version (optional, defaults to latest)
    pub version: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct GetItemParams {
    /// Name of the crate
    pub crate_name: String,
    /// Type of item: 'module', 'struct', 'trait', 'enum', 'type', 'fn', etc.
    pub item_type: String,
    /// The full path of the item (e.g. wasmtime::component::Component)
    pub item_path: String,
    /// Specific version (optional, defaults to latest)
    pub version: Option<String>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct SearchInCrateParams {
    /// Name of the crate to search
    pub crate_name: String,
    /// Search keyword (trait name, struct name, function name, etc.)
    pub query: String,
    /// Specific version (optional, defaults to latest)
    pub version: Option<String>,
    /// Filter by item type: struct | trait | fn | enum | union | macro | constant
    pub item_type: Option<String>,
}

/// HTTP client shared across tools.
#[derive(Clone)]
pub struct DocsRsClient {
    client: reqwest::Client,
}

impl DocsRsClient {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for DocsRsClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Fetch a docs.rs page and return the HTML body.
async fn fetch_docs_rs_page(url: &str, client: &reqwest::Client) -> anyhow::Result<String> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        anyhow::bail!("Failed to fetch {}: HTTP {}", url, resp.status());
    }
    Ok(resp.text().await?)
}

/// Extract main documentation content from HTML using scraper selectors.
fn extract_doc_content(html: &str) -> String {
    let document = scraper::Html::parse_document(html);
    // Try #main-content first (item pages)
    let selector = scraper::Selector::parse("#main-content").unwrap();
    if let Some(el) = document.select(&selector).next() {
        return html2md::parse_html(&el.html());
    }
    // Fallback: .docblock (module index pages)
    let selector = scraper::Selector::parse(".docblock").unwrap();
    if let Some(el) = document.select(&selector).next() {
        return html2md::parse_html(&el.html());
    }
    // Last fallback
    "No documentation content found.".to_string()
}

#[derive(Clone)]
pub struct DocsRsServer {
    client: DocsRsClient,
}

impl DocsRsServer {
    pub fn new() -> Self {
        Self {
            client: DocsRsClient::new(),
        }
    }
}

impl Default for DocsRsServer {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router(server_handler)]
impl DocsRsServer {
    #[tool(description = "Search for Rust crates by keywords on crates.io.")]
    async fn docs_rs_search_crates(
        &self,
        Parameters(params): Parameters<SearchCratesParams>,
    ) -> Result<String, String> {
        search_crates::search_crates(&self.client.client, &params).await
    }

    #[tool(description = "Get README/overview content of the specified crate")]
    async fn docs_rs_readme(
        &self,
        Parameters(params): Parameters<ReadMeParams>,
    ) -> Result<String, String> {
        readme::get_readme(&self.client.client, &params).await
    }

    #[tool(description = "Get documentation content of a specific item (module, struct, trait, enum, function, etc.) within a crate")]
    async fn docs_rs_get_item(
        &self,
        Parameters(params): Parameters<GetItemParams>,
    ) -> Result<String, String> {
        get_item::get_item(&self.client.client, &params).await
    }

    #[tool(description = "Search for traits, structs, methods, etc. from the crate's all.html page. To get a module, use docs_rs_get_item instead.")]
    async fn docs_rs_search_in_crate(
        &self,
        Parameters(params): Parameters<SearchInCrateParams>,
    ) -> Result<String, String> {
        search_in_crate::search_in_crate(&self.client.client, &params).await
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-mcp-servers
```

Expected: errors for missing submodules — that's expected.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-mcp-servers/src/docs_rs/mod.rs
git commit -m "feat: add docs_rs server definition with tool router"
```

---

### Task 4: Implement search_crates tool

**Files:**
- Create: `crates/vol-mcp-servers/src/docs_rs/search_crates.rs`

- [ ] **Step 1: Create search_crates.rs**

```rust
use reqwest::Client;
use serde::Deserialize;

use super::SearchCratesParams;

#[derive(Deserialize)]
struct CratesIoResponse {
    crates: Vec<CrateInfo>,
}

#[derive(Deserialize)]
struct CrateInfo {
    name: String,
    description: Option<String>,
    downloads: u64,
    max_version: String,
    documentation: Option<String>,
}

pub async fn search_crates(client: &Client, params: &SearchCratesParams) -> Result<String, String> {
    let per_page = params.per_page.unwrap_or(10).min(100);
    let sort = params.sort.as_deref().unwrap_or("relevance");

    let resp = client
        .get("https://crates.io/api/v1/crates")
        .query(&[
            ("q", &params.query),
            ("per_page", &per_page.to_string()),
            ("sort", sort),
        ])
        .header("Accept", "application/json")
        .header("User-Agent", "vol-mcp-servers/0.1.0")
        .send()
        .await
        .map_err(|e| format!("Failed to search crates: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("crates.io returned {}", resp.status()));
    }

    let body: CratesIoResponse = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse crates.io response: {e}"))?;

    if body.crates.is_empty() {
        return Ok(format!("No crates found for query \"{}\"", params.query));
    }

    let results: Vec<String> = body
        .crates
        .iter()
        .map(|c| {
            format!(
                "## {} ({})\n\n**Description:** {}\n\n**Downloads:** {}\n\n**Documentation:** {}\n\n---",
                c.name,
                c.max_version,
                c.description.as_deref().unwrap_or("No description available"),
                c.downloads,
                c.documentation.as_deref().unwrap_or("N/A"),
            )
        })
        .collect();

    Ok(format!(
        "# Crate Search Results for \"{}\"\n\n{}",
        params.query,
        results.join("\n")
    ))
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-mcp-servers
```

Expected: PASS (no remaining module errors).

- [ ] **Step 3: Commit**

```bash
git add crates/vol-mcp-servers/src/docs_rs/search_crates.rs
git commit -m "feat: implement docs_rs_search_crates tool"
```

---

### Task 5: Implement readme tool

**Files:**
- Create: `crates/vol-mcp-servers/src/docs_rs/readme.rs`

- [ ] **Step 1: Create readme.rs**

```rust
use reqwest::Client;

use super::{extract_doc_content, fetch_docs_rs_page, ReadMeParams};

pub async fn get_readme(client: &Client, params: &ReadMeParams) -> Result<String, String> {
    let version = params.version.as_deref().unwrap_or("latest");
    let url = format!(
        "https://docs.rs/{}/{}/{}/index.html",
        params.crate_name, version, params.crate_name
    );

    let html = fetch_docs_rs_page(&url, client)
        .await
        .map_err(|e| format!("Failed to get README for {}: {e}", params.crate_name))?;

    let content = extract_doc_content(&html);

    Ok(format!(
        "# {} Documentation\n\n**Documentation URL:** {}\n\n{}",
        params.crate_name, url, content
    ))
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-mcp-servers
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-mcp-servers/src/docs_rs/readme.rs
git commit -m "feat: implement docs_rs_readme tool"
```

---

### Task 6: Implement get_item tool

**Files:**
- Create: `crates/vol-mcp-servers/src/docs_rs/get_item.rs`

- [ ] **Step 1: Create get_item.rs**

```rust
use reqwest::Client;

use super::{extract_doc_content, fetch_docs_rs_page, GetItemParams};

pub async fn get_item(client: &Client, params: &GetItemParams) -> Result<String, String> {
    let version = params.version.as_deref().unwrap_or("latest");

    let url = if params.item_type == "module" {
        let path = params.item_path.replace("::", "/");
        format!("https://docs.rs/{}/{}/{}/index.html", params.crate_name, version, path)
    } else {
        let parts: Vec<&str> = params.item_path.split("::").collect();
        let item_name = parts.last().unwrap();
        let module_path = if parts.len() > 1 {
            parts[..parts.len() - 1].join("/")
        } else {
            params.crate_name.clone()
        };
        format!(
            "https://docs.rs/{}/{}/{}/{}.{}.html",
            params.crate_name, version, module_path, params.item_type, item_name
        )
    };

    let html = fetch_docs_rs_page(&url, client)
        .await
        .map_err(|e| format!("Failed to get item documentation for {}: {e}", params.item_path))?;

    let content = extract_doc_content(&html);

    Ok(format!(
        "# {} ({})\n\n**Documentation URL:** {}\n\n{}",
        params.item_path, params.item_type, url, content
    ))
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-mcp-servers
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-mcp-servers/src/docs_rs/get_item.rs
git commit -m "feat: implement docs_rs_get_item tool"
```

---

### Task 7: Implement search_in_crate tool

**Files:**
- Create: `crates/vol-mcp-servers/src/docs_rs/search_in_crate.rs`

- [ ] **Step 1: Create search_in_crate.rs**

```rust
use reqwest::Client;
use scraper::{Html, Selector};

use super::{fetch_docs_rs_page, SearchInCrateParams};

pub async fn search_in_crate(client: &Client, params: &SearchInCrateParams) -> Result<String, String> {
    let version = params.version.as_deref().unwrap_or("latest");
    let url = format!(
        "https://docs.rs/{}/{}/{}/all.html",
        params.crate_name, version, params.crate_name
    );

    let html = fetch_docs_rs_page(&url, client)
        .await
        .map_err(|e| format!("Failed to search items in {}: {e}", params.crate_name))?;

    let document = Html::parse_document(&html);
    let link_selector = Selector::parse("#main-content a").unwrap();

    let mut items: Vec<(String, String, String)> = Vec::new();

    for element in document.select(&link_selector) {
        let name = element.inner_html().trim().to_string();
        if name.is_empty() {
            continue;
        }
        let href = element.value().attr("href").unwrap_or("").to_string();
        if href.is_empty() {
            continue;
        }

        let item_type = classify_item_type(&href);
        if item_type == "unknown" {
            continue;
        }

        let matches_query = params.query.is_empty()
            || name.to_lowercase().contains(&params.query.to_lowercase());
        let matches_type = params.item_type.as_ref().is_none_or(|t| {
            item_type == *t || name.to_lowercase().contains(&t.to_lowercase())
        });

        if matches_query && matches_type {
            let full_link = if href.starts_with("http") {
                href.clone()
            } else {
                format!("https://docs.rs/{}/{}/{}/{}", params.crate_name, version, params.crate_name, href.trim_start_matches('/'))
            };
            items.push((name, item_type, full_link));
        }
    }

    // Deduplicate
    items.sort();
    items.dedup();

    let search_term = if params.query.is_empty() {
        "all items"
    } else {
        &params.query
    };

    if items.is_empty() {
        return Ok(format!(
            "# Search Results for \"{}\" in {}\n\nNo matching items found.",
            search_term, params.crate_name
        ));
    }

    let results: Vec<String> = items
        .iter()
        .map(|(name, itype, link)| {
            format!(
                "## {} ({})\n\n**Link:** [View Documentation]({})\n\n---",
                name, itype, link
            )
        })
        .collect();

    Ok(format!(
        "# Search Results for \"{}\" in {}\n\nFound {} items\n\n{}",
        search_term, params.crate_name, items.len(), results.join("\n")
    ))
}

fn classify_item_type(href: &str) -> &'static str {
    if href.contains("struct.") {
        "struct"
    } else if href.contains("trait.") {
        "trait"
    } else if href.contains("fn.") {
        "function"
    } else if href.contains("enum.") {
        "enum"
    } else if href.contains("type.") {
        "type"
    } else if href.contains("const.") {
        "constant"
    } else if href.contains("static.") {
        "static"
    } else if href.contains("macro.") {
        "macro"
    } else if href.contains("union.") {
        "union"
    } else {
        "unknown"
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-mcp-servers
```

Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-mcp-servers/src/docs_rs/search_in_crate.rs
git commit -m "feat: implement docs_rs_search_in_crate tool"
```

---

### Task 8: Implement binary entry point

**Files:**
- Create: `crates/vol-mcp-servers/src/bin/docs_rs.rs`

- [ ] **Step 1: Create docs_rs.rs**

```rust
use clap::Parser;
use tokio_util::sync::CancellationToken;
use vol_mcp_servers::docs_rs::DocsRsServer;
use vol_mcp_servers::transport::{run_server, TransportArgs};

#[derive(Parser, Debug)]
#[command(name = "docs-rs-mcp", about = "docs.rs MCP server with multi-transport support")]
struct Cli {
    #[command(flatten)]
    transport: TransportArgs,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    let mode = cli.transport.mode();
    let server = DocsRsServer::new();
    let ct = CancellationToken::new();

    run_server(mode, server, ct).await
}
```

- [ ] **Step 2: Full compilation check**

```bash
cargo build -p vol-mcp-servers --bin docs-rs-mcp
```

Expected: PASS with no errors.

- [ ] **Step 3: Test stdio transport startup**

```bash
timeout 3 cargo run --bin docs-rs-mcp 2>&1 || true
```

Expected: logs showing "docs-rs-mcp running on stdio", then process exits on timeout.

- [ ] **Step 4: Test HTTP transport startup**

```bash
timeout 3 cargo run --bin docs-rs-mcp -- --http 127.0.0.1:9999 2>&1 || true
```

Expected: logs showing "docs-rs-mcp listening on http://127.0.0.1:9999", then process exits on timeout.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-mcp-servers/src/bin/docs_rs.rs
git commit -m "feat: add docs-rs-mcp binary with CLI transport selection"
```

---

### Task 9: Integration test — verify tools respond correctly

**Files:**
- No new files (manual verification via MCP client)

- [ ] **Step 1: Test with an MCP client**

Run the server and test via a simple MCP client or the `mcpxy` tool:

```bash
# In one terminal
cargo run --bin docs-rs-mcp

# In another, use a simple stdio MCP test
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}' | cargo run --bin docs-rs-mcp
```

Expected: JSON response with server info and tool capabilities.

- [ ] **Step 2: Test HTTP endpoint**

```bash
# Start server
cargo run --bin docs-rs-mcp -- --http 127.0.0.1:9998 &

# Send initialize
curl -s http://127.0.0.1:9998/ \
  -H "Content-Type: application/json" \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"test","version":"0.1"}}}'

# Kill server
kill %1
```

Expected: JSON response with server info.

- [ ] **Step 3: Commit (any test scripts created)**

---

### Task 10: Final cleanup and commit

- [ ] **Step 1: Run cargo clippy**

```bash
cargo clippy -p vol-mcp-servers -- -D warnings
```

Fix any clippy warnings.

- [ ] **Step 2: Run cargo fmt**

```bash
cargo fmt -p vol-mcp-servers
```

- [ ] **Step 3: Final commit**

```bash
git add crates/vol-mcp-servers/
git commit -m "chore: format and clippy fixes for vol-mcp-servers"
```
