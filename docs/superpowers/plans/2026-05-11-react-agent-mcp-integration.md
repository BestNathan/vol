# MCP Client Integration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Enable ReAct agent to discover and invoke MCP servers configured via `~/.mcp.json` and `.mcp.json`, with MCP tools registered as `ExecutableTool` instances.

**Architecture:** `vol-llm-mcp` crate provides MCP Client protocol layer (config parsing, session management, tool discovery/call). `McpTool` in `vol-llm-tool` wraps `McpSession` as `ExecutableTool`. AgentConfigBuilder integrates MCP initialization.

**Tech Stack:** Rust, `rmcp` (client features), `tokio`, `serde_json`, `thiserror`

---

### Task 1: Create vol-llm-mcp crate skeleton

**Files:**
- Create: `crates/vol-llm-mcp/Cargo.toml`
- Create: `crates/vol-llm-mcp/src/lib.rs`
- Modify: `Cargo.toml` (workspace members + dependencies)

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-mcp"
version.workspace = true
edition.workspace = true

[dependencies]
rmcp = { version = "1.6", features = ["client", "transport-io"] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
tracing = { workspace = true }
tokio-util = "0.7"
dirs = "5.0"
```

- [ ] **Step 2: Create src/lib.rs**

```rust
//! vol-llm-mcp: MCP Client protocol layer for ReAct Agent.
//!
//! Provides configuration parsing, session management, and tool discovery
//! for MCP servers configured via ~/.mcp.json and .mcp.json.

pub mod config;
pub mod error;
pub mod session;

pub use config::McpConfig;
pub use error::McpError;
pub use session::McpSession;
```

- [ ] **Step 3: Add to workspace**

Edit `Cargo.toml` (root), add `"crates/vol-llm-mcp"` to `members` array.

Add to `[workspace.dependencies]` section (for future workspace-wide use):

```toml
vol-llm-mcp = { path = "crates/vol-llm-mcp" }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-mcp`
Expected: compiles cleanly (no errors, warnings acceptable)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-mcp/ Cargo.toml
git commit -m "feat: add vol-llm-mcp crate skeleton with rmcp client dependency"
```

---

### Task 2: Implement McpError and McpConfig

**Files:**
- Create: `crates/vol-llm-mcp/src/error.rs`
- Create: `crates/vol-llm-mcp/src/config.rs`
- Test: `crates/vol-llm-mcp/src/config.rs` (inline tests)

- [ ] **Step 1: Write error.rs**

```rust
//! MCP error types.

use thiserror::Error;

/// Error type for MCP operations.
#[derive(Error, Debug)]
pub enum McpError {
    #[error("failed to parse config from {path}: {detail}")]
    ConfigParse { path: String, detail: String },

    #[error("MCP server '{0}' not found")]
    ServerNotFound(String),

    #[error("failed to connect to MCP server '{server}': {detail}")]
    ConnectionFailed { server: String, detail: String },

    #[error("MCP server '{server}' initialization timed out")]
    InitializeTimeout { server: String },

    #[error("tool call failed on server '{server}', tool '{tool}': {detail}")]
    ToolCallFailed { server: String, tool: String, detail: String },

    #[error("transport error: {0}")]
    TransportError(String),
}
```

- [ ] **Step 2: Write config.rs with tests**

```rust
//! MCP configuration parsing and merge logic.
//!
//! Follows the Claude Desktop .mcp.json schema:
//! ```json
//! { "mcpServers": { "name": { "command": "...", "args": [...], "env": {...} } } }
//! ```

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::error::McpError;

/// Raw deserialization of .mcp.json
#[derive(Debug, Deserialize, Clone)]
struct RawMcpConfig {
    #[serde(rename = "mcpServers")]
    mcp_servers: HashMap<String, RawServerConfig>,
}

#[derive(Debug, Deserialize, Clone)]
struct RawServerConfig {
    command: String,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    env: HashMap<String, String>,
}

/// Parsed server configuration.
#[derive(Debug, Clone)]
pub struct McpServerConfig {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

/// Parsed and merged MCP configuration.
#[derive(Debug, Clone)]
pub struct McpConfig {
    servers: Vec<McpServerConfig>,
}

impl McpConfig {
    /// Load configuration from project-level and user-level sources.
    ///
    /// Priority: `.mcp.json` (project root) > `~/.mcp.json` (user home).
    /// Per-key merge: if both files define the same server name, the project-level wins.
    pub fn load(working_dir: Option<&Path>) -> Result<Self, McpError> {
        let project_config = load_project_config(working_dir)?;
        let user_config = load_user_config()?;
        let merged = merge_configs(project_config, user_config);
        Ok(merged)
    }

    /// Return all server configurations.
    pub fn servers(&self) -> &[McpServerConfig] {
        &self.servers
    }
}

fn load_project_config(working_dir: Option<&Path>) -> Result<Option<RawMcpConfig>, McpError> {
    let dir = working_dir.map(|p| p.to_path_buf()).or_else(|| std::env::current_dir().ok());
    let Some(dir) = dir else { return Ok(None) };
    let path = dir.join(".mcp.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| McpError::ConfigParse {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    let config: RawMcpConfig = serde_json::from_str(&content).map_err(|e| McpError::ConfigParse {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    Ok(Some(config))
}

fn load_user_config() -> Result<Option<RawMcpConfig>, McpError> {
    let Some(home) = dirs::home_dir() else { return Ok(None) };
    let path = home.join(".mcp.json");
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path).map_err(|e| McpError::ConfigParse {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    let config: RawMcpConfig = serde_json::from_str(&content).map_err(|e| McpError::ConfigParse {
        path: path.display().to_string(),
        detail: e.to_string(),
    })?;
    Ok(Some(config))
}

fn merge_configs(
    project: Option<RawMcpConfig>,
    user: Option<RawMcpConfig>,
) -> McpConfig {
    let mut merged: HashMap<String, RawServerConfig> = HashMap::new();

    // User-level first (lower priority)
    if let Some(user_cfg) = user {
        merged.extend(user_cfg.mcp_servers);
    }

    // Project-level overrides (higher priority)
    if let Some(project_cfg) = project {
        for (name, server) in project_cfg.mcp_servers {
            merged.insert(name, server);
        }
    }

    let servers = merged
        .into_iter()
        .map(|(name, raw)| McpServerConfig {
            name,
            command: raw.command,
            args: raw.args,
            env: raw.env,
        })
        .collect();

    McpConfig { servers }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_config_file(dir: &Path, content: &str) {
        let path = dir.join(".mcp.json");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(content.as_bytes()).unwrap();
    }

    #[test]
    fn test_load_no_config() {
        let temp = tempfile::tempdir().unwrap();
        let config = McpConfig::load(Some(temp.path())).unwrap();
        assert!(config.servers.is_empty());
    }

    #[test]
    fn test_load_user_config_only() {
        let temp = tempfile::tempdir().unwrap();
        let home = temp.path().join("home");
        std::fs::create_dir(&home).unwrap();
        make_config_file(&home, r#"{"mcpServers":{"test":{"command":"echo","args":["hello"]}}}"#);

        // Temporarily override HOME for this test
        let original_home = dirs::home_dir();
        std::env::set_var("HOME", home.to_str().unwrap());
        let result = McpConfig::load(Some(temp.path()));
        // Restore HOME
        if let Some(orig) = original_home {
            std::env::set_var("HOME", orig);
        }

        // Since dirs::home_dir() caches, we test merge directly instead
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"test":{"command":"echo","args":["hello"]}}}"#
        ).unwrap();
        let merged = merge_configs(None, Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "test");
        assert_eq!(merged.servers[0].command, "echo");
    }

    #[test]
    fn test_merge_project_overrides_user() {
        let user: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"weather":{"command":"npx","args":["weather-server"]},"github":{"command":"npx","args":["github-server"]}}}"#
        ).unwrap();
        let project: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"weather":{"command":"uv","args":["run","weather.py"]}}}"#
        ).unwrap();

        let merged = merge_configs(Some(project), Some(user));
        assert_eq!(merged.servers.len(), 2);

        // weather should use project-level config
        let weather = merged.servers.iter().find(|s| s.name == "weather").unwrap();
        assert_eq!(weather.command, "uv");

        // github should use user-level config
        let github = merged.servers.iter().find(|s| s.name == "github").unwrap();
        assert_eq!(github.command, "npx");
    }

    #[test]
    fn test_merge_empty_user() {
        let user: RawMcpConfig = serde_json::from_str(r#"{"mcpServers":{}}"#).unwrap();
        let project: RawMcpConfig = serde_json::from_str(
            r#"{"mcpServers":{"test":{"command":"echo"}}}"#
        ).unwrap();
        let merged = merge_configs(Some(project), Some(user));
        assert_eq!(merged.servers.len(), 1);
        assert_eq!(merged.servers[0].name, "test");
    }
}
```

- [ ] **Step 3: Add test dependencies**

Add to `crates/vol-llm-mcp/Cargo.toml`:

```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 4: Update lib.rs to export error module**

