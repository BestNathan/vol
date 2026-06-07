# vol-agent-server: Web Backend Binary

**Date:** 2026-06-05
**Status:** design-approved

## Summary

Extract the web backend from `vol-llm-agent-channel/examples/jsonrpc_agent_service.rs` (an example)
into a proper binary crate `crates/vol-agent-server` with TOML-based configuration.

The backend logic (`AgentServerCore`, `JsonRpcServer`) already lives in `vol-llm-agent-channel`
and is production-quality. The only missing piece is a proper binary entry point with
configuration.

## Architecture

```
crates/vol-agent-server/          (NEW: binary crate)
├── Cargo.toml                    depends on vol-llm-agent-channel
└── src/
    ├── main.rs                   bin entry: parse args, load config, start server
    └── config.rs                 ServerConfig structs + load from TOML

crates/vol-llm-agent-channel/     (UNCHANGED: library crate)
├── examples/jsonrpc_agent_service.rs   (REMOVED: replaced by the new binary)
└── src/...                             (UNCHANGED)

Makefile                           (UPDATED: web-backend target)
```

## TOML Configuration

### Schema

```toml
[server]
host = "0.0.0.0"    # default: "0.0.0.0"
port = 3001          # default: 3001

[runtime]
working_dir = "."    # default: "."
store_dir = "~/.vol" # default: "~/.vol"

[tracing]
level = "info"       # default: "info"  (trace/debug/info/warn/error)
format = "text"      # default: "text"  (text/json)
```

All fields have serde defaults. An empty file is valid.

### Config Resolution

1. If `--config <path>` is passed, load that file (fail if missing/invalid).
2. Otherwise, try `~/.vol/agent-server.toml`; if not found, use defaults.
3. `~` in paths is expanded via `HOME` env var, falling back to `/tmp`.

### Rust Structs

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default)]
    pub server: ServerSection,
    #[serde(default)]
    pub runtime: RuntimeSection,
    #[serde(default)]
    pub tracing: TracingSection,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerSection {
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeSection {
    #[serde(default = "default_working_dir")]
    pub working_dir: String,
    #[serde(default = "default_store_dir")]
    pub store_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TracingSection {
    #[serde(default = "default_level")]
    pub level: String,
    #[serde(default = "default_format")]
    pub format: String,
}
```

## CLI Usage

```bash
# Default: loads ~/.vol/agent-server.toml (falls back to defaults)
vol-agent-server

# Explicit config
vol-agent-server --config ./my-config.toml

# With env overrides (for LLM auth tokens)
ANTHROPIC_AUTH_TOKEN=sk-xxx vol-agent-server
```

CLI parsing: `--config` flag via `std::env::args()`. No heavy CLI framework needed —
it's a single optional flag.

## Binary Behavior

1. Parse `--config` from CLI args (or use default path `~/.vol/agent-server.toml`).
2. Load TOML config (use defaults if file absent at default path).
3. Expand `~` in paths.
4. Init `tracing_subscriber` with configured level/format.
5. Build `AgentServerCore` from `runtime.working_dir` and `runtime.store_dir`.
6. `core.discover_agents().await`.
7. Create `JsonRpcServer`, build axum router, bind to `server.host:server.port`.
8. Log startup info and serve.

## File Changes

### New Files

| File | Description |
|------|-------------|
| `crates/vol-agent-server/Cargo.toml` | Crate manifest depends on `vol-llm-agent-channel`, `serde`, `toml`, `tracing-subscriber`, `tokio`, `axum` |
| `crates/vol-agent-server/src/main.rs` | Binary entry point |
| `crates/vol-agent-server/src/config.rs` | Config structs + load logic |

### Modified Files

| File | Change |
|------|--------|
| `Cargo.toml` (workspace root) | Add `crates/vol-agent-server` to workspace members |
| `Makefile` | Update `web-backend` target from example to binary |

### Removed Files

| File | Reason |
|------|--------|
| `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs` | Superseded by the new binary |

## Tests

- `config.rs`: unit tests for TOML parsing, defaults, `~` expansion.

## Scope Boundaries

- **In scope**: New `vol-agent-server` crate, config loading, binary entry point, Makefile update, remove old example.
- **Out of scope**: Changes to `vol-llm-agent-channel` library, changes to `vol-llm-ui`, new features beyond config-based startup, hot-reload, TLS, auth middleware.
