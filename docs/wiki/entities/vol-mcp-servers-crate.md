---
type: entity
category: product
tags: [crate, mcp, transport, rust, docker]
created: 2026-05-10
updated: 2026-08-28
source_count: 3
---

# vol-mcp-servers Crate

**Category:** Rust crate — MCP server collection
**Related:** [[vol-llm-agent-protocol-crate]], [[rmcp-sdk]], [[mcp-transport-pattern]], [[docs-rs-tools]], [[vol-llm-mcp-crate]], [[mcp-client-integration]]

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
| `cli-tools-mcp` | Serves `.agents/cli-tools/*.toml` declarative CLI tools over MCP | one per enabled config (currently `ansible`, `gh`, `echo-tool`) |

### cli-tools-mcp
Source: [[sandbox-registry-manager-unification]]

Exposes each `.agents/cli-tools/*.toml` config as one MCP tool taking a single `command` string, validated against the config's `binaries` whitelist and executed inside the config's sandbox. See [[cli-style-tool-pattern]].

CLI flags: `--cli-tools-dir` (default `.agents/cli-tools`), `--sandboxes-dir` (default `.agents/sandboxes`), plus the shared transport args.

Startup builds a [[sandbox-lifecycle]] `SandboxManager`, registers Local/Tmp/SSH providers, calls `preload(sandboxes_dir)`, then hands the manager to `CliToolsMcpServer::load()`, which resolves each tool's `sandbox_ref` via `acquire_by_name()`.

Deployed as its own workload (`deploy/argocd/manifests/workloads/mcp/cli-tools-mcp/`) with cli-tool and sandbox ConfigMaps projected in, and the `ansible-ssh-key` Secret mounted at `/app/.ssh`. Runs as root — required for SSH key access.

**Known failure mode:** both the sandbox loader and the cli-tool loader warn-and-skip on failure, so a broken sandbox config yields a healthy server serving zero tools. This happened for real (2026-08-27 → 2026-08-28) via [[schema-drift]] — see [[sandbox-registry-manager-unification]]. When debugging "no tools", check the startup warnings, not the health endpoint.

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
- The `build-mcp-images` GitHub Actions workflow builds `docs-rs-mcp` for `linux/amd64`, pushes a short-SHA tag to ACR, and updates `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/deployment.yaml` for ArgoCD rollout [[argocd-gitops-deployment]]

## GitOps Deployment
Source: [[argocd-gitops-deployment]]

`docs-rs-mcp` is the first MCP service managed by the self-contained ArgoCD tree. It now lives under the `workloads` child Application, which syncs `deploy/argocd/manifests/workloads/mcp/docs-rs-mcp/` into `vol-agent-system`. The workload runs the server with `--http 0.0.0.0:8080`, exposes a ClusterIP service on port 8080, and includes `/health` readiness/liveness probes, proxy environment variables, resource requests/limits, and `acr-registry-secret` for private ACR pulls.