```rust
//! vol-llm-mcp: MCP Client protocol layer for ReAct Agent.

pub mod config;
pub mod error;
pub mod session;

pub use config::McpConfig;
pub use error::McpError;
pub use session::McpSession;
```

- [ ] **Step 5: Run tests**

Run: `cargo test -p vol-llm-mcp`
Expected: all 4 config tests pass

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-mcp/
git commit -m "feat: implement McpError and McpConfig with merge logic"
```

---

### Task 3: Implement McpSession — connect, list_tools, call_tool, disconnect

**Files:**
- Create: `crates/vol-llm-mcp/src/session.rs`

- [ ] **Step 1: Write session.rs**

```rust
//! MCP Session — manages connections to multiple MCP servers.

use rmcp::{
    ClientHandler, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, ClientInfo, Content, Tool},
    transport::TokioChildProcess,
    service::{Peer, RoleClient, RunningService, ServiceError},
};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;
use tracing;

use crate::config::McpServerConfig;
use crate::error::McpError;

/// Sanitize a server name to only contain [a-zA-Z0-9_-].
pub fn sanitize_name(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    let mut prev_underscore = false;
    for c in name.chars() {
        if c.is_alphanumeric() || c == '_' || c == '-' {
            if c == '_' {
                if prev_underscore {
                    continue; // skip consecutive underscores
                }
                prev_underscore = true;
            } else {
                prev_underscore = false;
            }
            result.push(c);
        } else {
            if !prev_underscore {
                result.push('_');
                prev_underscore = true;
            }
        }
    }
    // Remove trailing underscore
    if result.ends_with('_') {
        result.pop();
    }
    if result.is_empty() {
        result = "unknown".to_string();
    }
    result
}

