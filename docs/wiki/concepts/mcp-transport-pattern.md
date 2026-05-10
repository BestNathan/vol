---
type: concept
category: framework
tags: [mcp, transport, http, sse, stdio, rmcp]
created: 2026-05-10
updated: 2026-05-10
source_count: 1
---

# MCP Transport Pattern

**Category:** Network transport
**Related:** [[vol-mcp-servers-crate]], [[rmcp-sdk]], [[docs-rs-tools]], [[vol-llm-agent-channel-crate]]

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

## Comparison with vol-llm-agent-channel Transports

| Aspect | vol-mcp-servers transport | vol-llm-agent-channel transport |
|--------|--------------------------|--------------------------------|
| Protocol | MCP JSON-RPC 2.0 | Custom Message protocol |
| HTTP handling | StreamableHttpService (rmcp native) | Hand-built axum handlers |
| SSE | Built into StreamableHttpService | Manual `broadcast::channel` merge |
| Session mgmt | UUID-based via LocalSessionManager | ConnectionHolder (single connection) |
| Purpose | External API exposure | Agent-to-client communication |
