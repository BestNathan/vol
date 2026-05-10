---
type: source
category: implementation
tags: [mcp, docs-rs, rust, vol-mcp-servers]
created: 2026-05-10
updated: 2026-05-10
---

# docs-rs MCP Server Implementation

**Category:** Implementation
**Related:** [[vol-mcp-servers-crate]], [[mcp-transport-pattern]], [[docs-rs-tools]], [[rmcp-sdk]]

## Overview

Created `vol-mcp-servers` crate with `docs-rs-mcp` binary implementing 4 MCP tools for docs.rs/crates.io documentation search. Reference implementation was `@nuskey8/docs-rs-mcp` (TypeScript), ported to Rust using `rmcp` SDK.

## Key Decisions

### Single Crate, Multiple Binaries

Each MCP server is a `[[bin]]` entry in Cargo.toml, not a separate crate. This keeps shared dependencies (transport module, common utilities) in one place.

### rmcp over Custom MCP

Chose `rmcp 1.6.0` over hand-rolling MCP protocol. The `#[tool_router]` + `#[tool]` macro system reduces boilerplate significantly compared to the TypeScript reference which manually registered tool handlers.

### HTTP/SSE via StreamableHttpService

The `StreamableHttpService` API differs from the planned `new_server` constructor. Actual API:

```rust
StreamableHttpService::new(
    move || Ok(server.clone()),
    Arc::new(LocalSessionManager::default()),
    StreamableHttpServerConfig::default()
        .with_cancellation_token(ct.clone()),
)
```

The service is mounted via `Router::new().nest_service("/", service)`.

## File Structure

```
vol-mcp-servers/
├── Cargo.toml          # [[bin]] name = "docs-rs-mcp"
├── src/
│   ├── lib.rs          # pub mod docs_rs; pub mod transport;
│   ├── bin/docs_rs.rs  # CLI entry point
│   ├── transport/
│   │   ├── mod.rs      # TransportArgs, TransportMode, run_server()
│   │   └── http_sse.rs # StreamableHttpService → axum Router
│   └── docs_rs/
│       ├── mod.rs              # DocsRsServer, params structs, shared helpers
│       ├── search_crates.rs    # crates.io API call
│       ├── readme.rs           # docs.rs index page scraping
│       ├── get_item.rs         # docs.rs item page scraping
│       └── search_in_crate.rs  # docs.rs all.html link parsing
```

## Transport Support

- **stdio** (default): `cargo run --bin docs-rs-mcp`
- **HTTP/SSE**: `cargo run --bin docs-rs-mcp -- --http 0.0.0.0:8080`

## Dependencies Added

| Crate | Purpose |
|-------|---------|
| `rmcp 1.6` | MCP protocol, server, macros |
| `scraper 0.26` | HTML parsing with CSS selectors |
| `html2md 0.2` | HTML→Markdown conversion |
| `clap 4` | CLI argument parsing |

All other deps (`reqwest`, `tokio`, `serde`, `axum`, `tower`) from workspace.

## Testing

- Both transports verified: stdio logs "docs-rs-mcp running on stdio", HTTP logs "listening on http://..."
- HTTP endpoint returns valid MCP initialize response with `{"capabilities":{"tools":{}}}`
- `cargo clippy` passes with `-D warnings`
- `cargo fmt` applied