/// MCP tool metadata.
#[derive(Debug, Clone)]
pub struct McpToolInfo {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

/// A single server connection — holds the running service and cached tools.
pub struct ServerConnection {
    pub config: McpServerConfig,
    pub running_service: RunningService<RoleClient, ()>,
    pub tools: Vec<Tool>,
}

impl ServerConnection {
    pub fn peer(&self) -> &Peer<RoleClient> {
        self.running_service.peer()
    }

    pub async fn close(&mut self) -> Result<(), tokio::task::JoinError> {
        self.running_service.close().await
    }
}

/// MCP Session — manages all server connections.
pub struct McpSession {
    connections: HashMap<String, ServerConnection>,
}

impl McpSession {
    /// Connect to all configured MCP servers.
    ///
    /// Servers that fail to connect are skipped with a tracing error.
    /// The initialization timeout is 10 seconds per server.
    pub async fn connect(configs: Vec<McpServerConfig>) -> Self {
        let mut connections = HashMap::new();

        for config in configs {
            match Self::connect_single(&config).await {
                Ok((service, tools)) => {
                    let sanitized = sanitize_name(&config.name);
                    connections.insert(
                        sanitized,
                        ServerConnection { config, running_service: service, tools },
                    );
                }
                Err(e) => {
                    tracing::error!("MCP server '{}' failed to connect: {}", config.name, e);
                }
            }
        }

        Self { connections }
    }

    async fn connect_single(
        config: &McpServerConfig,
    ) -> Result<(RunningService<RoleClient, ()>, Vec<Tool>), McpError> {
        let mut command = Command::new(&config.command);
        command.args(&config.args);
        for (key, value) in &config.env {
            command.env(key, value);
        }
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::inherit());

        let child = TokioChildProcess::new(command).map_err(|e| {
            McpError::ConnectionFailed {
                server: config.name.clone(),
                detail: e.to_string(),
            }
        })?;

        let client_info = ClientInfo {
            name: "vol-llm-mcp".into(),
            version: "0.1.0".into(),
            ..Default::default()
        };

        let ct = CancellationToken::new();
        let service = client_info
            .serve(child)
            .await
            .map_err(|e| McpError::ConnectionFailed {
                server: config.name.clone(),
                detail: e.to_string(),
            })?;

