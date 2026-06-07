# agent-server Binary Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extract the web backend from `vol-llm-agent-channel/examples/jsonrpc_agent_service.rs` into a proper binary crate `vol-agent-server` with TOML-based configuration.

**Architecture:** New `crates/vol-agent-server/` binary crate depends on `vol-llm-agent-channel` for `AgentServerCore` + `JsonRpcServer`. Config via TOML (`~/.vol/agent-server.toml` default, `--config` override). No changes to `vol-llm-agent-channel` library.

**Tech Stack:** Rust, tokio, axum, serde + toml, tracing-subscriber, vol-llm-agent-channel

---

### Task 1: Create crate skeleton

**Files:**
- Create: `crates/vol-agent-server/Cargo.toml`
- Create: `crates/vol-agent-server/src/config.rs`
- Create: `crates/vol-agent-server/src/main.rs`
- Modify: `Cargo.toml` (workspace root)

- [ ] **Step 1: Create Cargo.toml**

Write `crates/vol-agent-server/Cargo.toml`:

```toml
[package]
name = "vol-agent-server"
version.workspace = true
edition.workspace = true

[[bin]]
name = "vol-agent-server"
path = "src/main.rs"

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
toml = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
axum = { workspace = true }
vol-llm-agent-channel = { path = "../vol-llm-agent-channel" }
```

- [ ] **Step 2: Add crate to workspace members**

Edit `Cargo.toml` (workspace root), add `"crates/vol-agent-server"` to the `members` array after `"crates/vol-llm-agent-channel"`:

```diff
     "crates/vol-llm-agent-channel",
+    "crates/vol-agent-server",
     "crates/vol-llm-ui",
```

- [ ] **Step 3: Verify crate skeleton compiles (empty main)**

Write a minimal `crates/vol-agent-server/src/main.rs`:

```rust
fn main() {
    println!("vol-agent-server");
}
```

Write an empty `crates/vol-agent-server/src/config.rs`:

```rust
//! Server configuration via TOML.
```

Run: `cargo check -p vol-agent-server`
Expected: compiles successfully.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-server/Cargo.toml crates/vol-agent-server/src/main.rs crates/vol-agent-server/src/config.rs Cargo.toml
git commit -m "feat(agent-server): add crate skeleton with workspace membership"
```

---

### Task 2: Implement config.rs

**Files:**
- Modify: `crates/vol-agent-server/src/config.rs`

- [ ] **Step 1: Write config.rs with all structs and defaults**

Replace `crates/vol-agent-server/src/config.rs` with:

```rust
//! Server configuration via TOML.
//!
//! Loads from `~/.vol/agent-server.toml` by default, or from `--config <path>`.

use serde::Deserialize;
use std::path::PathBuf;

/// Top-level server configuration.
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

// --- Defaults ---

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    3001
}

fn default_working_dir() -> String {
    ".".to_string()
}

fn default_store_dir() -> String {
    "~/.vol".to_string()
}

fn default_level() -> String {
    "info".to_string()
}

fn default_format() -> String {
    "text".to_string()
}

// --- Default trait implementations ---

impl Default for ServerSection {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
        }
    }
}

impl Default for RuntimeSection {
    fn default() -> Self {
        Self {
            working_dir: default_working_dir(),
            store_dir: default_store_dir(),
        }
    }
}

impl Default for TracingSection {
    fn default() -> Self {
        Self {
            level: default_level(),
            format: default_format(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            server: ServerSection::default(),
            runtime: RuntimeSection::default(),
            tracing: TracingSection::default(),
        }
    }
}

// --- Load ---

impl ServerConfig {
    /// Load config from a TOML file path.
    pub fn load(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Failed to read config file {:?}: {}", path, e))?;
        toml::from_str(&content)
            .map_err(|e| format!("Failed to parse config {:?}: {}", path, e))
    }

    /// Load from explicit path, or fall back to default path, or use pure defaults.
    pub fn load_or_default(explicit: Option<&str>) -> Result<(Self, Option<PathBuf>), String> {
        if let Some(p) = explicit {
            let path = PathBuf::from(p);
            let config = Self::load(&path)?;
            return Ok((config, Some(path)));
        }
        let default_path = default_config_path();
        if default_path.exists() {
            let config = Self::load(&default_path)?;
            return Ok((config, Some(default_path)));
        }
        Ok((ServerConfig::default(), None))
    }

