# MCP Config Multi-Transport Design

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support multiple MCP transport types (stdio, HTTP, HTTP/SSE) via a required `type` field in `.mcp.json`, replacing the current stdio-only parsing.

**Architecture:** Introduce `McpTransport` enum with `Stdio` and `Http` variants. `McpServerConfig` holds the enum instead of flat fields. Parsing branches on the `type` field, not on field presence.

**Tech Stack:** Rust, serde, rmcp (with `transport-streamable-http-client` feature)

---

### Problem

Current `McpServerConfig` only supports stdio transport (requires `command` field). HTTP-type servers (with `url` field) are silently skipped with a warning. The user's `~/.mcp.json` has 3 servers but only 1 connects.

### Design

#### 1. Transport Enum Types

`config.rs` gets a new `McpTransport` enum:

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        #[serde(default)]
        headers: Option<HashMap<String, String>>,
    },
}
```

`McpServerConfig` becomes:

```rust
pub struct McpServerConfig {
    pub name: String,
    pub transport: McpTransport,
}
```

#### 2. Parsing by `type` Field

`type` is **required**. No fallback for missing `type`.

Raw server config deserialization uses a tagged enum approach with serde's `#[serde(tag = "type")]`:

- `"type": "stdio"` → `Stdio` variant, requires `command`, parses `args`/`env`
- `"type": "http"` → `Http` variant, requires `url`, parses optional `headers`

The `transport` field in JSON (e.g. `"transport": "sse"`) is treated as an alias for `type: "http"`.

Servers with missing or unrecognized `type` are skipped with a warning that names the server and the problem.

#### 3. Connection Strategy (manager.rs)

`McpManager::connect_single` matches on `McpTransport`:

- `Stdio` → current `TokioChildProcess` approach (no change)
- `Http` → use `rmcp`'s `StreamableHttpClientTransport` (requires adding `"transport-streamable-http-client"` feature to `rmcp` dependency in `Cargo.toml`)

The `ServerStatus` tracking, retry backoff, and tool/resource caching remain unchanged.

#### 4. Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-mcp/src/config.rs` | Replace `RawServerConfig`/`McpServerConfig` with enum-based types, add `type`-field parsing |
| `crates/vol-llm-mcp/src/manager.rs` | Add `match transport` branch in `connect_single` for HTTP connection |
| `crates/vol-llm-mcp/Cargo.toml` | Add `"transport-streamable-http-client"` feature to `rmcp` dependency |
| `crates/vol-llm-mcp/src/lib.rs` | Export `McpTransport` if needed downstream |

### Task 1: Add transport enum and update config parsing

**Files:**
- Modify: `crates/vol-llm-mcp/src/config.rs`
- Test: `crates/vol-llm-mcp/src/config.rs` (inline tests)

- [ ] Define `McpTransport` enum with `Stdio` and `Http` variants
- [ ] Define `RawMcpTransport` enum with `#[serde(tag = "type")]` for deserialization
- [ ] Update `McpServerConfig` to use `McpTransport` instead of flat fields
- [ ] Update `try_parse_server` and `merge_configs` to use new types
- [ ] Update tests: add test for `type: "stdio"`, `type: "http"`, missing `type` (should skip), and unrecognized type (should skip)
- [ ] Run `cargo test -p vol-llm-mcp` to verify all tests pass

### Task 2: Add HTTP connection support in manager

**Files:**
- Modify: `crates/vol-llm-mcp/src/manager.rs`
- Modify: `crates/vol-llm-mcp/Cargo.toml`

- [ ] Add `"transport-streamable-http-client"` feature to `rmcp` dependency in `Cargo.toml`
- [ ] In `McpManager::connect_single`, match on `config.transport`:
  - `Stdio { command, args, env }` → keep existing `TokioChildProcess` logic
  - `Http { url, headers }` → create `StreamableHttpClientTransport` from `rmcp::transport::streamable_http_client` and use it to connect
- [ ] Ensure error types from HTTP connection are wrapped in `McpError::ConnectionFailed`
- [ ] Run `cargo build -p vol-llm-mcp` to verify compilation
- [ ] Run the full binary example `jsonrpc_agent_service` and verify all 3 servers appear in the MCP panel