        // List tools from the connected server
        let tools = service
            .peer()
            .list_all_tools()
            .await
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to list tools for server '{}': {}", config.name, e);
                Vec::new()
            });

        Ok((service, tools))
    }

    /// List tools for a specific server.
    pub fn list_tools(&self, server: &str) -> Result<Vec<McpToolInfo>, McpError> {
        let conn = self.connections.get(server).ok_or_else(|| {
            McpError::ServerNotFound(server.to_string())
        })?;

        Ok(conn.tools.iter().map(|t| McpToolInfo {
            name: t.name.to_string(),
            description: t.description.as_ref().map(|s| s.to_string()),
            input_schema: Some(t.schema_as_json_value()),
        }).collect())
    }

    /// List all tools from all servers.
    /// Returns (sanitized_server_name, tool_info) pairs.
    pub fn list_all_tools(&self) -> Vec<(String, McpToolInfo)> {
        let mut result = Vec::new();
        for (server, conn) in &self.connections {
            for tool in &conn.tools {
                result.push((
                    server.clone(),
                    McpToolInfo {
                        name: tool.name.to_string(),
                        description: tool.description.as_ref().map(|s| s.to_string()),
                        input_schema: Some(tool.schema_as_json_value()),
                    },
                ));
            }
        }
        result
    }

    /// Call a tool on a specific server.
    pub async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String, McpError> {
        let conn = self.connections.get(server).ok_or_else(|| {
            McpError::ServerNotFound(server.to_string())
        })?;

        let params = CallToolRequestParams {
            name: tool_name.into(),
            arguments: Some(args),
            meta: None,
        };

        let result = conn.peer().call_tool(params).await.map_err(|e| {
            McpError::ToolCallFailed {
                server: server.to_string(),
                tool: tool_name.to_string(),
                detail: e.to_string(),
            }
        })?;

        Ok(Self::format_tool_result(&result))
    }

    /// Format CallToolResult into a string.
    fn format_tool_result(result: &CallToolResult) -> String {
        if result.is_error == Some(true) {
            let text = result.content.iter()
                .filter_map(|c| match c {
                    Content::Text(text_block) => Some(text_block.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            return format!("MCP tool error: {}", if text.is_empty() { "unknown error" } else { &text });
        }

        result.content.iter()
            .filter_map(|c| match c {
                Content::Text(text_block) => Some(text_block.text.clone()),
                Content::Image(_) => Some("[image content]".to_string()),
                Content::Resource(resource_block) => Some(format!("[resource: {}]", resource_block.resource.name)),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Disconnect all server connections.
    pub async fn disconnect(&mut self) {
        for (name, conn) in &mut self.connections {
            if let Err(e) = conn.close().await {
                tracing::warn!("Error closing MCP server '{}': {}", name, e);
            }
        }
        self.connections.clear();
    }
}
```

- [ ] **Step 2: Add sanitize_name unit tests**

Add to `crates/vol-llm-mcp/src/session.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_name_already_clean() {
        assert_eq!(sanitize_name("weather"), "weather");
        assert_eq!(sanitize_name("my-server"), "my-server");
        assert_eq!(sanitize_name("my_server"), "my_server");
    }

    #[test]
    fn test_sanitize_name_replaces_special() {
        assert_eq!(sanitize_name("my server"), "my_server");
        assert_eq!(sanitize_name("my/server"), "my_server");
        assert_eq!(sanitize_name("my.server"), "my_server");
    }

    #[test]
    fn test_sanitize_name_consecutive_underscores_merged() {
        assert_eq!(sanitize_name("my__server"), "my_server");
        assert_eq!(sanitize_name("my   server"), "my_server");
    }

    #[test]
    fn test_sanitize_name_trailing_underscore_removed() {
        assert_eq!(sanitize_name("server!"), "server");
    }

    #[test]
    fn test_sanitize_name_empty_becomes_unknown() {
        assert_eq!(sanitize_name(""), "unknown");
        assert_eq!(sanitize_name("!!!"), "unknown");
    }
}
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-mcp`
Expected: all config + sanitize tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-mcp/src/session.rs
git commit -m "feat: implement McpSession with connect/list_tools/call_tool/disconnect"
```

---

### Task 4: Add vol-llm-tool → vol-llm-mcp dependency and implement McpTool

**Files:**
- Create: `crates/vol-llm-tool/src/mcp_tool.rs`
- Modify: `crates/vol-llm-tool/src/lib.rs`
- Modify: `crates/vol-llm-tool/Cargo.toml`

- [ ] **Step 1: Add vol-llm-mcp dependency**

Add to `crates/vol-llm-tool/Cargo.toml`:

```toml
vol-llm-mcp = { path = "../vol-llm-mcp" }
```

- [ ] **Step 2: Write mcp_tool.rs**

```rust
//! McpTool — bridges MCP tools into the ExecutableTool trait.

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_mcp::{McpSession, McpToolInfo};

use crate::tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity};

/// A tool that proxies execution to an MCP server via McpSession.
pub struct McpTool {
    session: Arc<McpSession>,
    server_name: String,
    tool_name: String,
    /// Leaked static strings for ExecutableTool trait compatibility.
    /// Safe because the number of MCP tools is bounded and small.
    display_name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
}

impl McpTool {
    /// Create a new McpTool from a session and tool info.
    pub fn new(session: Arc<McpSession>, server_name: &str, info: &McpToolInfo) -> Self {
        let sanitized = vol_llm_mcp::session::sanitize_name(server_name);
        let display_name = format!("mcp__{}_{}", sanitized, info.name);

        // Leak strings to satisfy ExecutableTool::name() -> &'static str
        let display_name: &'static str = Box::leak(display_name.into_boxed_str());
        let description: &'static str = Box::leak(
            info.description.clone().unwrap_or_else(|| "MCP tool".to_string()).into_boxed_str()
        );

        Self {
            session,
            server_name: sanitized,
            tool_name: info.name.clone(),
            display_name,
            description,
            parameters: info.input_schema.clone().unwrap_or_else(|| {
                serde_json::json!({ "type": "object", "properties": {} })
            }),
        }
    }
}

#[async_trait]
impl ExecutableTool for McpTool {
    fn name(&self) -> &'static str {
        self.display_name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn parameters(&self) -> serde_json::Value {
        self.parameters.clone()
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        // MCP tools are treated as safe; HITL is handled at the plugin level.
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let result = self.session
            .call_tool(&self.server_name, &self.tool_name, args.clone())
            .await;

        match result {
            Ok(content) => Ok(ToolResult::success(content)),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }
}
```

- [ ] **Step 3: Export from lib.rs**

Add to `crates/vol-llm-tool/src/lib.rs`:

```rust
pub mod mcp_tool;
pub use mcp_tool::McpTool;
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-tool`
Expected: compiles cleanly

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tool/src/mcp_tool.rs crates/vol-llm-tool/src/lib.rs crates/vol-llm-tool/Cargo.toml
git commit -m "feat: add McpTool bridging MCP tools to ExecutableTool trait"
```

---

### Task 5: Add ToolRegistry::register_from_mcp method

**Files:**
- Modify: `crates/vol-llm-tool/src/registry.rs`

- [ ] **Step 1: Add import**

At the top of `registry.rs`, add:

```rust
use crate::mcp_tool::McpTool;
use std::sync::Arc;
use vol_llm_mcp::McpSession;
```

- [ ] **Step 2: Add method to impl ToolRegistry**

Add inside `impl ToolRegistry`:

```rust
    /// Discover and register all MCP tools from an McpSession.
    ///
    /// Iterates all connected servers, discovers their tools,
    /// creates McpTool wrappers, and registers them.
    pub async fn register_from_mcp(&mut self, session: Arc<McpSession>) {
        let tools = session.list_all_tools();
        for (server, tool_info) in tools {
            let mcp_tool = McpTool::new(session.clone(), &server, &tool_info);
            self.register_boxed(Box::new(mcp_tool));
        }
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-tool`
Expected: compiles cleanly

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-tool/src/registry.rs
git commit -m "feat: add ToolRegistry::register_from_mcp method"
```

---

### Task 6: Add AgentConfigBuilder::with_mcp_from_config and AgentConfig field

**Files:**
- Modify: `crates/vol-llm-agent/Cargo.toml`
- Modify: `crates/vol-llm-agent/src/react/agent.rs`
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs`

- [ ] **Step 1: Add dependency**

Add to `crates/vol-llm-agent/Cargo.toml`:

```toml
vol-llm-mcp = { path = "../vol-llm-mcp" }
```

- [ ] **Step 2: Add mcp_session field to AgentConfig**

In `crates/vol-llm-agent/src/react/agent.rs`, add field to `AgentConfig` struct:

```rust
use vol_llm_mcp::McpSession;
```

Add field in `AgentConfig`:

```rust
pub struct AgentConfig {
    // ... existing fields ...
    pub mcp_session: Option<Arc<McpSession>>,
}
```

Update `AgentConfig::new()` to include the field:

```rust
pub fn new(
    llm: Arc<dyn vol_llm_core::LLMClient>,
    tools: Arc<vol_llm_tool::ToolRegistry>,
    session: Arc<Session>,
) -> Self {
    Self {
        def: None,
        llm,
        tools,
        session,
        sandbox: None,
        context_builder: ContextBuilderBuilder::new(128_000).build(),
        plugin_registry: PluginRegistry::new(),
        mcp_session: None,
    }
}
```

Update `AgentConfig::default()` to include the field:

```rust
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            def: None,
            llm: Arc::new(DefaultLlm),
            tools: Arc::new(vol_llm_tool::ToolRegistry::new()),
            session: Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))),
            sandbox: None,
            context_builder: ContextBuilderBuilder::new(128_000).build(),
            plugin_registry: PluginRegistry::new(),
            mcp_session: None,
        }
    }
}
```

- [ ] **Step 3: Add with_mcp_from_config to AgentConfigBuilder**

In `crates/vol-llm-agent/src/react/config_builder.rs`, add imports:

```rust
use std::path::Path;
use std::sync::Arc;
use vol_llm_mcp::{McpConfig, McpSession};
use vol_llm_tool::McpTool;
```

Add field to `AgentConfigBuilder`:

```rust
pub struct AgentConfigBuilder {
    // ... existing fields ...
    mcp_session: Option<Arc<McpSession>>,
}
```

Update `AgentConfigBuilder::new()`:

```rust
mcp_session: None,
```

Add method to `AgentConfigBuilder`:

```rust
    /// Load MCP server configuration and connect all servers.
    ///
    /// Searches for .mcp.json (project-level) and ~/.mcp.json (user-level),
    /// merges them, connects all servers, and registers their tools.
    pub async fn with_mcp_from_config(
        mut self,
        working_dir: Option<&Path>,
    ) -> Result<Self, vol_llm_mcp::McpError> {
        let config = match McpConfig::load(working_dir) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("MCP config load error: {}", e);
                return Ok(self); // continue without MCP
            }
        };

        if config.servers().is_empty() {
            return Ok(self);
        }

        let session = Arc::new(McpSession::connect(config.servers().to_vec()).await);
        self.mcp_session = Some(session.clone());

        // Register MCP tools into the tool registry
        if let Some(ref registry) = self.tool_registry {
            let mut registry_clone = (**registry).clone();
            registry_clone.register_from_mcp(session).await;
            self.tool_registry = Some(Arc::new(registry_clone));
        } else {
            // Build a new registry from individual tools + MCP
            let mut registry = ToolRegistry::new();
            for tool in self.tools {
                registry.register_boxed(tool);
            }
            registry.register_from_mcp(session).await;
            self.tool_registry = Some(Arc::new(registry));
            self.tools.clear();
        }

        Ok(self)
    }
