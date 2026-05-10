---
type: concept
category: framework
tags: [mcp, tools, docs-rs, crates-io, html-scraping]
created: 2026-05-10
updated: 2026-05-10
source_count: 1
---

# docs-rs Tools

**Category:** MCP tool set
**Related:** [[vol-mcp-servers-crate]], [[docs-rs-mcp-impl]]

## Definition

Four MCP tools that expose docs.rs and crates.io documentation as structured tool calls for AI assistants. Ported from the `@nuskey8/docs-rs-mcp` TypeScript implementation using `rmcp` in Rust.

## Tools

| Tool | Data Source | Description |
|------|-------------|-------------|
| `docs_rs_search_crates` | crates.io REST API | Search crates by keyword with sort/pagination |
| `docs_rs_readme` | docs.rs HTML | Get crate overview/README from docs.rs index page |
| `docs_rs_get_item` | docs.rs HTML | Get specific item (struct, trait, fn, module) documentation |
| `docs_rs_search_in_crate` | docs.rs all.html | Search within a crate's public API index |

## Implementation Pattern

Each tool follows the same pattern:
1. Build URL from typed params struct
2. Fetch via shared `reqwest::Client`
3. Parse HTML with `scraper` (CSS selectors)
4. Extract content using `html2md` (HTML→Markdown conversion)
5. Return formatted markdown string

## HTML Content Extraction

Two-tier selector strategy:
1. Primary: `#main-content` — used on item documentation pages
2. Fallback: `.docblock` — used on module index pages

## URL Construction (get_item)

- **Module**: `https://docs.rs/{crate}/{version}/{path::with::slashes}/index.html`
- **Other items**: `https://docs.rs/{crate}/{version}/{module/path}/{type}.{name}.html`
  - Split `item_path` by `::`, last segment = item name, rest = module path

## crates.io API Notes

- Requires `User-Agent` header (crates.io rejects requests without it)
- Response field is `newest_version` (not `max_version`)
- Pagination: `per_page` max 100, default 10
