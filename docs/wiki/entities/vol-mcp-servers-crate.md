---
type: entity
category: product
tags: [crate, mcp, transport, rust, docker]
created: 2026-05-10
updated: 2026-06-16
source_count: 2
---

# vol-mcp-servers Crate

**Category:** Rust crate — MCP server collection
**Related:** [[vol-llm-agent-channel-crate]], [[rmcp-sdk]], [[mcp-transport-pattern]], [[docs-rs-tools]], [[vol-llm-mcp-crate]], [[mcp-client-integration]]

## Overview

The `vol-mcp-servers` crate provides standalone MCP (Model Context Protocol) server binaries using the `rmcp` Rust SDK. Each server is an independent binary with multi-transport support (stdio, HTTP/SSE), designed to expose external APIs and documentation as MCP tools for AI assistants.

## Key Facts
- Each MCP server is a separate binary via Cargo.toml `bin` section entries
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

- Multi-stage Alpine 3.21 Dockerfile packages any binary via `--build-arg BIN_NAME=<name>` [[vol-mcp-servers-dockerfile]]
- The GitOps path adds `dockers/vol-mcp-servers.Dockerfile`, a Debian slim multi-stage build with `--build-arg BIN=docs-rs-mcp` and `REGION=cn|global` for region-aware Rust/Debian mirrors [[argocd-gitops-deployment]]
- Builder stage compiles `cargo build --release -p vol-mcp-servers --bin "${BIN}"` and strips the binary
- Runtime stage installs the selected binary as `/usr/local/bin/mcp-server` and exposes port 8080 for HTTP transport
- The `build-mcp-images` GitHub Actions workflow builds `docs-rs-mcp` for `linux/amd64`, pushes a short-SHA tag to ACR, and updates `deploy/argocd/manifests/mcp/docs-rs-mcp/deployment.yaml` for ArgoCD rollout [[argocd-gitops-deployment]]

## GitOps Deployment
Source: [[argocd-gitops-deployment]]

`docs-rs-mcp` is the first MCP service managed by the self-contained ArgoCD tree. Its child Application syncs `deploy/argocd/manifests/mcp/docs-rs-mcp/` into `vol-agent-system`, runs the server with `--http 0.0.0.0:8080`, exposes a ClusterIP service on port 8080, and includes `/health` readiness/liveness probes, proxy environment variables, resource requests/limits, and `acr-registry-secret` for private ACR pulls.