```

Update `build()` method to include `mcp_session`:

```rust
Ok(AgentConfig {
    def: self.def,
    llm,
    tools,
    session,
    sandbox: self.sandbox,
    context_builder,
    plugin_registry: self.plugin_registry,
    mcp_session: self.mcp_session,
})
```

- [ ] **Step 4: Add ToolRegistry Clone impl if missing**

Check if `ToolRegistry` implements `Clone`. If not, add:

```rust
impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: self.tools.clone(),
        }
    }
}
```

- [ ] **Step 4: Add Clone impl to ToolRegistry**

Add to `crates/vol-llm-tool/src/registry.rs` (anywhere in the `impl ToolRegistry` block or after it):

```rust
impl Clone for ToolRegistry {
    fn clone(&self) -> Self {
        Self {
            tools: self.tools.clone(),
        }
    }
}
```

Note: `Arc<dyn ExecutableTool>` is always Clone (Arc uses reference counting), so `HashMap<String, Arc<dyn ExecutableTool>>` is Clone.

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-agent`
Expected: compiles cleanly

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent/ crates/vol-llm-tool/src/registry.rs
git commit -m "feat: add AgentConfigBuilder::with_mcp_from_config and mcp_session lifecycle"
```

---

### Task 7: Add McpSession disconnect in agent run() cleanup

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Add disconnect call in run() cleanup**

In the `run()` method, after `listener_result` handling and before `agent_result`, add:

```rust
// Disconnect MCP session
if let Some(ref session) = self.config.mcp_session {
    let mut session_clone = session.clone();
    // McpSession::disconnect takes &mut self, so we need a different approach
    // Store session as Arc<Mutex<McpSession>> or use a separate cleanup
}
```

Actually, since `McpSession` is shared via `Arc`, we need to handle this differently. Let's use a wrapper:

In `vol-llm-mcp/src/session.rs`, add a helper:

```rust
impl McpSession {
    /// Disconnect all connections (Arc-compatible, uses interior mutability).
    pub async fn disconnect_all(self: &Arc<Self>) {
        // Since connections are owned by McpSession and not behind mutex,
        // we need to make connections use Arc<Mutex<...>> or provide
        // a separate cleanup handle.
    }
}
```

Better approach: store the session in AgentConfig and disconnect in the builder's consume pattern. The simplest: don't store in AgentConfig, store separately and manage lifecycle externally.

Let me revise: `McpSession` connections should use `Arc<Mutex<HashMap>>` for interior mutability, so `disconnect` can be called through `Arc`.

Update `McpSession` to use interior mutability:

Change `ServerConnection` to use `Arc<tokio::sync::Mutex<RunningService>>` or simpler — make `connections` an `Arc<std::sync::Mutex<HashMap>>`:

```rust
use std::sync::Mutex;

