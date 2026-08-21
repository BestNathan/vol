---
type: concept
category: framework
tags: [mcp, transport, http, sse, stdio, rmcp]
created: 2026-05-10
updated: 2026-08-13
source_count: 3
---

# MCP Transport Pattern

**Category:** Network transport
**Related:** [[vol-mcp-servers-crate]], [[rmcp-sdk]], [[docs-rs-tools]], [[vol-llm-agent-protocol-crate]], [[vol-llm-mcp-crate]], [[playwright-mcp-service]]

## Definition

Multi-transport startup pattern for MCP servers: all servers share a common `transport/` module that supports stdio (default) and HTTP/SSE via CLI flags.

## Key Points
- `TransportArgs` struct with `#[arg(long)] pub http: Option<SocketAddr>` — flat CLI, no subcommands
- `TransportMode` enum: `Stdio` or `HttpSse(SocketAddr)`
- `run_server()` generic over `ServerHandler + Clone + 'static` — any MCP server can use it
- Stdio mode uses `rmcp::transport::stdio()` with `server.serve(stdio()).await`
- HTTP/SSE mode uses `StreamableHttpService` with `LocalSessionManager` for session tracking

## HTTP/SSE Architecture

```
StreamableHttpService::new(
    move || Ok(server.clone()),  // Server factory
    Arc::new(LocalSessionManager::default()),  // Session manager
    StreamableHttpServerConfig::default()
        .with_cancellation_token(ct.clone()),  // Cancellation
)
→ Router::new().nest_service("/", service)
→ axum::serve(listener, app)
```

- Sessions are stateful by default — each initialize request creates a new session with a UUID
- Client receives `Mcp-Session-Id` header and must include it on subsequent requests
- Graceful shutdown via `CancellationToken`

## Client-Side Transport Config

The client-side (`vol-llm-mcp`) matches the server-side transport types via a required `type` field in `.mcp.json`:

| `type` value | Transport | rmcp implementation |
|--------------|-----------|---------------------|
| `"stdio"` | Child process stdin/stdout | `TokioChildProcess` |
| `"http"` | HTTP POST + SSE stream | `StreamableHttpClientTransport` (reqwest) |

Parsing uses serde's internally-tagged enum: `#[serde(tag = "type")]` dispatches to `RawStdioConfig` or `RawHttpConfig`. Missing or unrecognized `type` values are skipped with a warning — no backward compatibility for configs without `type`.

HTTP config supports optional `headers` field (e.g. `{"Authorization": "Bearer token"}`) for auth.

## In-Cluster Deployment Pattern for Third-Party MCP Servers

Operational lesson from deploying playwright-mcp (2026-08-13): third-party MCP servers (docs-rs-mcp, cli-tools-mcp, playwright-mcp) run in-cluster as a **standalone Deployment + ClusterIP Service**, referenced from the shared `.mcp.json` / mcp-config via an `"type": "http"` URL:

```json
"playwright": {
  "type": "http",
  "url": "http://playwright-mcp.vol-agent-system.svc.cluster.local:8931/mcp"
}
```

- The agent-server pods never execute the server — they connect over the service DNS with the rmcp `StreamableHttpClientTransport` (streamable HTTP).
- mcp-config is generated from repo-root `.mcp.json` by `scripts/sync-configmaps.py`; ConfigMap updates are NOT hot-reloaded — agent-server deployments need a rollout restart.
- Host-allowlist pitfall: playwright-core's streamable HTTP server bound to `0.0.0.0` normalizes to `localhost` and 403s other Host headers — in-cluster deployments may need `--allowed-hosts *` (playwright-mcp case) or equivalent.

## Stdio Pitfall: the Runtime Image Must Contain the Command

A stdio MCP entry (`"type": "stdio"`, e.g. `npx @playwright/mcp --headless`) requires the *agent's* runtime image to contain the command and its runtime (node/npx). The agent-server image is Rust-only (Debian slim, read-only root filesystem) — it cannot run any `npx`-based server, so stdio entries fail at spawn (`MCP server binary not found` / `No such file or directory`) and surface as connection errors on every agent session. For in-cluster agents, prefer the standalone-service pattern above instead of stdio.

## Comparison with vol-llm-agent-channel Transports

| Aspect | vol-mcp-servers transport | vol-llm-agent-channel transport |
|--------|--------------------------|--------------------------------|
| Protocol | MCP JSON-RPC 2.0 | Custom Message protocol |
| HTTP handling | StreamableHttpService (rmcp native) | Hand-built axum handlers |
| SSE | Built into StreamableHttpService | Manual `broadcast::channel` merge |
| Session mgmt | UUID-based via LocalSessionManager | ConnectionHolder (single connection) |
| Purpose | External API exposure | Agent-to-client communication |