    /// Expand `~` in path fields to home directory.
    pub fn expand_tilde(&mut self) {
        self.runtime.working_dir = expand_tilde_str(&self.runtime.working_dir);
        self.runtime.store_dir = expand_tilde_str(&self.runtime.store_dir);
    }
}

/// Default config path: `~/.vol/agent-server.toml`
fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(format!("{}/.vol/agent-server.toml", home))
}

fn expand_tilde_str(s: &str) -> String {
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let rest = s.trim_start_matches('~').trim_start_matches('/');
        if rest.is_empty() {
            home
        } else {
            format!("{}/{}", home, rest)
        }
    } else {
        s.to_string()
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 3001);
        assert_eq!(config.runtime.working_dir, ".");
        assert_eq!(config.runtime.store_dir, "~/.vol");
        assert_eq!(config.tracing.level, "info");
        assert_eq!(config.tracing.format, "text");
    }

    #[test]
    fn test_expand_tilde() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let result = expand_tilde_str("~/foo/bar");
        assert_eq!(result, format!("{}/foo/bar", home));
    }

    #[test]
    fn test_expand_tilde_home_only() {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/home/user".to_string());
        let result = expand_tilde_str("~");
        assert_eq!(result, home);
    }

    #[test]
    fn test_expand_no_tilde() {
        let result = expand_tilde_str("/absolute/path");
        assert_eq!(result, "/absolute/path");
    }

    #[test]
    fn test_parse_minimal_toml() {
        let toml_str = "";
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 3001);
    }

    #[test]
    fn test_parse_partial_toml() {
        let toml_str = r#"
[server]
port = 8080

[tracing]
level = "debug"
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.host, "0.0.0.0"); // default preserved
        assert_eq!(config.tracing.level, "debug");
        assert_eq!(config.tracing.format, "text"); // default preserved
        assert_eq!(config.runtime.working_dir, "."); // default preserved
    }

    #[test]
    fn test_parse_full_toml() {
        let toml_str = r#"
[server]
host = "127.0.0.1"
port = 9090

[runtime]
working_dir = "/app"
store_dir = "/data"

[tracing]
level = "debug"
format = "json"
"#;
        let config: ServerConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.runtime.working_dir, "/app");
        assert_eq!(config.runtime.store_dir, "/data");
        assert_eq!(config.tracing.level, "debug");
        assert_eq!(config.tracing.format, "json");
    }
}
```

- [ ] **Step 2: Run config tests**

```bash
cargo test -p vol-agent-server
```

Expected: all 7 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-server/src/config.rs
git commit -m "feat(agent-server): add TOML config with defaults and tilde expansion"
```

---

### Task 3: Implement main.rs

**Files:**
- Modify: `crates/vol-agent-server/src/main.rs`

- [ ] **Step 1: Write main.rs**

Replace `crates/vol-agent-server/src/main.rs` with:

```rust
//! vol-agent-server: JSON-RPC agent service binary.
//!
//! Serves agent operations via JSON-RPC 2.0 over WebSocket.
//!
//! ## Usage
//!
//! ```bash
//! # Default config (~/.vol/agent-server.toml or built-in defaults)
//! vol-agent-server
//!
//! # Explicit config
//! vol-agent-server --config ./my-config.toml
//! ```

use std::sync::Arc;

use vol_llm_agent_channel::{AgentServerCore, JsonRpcServer};

mod config;
use config::ServerConfig;

