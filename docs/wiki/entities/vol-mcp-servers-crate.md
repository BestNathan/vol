---
type: entity
category: product
tags: [crate, mcp, transport, rust, docker]
created: 2026-05-10
updated: 2026-05-10
source_count: 1
---

# vol-mcp-servers Crate

**Category:** Rust crate — MCP server collection
**Related:** [[vol-llm-agent-channel-crate]], [[rmcp-sdk]], [[mcp-transport-pattern]], [[docs-rs-tools]], [[vol-llm-mcp-crate]], [[mcp-client-integration]]

## Overview

The `vol-mcp-servers` crate provides standalone MCP (Model Context Protocol) server binaries using the `rmcp` Rust SDK. Each server is an independent binary with multi-transport support (stdio, HTTP/SSE), designed to expose external APIs and documentation as MCP tools for AI assistants.

## Key Facts
- Each MCP server is a separate binary via `[[bin]]` entries in Cargo.toml
- All servers share a unified `transport/` module for stdio and HTTP/SSE startup
- CLI uses `clap` derive: `--http <addr>` flag switches from stdio to HTTP/SSE transport
- `rmcp 1.6.0` provides the MCP protocol layer with `#[tool_router(server_handler)]` and `#[tool]` macros
- HTTP/SSE transport uses `StreamableHttpService` from rmcp with `LocalSessionManager` for session management

## Current Servers

| Binary | Description | Tools |
|--------|-------------|-------|
| `docs-rs-mcp` | docs.rs/crates.io documentation search | 4 (search_crates, readme, get_item, search_in_crate) |

## Transport Architecture

```
CLI (--http / default stdio) → transport::run_server()
    ├── Stdio: rmcp::transport::stdio() → server.serve(stdio()).await
    └── HttpSse: StreamableHttpService → axum Router → TCP listener
```

## Timeline
- **2026-05-10**: Crate created with docs-rs-mcp server supporting stdio and HTTP/SSE transports [[docs-rs-mcp-impl]]
- **2026-05-10**: Docker packaging added — single-stage Ubuntu image with ARG-based binary selection [[vol-mcp-servers-dockerfile]]

## Docker Packaging

- Multi-stage Alpine 3.21 Dockerfile packages any binary via `--build-arg BIN_NAME=<name>`
- Builder stage: Rust toolchain + rsproxy mirror (via `.cargo/config.toml`)
- Runtime stage: ~30MB Alpine image
- Registry: `crpi-ck06yio90i1ttwlz.cn-beijing.personal.cr.aliyuncs.com/n_common/vol-monitor:<name>`
- Alibaba mirrors: apk → `mirrors.aliyun.com`, crates.io → `rsproxy.cn`
