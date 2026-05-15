---
type: source
source_type: design
date: 2026-05-15
ingested: 2026-05-15
tags: [mcp, transport, config, serde, rmcp]
---

# MCP Multi-Transport Config Design & Implementation

**Authors/Creators:** BestNathan + Claude
**Date:** 2026-05-15
**Link:** `docs/superpowers/specs/2026-05-15-mcp-multi-transport-config-design.md`

## TL;DR
Replaced flat-field `McpServerConfig` with a `McpTransport` enum (`Stdio`/`Http`) that uses a required `type` field in `.mcp.json` for discriminated parsing via serde's `#[serde(tag = "type")]`. HTTP transport connects via rmcp's `StreamableHttpClientTransport` (reqwest-based).

## Key Takeaways
- `type` field is **required** — no backward compatibility fallback; servers without a valid `type` are skipped with a warning
- Serde tagged enum (`#[serde(tag = "type")]`) dispatches deserialization to `RawStdioConfig` or `RawHttpConfig`
- `manager.rs` matches on `McpTransport` to choose between `TokioChildProcess` (stdio) and `StreamableHttpClientTransport` (HTTP)
- HTTP config supports optional `headers` field for auth/custom headers
- `rmcp` dependency gained `transport-streamable-http-client-reqwest` feature

## Detailed Summary

### Config Parsing (`config.rs`)

Before: `RawServerConfig` required `command` field, silently skipping HTTP-type servers.
After: `RawMcpTransport` is a serde internally-tagged enum:

```rust
#[derive(Deserialize)]
#[serde(tag = "type")]
enum RawMcpTransport {
    #[serde(rename = "stdio")]
    Stdio(RawStdioConfig),  // requires: command, optional: args, env
    #[serde(rename = "http")]
    Http(RawHttpConfig),    // requires: url, optional: headers
}
```

`McpServerConfig` changed from flat fields (`command`, `args`, `env`) to `transport: McpTransport`.

### Connection Dispatch (`manager.rs`)

`connect_single` matches on `config.transport`:
- `Stdio` → extracted `connect_stdio` helper (same as before)
- `Http` → new `connect_http` builds `StreamableHttpClientTransportConfig` with URL + optional headers, creates `StreamableHttpClientTransport`, connects via `ClientInfo::serve_with_ct`

Both paths use the same timeout (10s) and error wrapping (`McpError::ConnectionFailed`, `McpError::InitializeTimeout`).

### Cargo Dependencies

Added to `vol-llm-mcp/Cargo.toml`:
- `rmcp`: `transport-streamable-http-client-reqwest` feature
- `reqwest = "0.12"` (json feature)
- `http = "1"`

## Entities Mentioned
- [[vol-llm-mcp-crate]]: primary entity — config.rs, manager.rs, Cargo.toml all modified

## Concepts Covered
- [[mcp-transport-pattern]]: multi-transport startup pattern now implemented for stdio + HTTP
- [[mcp-manager-lifecycle]]: McpManager now dispatches on transport type for connection

## Notes
- `session.rs` also updated to pattern-match on the new enum (compilation fix), but remains stdio-only for now
- `type` is required per user request — no backward compatibility for configs without it