#[tokio::main]
async fn main() {
    // --- Parse --config flag ---
    let explicit_config = std::env::args()
        .nth(1)
        .and_then(|arg| {
            if arg == "--config" {
                std::env::args().nth(2)
            } else if arg.starts_with("--config=") {
                Some(arg.trim_start_matches("--config=").to_string())
            } else {
                None
            }
        });

    // --- Load config ---
    let (mut config, config_path) = ServerConfig::load_or_default(explicit_config.as_deref())
        .unwrap_or_else(|e| {
            eprintln!("Config error: {}", e);
            std::process::exit(1);
        });
    config.expand_tilde();

    // --- Init tracing ---
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| {
            tracing_subscriber::EnvFilter::new(&config.tracing.level)
        });

    match config.tracing.format.as_str() {
        "json" => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .json()
                .init();
        }
        _ => {
            tracing_subscriber::fmt()
                .with_env_filter(env_filter)
                .init();
        }
    }

    if let Some(ref path) = config_path {
        tracing::info!("Config loaded from {:?}", path);
    } else {
        tracing::info!("Using built-in defaults (no config file found)");
    }

    // --- Build core ---
    tracing::info!(
        working_dir = %config.runtime.working_dir,
        store_dir = %config.runtime.store_dir,
        "Building AgentServerCore"
    );

    let core = AgentServerCore::new(&config.runtime.working_dir, &config.runtime.store_dir)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to build AgentServerCore: {}", e);
            std::process::exit(1);
        });

    // --- Discover agents ---
    core.discover_agents()
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to discover agents: {}", e);
            std::process::exit(1);
        });

    // --- Start server ---
    let server = JsonRpcServer::new(Arc::new(core));
    let app = server.into_axum_router();

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Failed to bind {}: {}", addr, e);
            std::process::exit(1);
        });

    tracing::info!("JSON-RPC server started on ws://{}", addr);
    tracing::info!("  Methods: agent.submit, agent.cancel, agent.approve");
    tracing::info!("           agent.list, agent.subscribe, agent.unsubscribe");
    tracing::info!("           file.list, file.read");
    tracing::info!("           log.list, log.read");
    tracing::info!("           session.list, session.resume");
    tracing::info!("           mcp.* (list_servers, list_tools, call_tool, etc.)");
    tracing::info!("           skill.list, skill.get");
    tracing::info!("           task.list, task.output");

    axum::serve(listener, app)
        .await
        .unwrap_or_else(|e| {
            tracing::error!("Server error: {}", e);
            std::process::exit(1);
        });
}
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p vol-agent-server
```

Expected: compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-server/src/main.rs
git commit -m "feat(agent-server): implement main.rs with config-driven startup"
```

---

### Task 4: Update Makefile web-backend target

**Files:**
- Modify: `Makefile`

- [ ] **Step 1: Update web-backend target**

Replace the `web-backend` line in `Makefile` (line 16-17):

```diff
- web-backend: ## Start backend JSON-RPC agent service (port 3001)
- 	ANTHROPIC_AUTH_TOKEN=sk cargo watch -x "run --example jsonrpc_agent_service -p vol-llm-agent-channel"
+ web-backend: ## Start backend JSON-RPC agent service (port 3001)
+ 	ANTHROPIC_AUTH_TOKEN=sk cargo watch -x "run -p vol-agent-server"
```

- [ ] **Step 2: Verify Makefile syntax**

```bash
make -n web-backend
```

Expected: shows the new command without errors.

- [ ] **Step 3: Commit**

```bash
git add Makefile
git commit -m "chore: switch web-backend from example to vol-agent-server binary"
```

---

### Task 5: Remove old example

**Files:**
- Delete: `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`

- [ ] **Step 1: Delete the old example file**

```bash
rm crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs
```

- [ ] **Step 2: Verify remaining examples still compile**

```bash
cargo check --example multi_agent -p vol-llm-agent-channel
cargo check --example single_agent -p vol-llm-agent-channel
```

Expected: both examples compile successfully.

- [ ] **Step 3: Commit**

```bash
git rm crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs
git commit -m "chore: remove jsonrpc_agent_service example (superseded by vol-agent-server binary)"
```

---

### Task 6: End-to-end verification

- [ ] **Step 1: Full check**

```bash
cargo check -p vol-agent-server
cargo test -p vol-agent-server
```

Expected: compile + all tests pass.

- [ ] **Step 2: Verify no references to old example remain**

```bash
rg "jsonrpc_agent_service" crates/ Makefile docs/
```

Expected: no results.

- [ ] **Step 3: Verify binary runs and shows startup messages (dry run)**

```bash
ANTHROPIC_AUTH_TOKEN=sk-any cargo run -p vol-agent-server 2>&1 | head -5
```

Expected: shows tracing output with "Building AgentServerCore" and config info. Kill with Ctrl+C after confirming.

- [ ] **Step 4: Commit any remaining changes**

```bash
git status
# Commit if anything left
```
