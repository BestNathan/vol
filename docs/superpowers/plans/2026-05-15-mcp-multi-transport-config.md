# MCP Multi-Transport Config Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Support multiple MCP transport types (stdio, HTTP) via a required `type` field in `.mcp.json`, replacing the current stdio-only parsing.

**Architecture:** Introduce `McpTransport` enum with `Stdio` and `Http` variants. `McpServerConfig` holds the enum instead of flat fields. Parsing branches on the `type` field using serde's internally tagged enum (`#[serde(tag = "type")]`). `manager.rs` matches on the enum to choose between `TokioChildProcess` (stdio) and `StreamableHttpClientTransport` (HTTP).

**Tech Stack:** Rust, serde, rmcp (with `transport-streamable-http-client-reqwest` feature), tokio

**Design Spec:** `docs/superpowers/specs/2026-05-15-mcp-multi-transport-config-design.md`

---

### File Responsibility Map

| File | Responsibility |
|------|---------------|
| `crates/vol-llm-mcp/src/config.rs` | Transport enum types, raw deserialization, `try_parse_server`, `merge_configs`, tests |
| `crates/vol-llm-mcp/src/manager.rs` | `connect_single` transport dispatch (stdio vs HTTP branching) |
| `crates/vol-llm-mcp/Cargo.toml` | Add `transport-streamable-http-client-reqwest` feature to `rmcp` |
| `crates/vol-llm-mcp/src/lib.rs` | No changes needed (exports `McpConfig`, `McpManager`, `McpError` — `McpTransport` stays internal for now) |

---

### Task 1: Add transport enum and update config parsing

**Files:**
- Modify: `crates/vol-llm-mcp/src/config.rs`
- Test: `crates/vol-llm-mcp/src/config.rs` (inline tests)

- [ ] **Step 1: Write tests for new transport types**

```rust
#[test]
fn test_parse_stdio_type() {
    let value: serde_json::Value = serde_json::from_str(
        r#"{"type":"stdio","command":"echo","args":["hello"]}"#
    ).unwrap();
    let parsed = try_parse_server("test-srv", &value).unwrap();
    assert!(matches!(parsed.transport, McpTransport::Stdio { .. }));
    assert_eq!(parsed.name, "test-srv");
}

#[test]
fn test_parse_http_type() {
    let value: serde_json::Value = serde_json::from_str(
        r#"{"type":"http","url":"http://localhost:3000/mcp"}"#
    ).unwrap();
    let parsed = try_parse_server("http-srv", &value).unwrap();
    assert!(matches!(parsed.transport, McpTransport::Http { .. }));
    assert_eq!(parsed.name, "http-srv");
}

#[test]
fn test_parse_http_with_headers() {
    let value: serde_json::Value = serde_json::from_str(
        r#"{"type":"http","url":"http://localhost:3000/mcp","headers":{"Authorization":"Bearer token"}}"#
    ).unwrap();
    let parsed = try_parse_server("auth-srv", &value).unwrap();
    match &parsed.transport {
        McpTransport::Http { headers, .. } => {
            assert!(headers.is_some());
            let h = headers.as_ref().unwrap();
            assert_eq!(h.get("Authorization").unwrap(), "Bearer token");
        }
        _ => panic!("expected Http transport"),
    }
}

#[test]
fn test_missing_type_is_skipped() {
    let value: serde_json::Value = serde_json::from_str(
        r#"{"command":"echo"}"#
    ).unwrap();
    let result = try_parse_server("no-type-srv", &value);
    assert!(result.is_none());
}

#[test]
fn test_unrecognized_type_is_skipped() {
    let value: serde_json::Value = serde_json::from_str(
        r#"{"type":"websocket","url":"ws://localhost:3000"}"#
    ).unwrap();
    let result = try_parse_server("ws-srv", &value);
    assert!(result.is_none());
}

#[test]
fn test_merge_with_mixed_transports() {
    let user: RawMcpConfig = serde_json::from_str(
        r#"{"mcpServers":{
            "stdio-srv":{"type":"stdio","command":"echo","args":["hi"]},
            "http-srv":{"type":"http","url":"http://localhost:3000"}
        }}"#
    ).unwrap();
    let merged = merge_configs(None, Some(user));
    assert_eq!(merged.servers.len(), 2);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd /root/nq-deribit && cargo test -p vol-llm-mcp 2>&1 | tail -20`
