# vol-mcp-servers Docker Image Specification

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Dockerfile to `vol-mcp-servers` that builds any MCP server binary via `ARG BIN_NAME` and produces a minimal Alpine-based image.

**Architecture:** Two-stage multi-stage Dockerfile. Stage 1 (`builder`) installs Rust toolchain on Alpine, compiles the specified binary. Stage 2 (runtime) copies the stripped binary into a clean Alpine image with only `ca-certificates`.

**Tech Stack:** Docker multi-stage, Alpine 3.21, Alpine native Rust toolchain, `strip` for binary size reduction.

---

## Dockerfile Structure

### Builder Stage

- Base: `alpine:3.21`
- Installs: `rustup cargo gcc libc-dev` (native Alpine build tools)
- Copies workspace Cargo.toml + Cargo.lock for dependency fetch
- Copies only `crates/vol-mcp-servers/` source (not entire workspace)
- Builds: `cargo build --release --bin ${BIN_NAME} -p vol-mcp-servers`
- Strips: `strip target/release/${BIN_NAME}`

### Runtime Stage

- Base: `alpine:3.21`
- Installs: `ca-certificates` only (required for HTTPS to crates.io/docs.rs)
- Copies: single binary from builder
- Exposes: port 8080
- Entrypoint: `/usr/local/bin/${BIN_NAME}`
- Default CMD: `--http 0.0.0.0:8080`

### Build Command

```bash
docker build --build-arg BIN_NAME=docs-rs-mcp -t vol-mcp-servers:docs-rs .
```

### Expected Image Size

~30-50MB (Alpine base ~7MB + ca-certificates ~1MB + binary ~20-30MB stripped)

---

## .dockerignore

Exclude everything except `vol-mcp-servers` crate and workspace root files needed for compilation:

```
*
!Cargo.toml
!Cargo.lock
!crates/vol-mcp-servers/
```