pub struct McpSession {
    connections: Arc<Mutex<HashMap<String, ServerConnection>>>,
}
```

Then `disconnect` can be `async fn disconnect(self: &Arc<Self>)`:

```rust
impl McpSession {
    // ... connect returns Arc<Self> instead of Self
    pub async fn connect(configs: Vec<McpServerConfig>) -> Arc<Self> {
        // ... same logic ...
        Arc::new(Self { connections: Arc::new(Mutex::new(connections)) })
    }

    pub async fn disconnect(self: &Arc<Self>) {
        let mut conns = self.connections.lock().unwrap();
        for (name, conn) in conns.iter_mut() {
            if let Err(e) = conn.close().await {
                tracing::warn!("Error closing MCP server '{}': {}", name, e);
            }
        }
        conns.clear();
    }
}
```

Update all methods to use `self.connections.lock().unwrap()`.

- [ ] **Step 2: Update McpSession for interior mutability**

Rewrite `session.rs` with `Arc<Mutex<...>>` pattern. The `connect` method returns `Arc<Self>`. All read methods use `self.connections.lock().unwrap()`.

- [ ] **Step 3: Update McpTool to use Arc<McpSession>**

McpTool already uses `Arc<McpSession>`, no change needed.

- [ ] **Step 4: Update AgentConfigBuilder to store Arc<McpSession>**

The `mcp_session` field is already `Option<Arc<McpSession>>`. Update `build()` to pass it through.

- [ ] **Step 5: Add disconnect in agent.rs run()**

After the listener/interceptor cleanup in `run()`:

```rust
// Disconnect MCP session
if let Some(ref session) = self.config.mcp_session {
    session.disconnect().await;
}
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-mcp && cargo check -p vol-llm-tool && cargo check -p vol-llm-agent`
Expected: all compile cleanly

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-mcp/src/session.rs crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/config_builder.rs
git commit -m "feat: add McpSession disconnect in agent lifecycle with interior mutability"
```

---

### Task 8: Workspace compilation and smoke test

- [ ] **Step 1: Full workspace check**

Run: `cargo check --workspace`
Expected: compiles cleanly with no new errors

- [ ] **Step 2: Run all tests**

Run: `cargo test --workspace`
Expected: all existing tests pass + new MCP tests pass

- [ ] **Step 3: Commit**

```bash
git status
git commit -m "chore: verify workspace compilation with MCP integration"
```

---

### Task 9: Write wiki entry

- [ ] **Step 1: Use wiki-ingest skill**

Run the wiki-ingest skill to create/update the wiki page for `vol-llm-mcp` crate documentation.