Expected: compilation errors (types don't exist yet)

- [ ] **Step 3: Define `McpTransport` enum and update `McpServerConfig`**

Replace the flat-field `McpServerConfig` with enum-based types:

```rust
/// Resolved transport configuration after parsing.
#[derive(Debug, Clone, PartialEq)]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: HashMap<String, String>,
    },
    Http {
        url: String,
        headers: Option<HashMap<String, String>>,
    },
}

/// Parsed server configuration.
#[derive(Debug, Clone, PartialEq)]
pub struct McpServerConfig {
    pub name: String,
    pub transport: McpTransport,
}
```

- [ ] **Step 4: Define raw deserialization types with `#[serde(tag = "type")]`**

```rust
/// Raw stdio server config for deserialization.
#[derive(Debug, Deserialize, Clone)]
struct RawStdioConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

/// Raw HTTP server config for deserialization.
#[derive(Debug, Deserialize, Clone)]
struct RawHttpConfig {
    url: String,
    #[serde(default)]
    headers: Option<HashMap<String, String>>,
}

/// Tagged union for raw deserialization — `type` field is required.
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type")]
enum RawMcpTransport {
    #[serde(rename = "stdio")]
    Stdio(RawStdioConfig),
    #[serde(rename = "http")]
    Http(RawHttpConfig),
}
```

- [ ] **Step 5: Update `try_parse_server` to use tagged enum**

Replace the current `try_parse_server` function:

```rust
fn try_parse_server(name: &str, value: &serde_json::Value) -> Option<McpServerConfig> {
    match serde_json::from_value::<RawMcpTransport>(value.clone()) {
        Ok(raw) => {
            let transport = match raw {
                RawMcpTransport::Stdio(cfg) => McpTransport::Stdio {
                    command: cfg.command,
                    args: cfg.args,
                    env: cfg.env,
                },
                RawMcpTransport::Http(cfg) => McpTransport::Http {
                    url: cfg.url,
                    headers: cfg.headers,
                },
            };
            Some(McpServerConfig {
                name: name.to_string(),
                transport,
            })
        }
        Err(e) => {
            tracing::warn!(
                "Skipping MCP server '{}' (missing or invalid 'type' field): {}",
                name,
                e
            );
            None
        }
    }
}
```

- [ ] **Step 6: Update `merge_configs` to use new types**

Replace the server construction in `merge_configs`:

```rust
let servers = merged
    .into_iter()
    .filter_map(|(name, value)| try_parse_server(&name, &value))
    .collect();
```

- [ ] **Step 7: Run tests to verify they pass**

Run: `cd /root/nq-deribit && cargo test -p vol-llm-mcp -- --nocapture 2>&1 | tail -30`
Expected: all tests pass, including the 6 new tests

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-mcp/src/config.rs
git commit -m "feat(mcp): add McpTransport enum with type-field parsing

Replace flat-field McpServerConfig with enum-based transport types.
'type' field is required — servers without it are skipped with a warning.
Supports 'stdio' and 'http' transport variants via serde tagged enum."
```

---

### Task 2: Add HTTP connection support in manager

**Files:**
- Modify: `crates/vol-llm-mcp/Cargo.toml`
- Modify: `crates/vol-llm-mcp/src/manager.rs`

- [ ] **Step 1: Add rmcp feature flag**

In `crates/vol-llm-mcp/Cargo.toml`, update the `rmcp` dependency:

```toml
rmcp = { version = "1.6", features = ["client", "transport-io", "transport-child-process", "transport-streamable-http-client-reqwest"] }
```

Also add `reqwest` if not already transitively available:

```toml
reqwest = { version = "0.12", features = ["json"] }
```

And `http` for HeaderName/HeaderValue:

```toml
http = "1"
```

- [ ] **Step 2: Verify compilation with new feature**

Run: `cd /root/nq-deribit && cargo build -p vol-llm-mcp 2>&1 | tail -20`
Expected: compiles without errors

- [ ] **Step 3: Update `connect_single` to dispatch on transport type**

Change the function signature and add a `match` on `config.transport`:

```rust
async fn connect_single(
    config: &McpServerConfig,
    cancel_token: &CancellationToken,
) -> Result<(RunningService<RoleClient, ClientInfo>, Vec<Tool>, Vec<Resource>, Vec<ResourceTemplate>, Vec<Prompt>), McpError> {
    let service = match &config.transport {
        McpTransport::Stdio { command, args, env } => {
            connect_stdio(command, args, env, config, cancel_token).await?
        }
        McpTransport::Http { url, headers } => {
            connect_http(url, headers.as_ref(), config, cancel_token).await?
        }
    };

    let peer = service.peer();

    let tools = peer.list_all_tools().await.unwrap_or_else(|e| {
        tracing::warn!("Failed to list tools for '{}': {}", config.name, e);
        Vec::new()
    });

    let resources = peer.list_all_resources().await.unwrap_or_else(|e| {
        tracing::warn!("Failed to list resources for '{}': {}", config.name, e);
        Vec::new()
    });

    let resource_templates = peer.list_all_resource_templates().await.unwrap_or_else(|e| {
        tracing::warn!("Failed to list resource templates for '{}': {}", config.name, e);
        Vec::new()
    });

    let prompts = peer.list_all_prompts().await.unwrap_or_else(|e| {
        tracing::warn!("Failed to list prompts for '{}': {}", config.name, e);
        Vec::new()
    });

    Ok((service, tools, resources, resource_templates, prompts))
}
```

- [ ] **Step 4: Extract stdio connection into `connect_stdio` helper**

```rust
async fn connect_stdio(
    command: &str,
    args: &[String],
    env: &HashMap<String, String>,
    config: &McpServerConfig,
    cancel_token: &CancellationToken,
) -> Result<RunningService<RoleClient, ClientInfo>, McpError> {
    let mut cmd = Command::new(command);
    cmd.args(args);
    for (key, value) in env {
        cmd.env(key, value);
    }
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::inherit());

    let child = TokioChildProcess::new(cmd).map_err(|e: std::io::Error| {
        McpError::ConnectionFailed {
            server: config.name.clone(),
            detail: e.to_string(),
        }
    })?;

    let client_info = ClientInfo::default();
    tokio::time::timeout(
        Duration::from_secs(10),
        client_info.serve_with_ct(child, cancel_token.clone()),
    )
    .await
    .map_err(|_| McpError::InitializeTimeout {
        server: config.name.clone(),
    })?
    .map_err(|e| McpError::ConnectionFailed {
        server: config.name.clone(),
        detail: e.to_string(),
    })
}
```

- [ ] **Step 5: Add `connect_http` function**

```rust
async fn connect_http(
    url: &str,
    headers: Option<&HashMap<String, String>>,
    config: &McpServerConfig,
    cancel_token: &CancellationToken,
) -> Result<RunningService<RoleClient, ClientInfo>, McpError> {
    use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;

    let mut transport_config = StreamableHttpClientTransportConfig::with_uri(url);

    if let Some(hdrs) = headers {
        if !hdrs.is_empty() {
            let mut http_headers = std::collections::HashMap::new();
            for (name, value) in hdrs {
                if let (Ok(name), Ok(value)) = (
                    http::HeaderName::from_bytes(name.as_bytes()),
                    http::HeaderValue::from_str(value),
                ) {
                    http_headers.insert(name, value);
                }
            }
            transport_config = transport_config.custom_headers(http_headers);
        }
    }

    let transport =
        rmcp::transport::StreamableHttpClientTransport::from_config(transport_config);

    let client_info = ClientInfo::default();
    tokio::time::timeout(
        Duration::from_secs(10),
        client_info.serve_with_ct(transport, cancel_token.clone()),
    )
    .await
    .map_err(|_| McpError::InitializeTimeout {
        server: config.name.clone(),
    })?
    .map_err(|e| McpError::ConnectionFailed {
        server: config.name.clone(),
        detail: e.to_string(),
    })
}
```

- [ ] **Step 6: Add `McpTransport` import to manager.rs**

At the top of `manager.rs`, update the import:

```rust
use crate::config::{McpServerConfig, McpTransport};
```

- [ ] **Step 7: Run build to verify compilation**

Run: `cd /root/nq-deribit && cargo build -p vol-llm-mcp 2>&1 | tail -20`
Expected: compiles without errors

- [ ] **Step 8: Run existing manager tests to ensure they still pass**

Run: `cd /root/nq-deribit && cargo test -p vol-llm-mcp -- --nocapture 2>&1 | tail -30`
Expected: all tests pass (manager tests still use `McpTransport::Stdio` constructed manually)

Note: The existing manager tests construct `McpServerConfig` directly. They need to be updated to use the new struct shape:

```rust
let config = McpServerConfig {
    name: "failing-server".to_string(),
    transport: McpTransport::Stdio {
        command: "nonexistent-command-that-will-fail".to_string(),
        args: vec![],
        env: std::collections::HashMap::new(),
    },
};
```

- [ ] **Step 9: Update manager tests for new config shape**

Update all `McpServerConfig` constructions in `manager.rs` tests to use `transport: McpTransport::Stdio { ... }` instead of flat `command`, `args`, `env` fields.

- [ ] **Step 10: Run full test suite**

Run: `cd /root/nq-deribit && cargo test -p vol-llm-mcp -- --nocapture 2>&1 | tail -30`
Expected: all tests pass

- [ ] **Step 11: Commit**

```bash
git add crates/vol-llm-mcp/Cargo.toml crates/vol-llm-mcp/src/manager.rs
git commit -m "feat(mcp): add HTTP transport support via StreamableHttpClientTransport

Dispatch connect_single on McpTransport enum:
- Stdio → TokioChildProcess (existing behavior)
- Http → StreamableHttpClientTransport (reqwest-based)

Add transport-streamable-http-client-reqwest feature to rmcp dependency."
```

---

### Task 3: Integration test with user's ~/.mcp.json

**Files:**
- No file changes — manual verification

- [ ] **Step 1: Build the full binary**

Run: `cd /root/nq-deribit && cargo build --example jsonrpc_agent_service -p vol-llm-agent-channel 2>&1 | tail -10`
Expected: compiles without errors

- [ ] **Step 2: Run and verify all 3 servers appear**

Run: `ANTHROPIC_AUTH_TOKEN=sk cargo run --example jsonrpc_agent_service -p vol-llm-agent-channel 2>&1`
Expected: all 3 servers from `~/.mcp.json` appear in the MCP panel (stdio + 2 HTTP)

Note: The user's `~/.mcp.json` currently lacks `type` fields on existing entries. The user will need to add `"type": "stdio"` or `"type": "http"` to each server entry for them to be parsed. This is by design — `type` is required per the spec.

- [ ] **Step 3: Commit any .mcp.json updates if the user wants them**

No commit needed unless the user modifies their `.mcp.json`.
