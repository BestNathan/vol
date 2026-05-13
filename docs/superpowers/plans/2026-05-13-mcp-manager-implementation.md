# McpManager Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `McpSession` with `McpManager` — a connection lifecycle manager that tracks per-server state, auto-reconnects with exponential backoff, and supports the full MCP protocol (tools, resources, prompts).

**Architecture:** Single `McpManager` struct with `Arc<RwLock<>>` internal mutability. Per-server background reconnect tasks spawned within `connect()`, each owning a `CancellationToken`. Caches populated at connect time, cleared on disconnect, filtered by state for discovery.

**Tech Stack:** Rust, `rmcp 1.6` (features: `client`, `transport-io`, `transport-child-process`), `tokio`, `tokio-util`, `tracing`, `thiserror`

---

### Task 1: Extend McpError with new variants

**Files:**
- Modify: `crates/vol-llm-mcp/src/error.rs`

- [ ] **Step 1: Add new error variants**

Add these three variants to the existing `McpError` enum in `crates/vol-llm-mcp/src/error.rs`:

```rust
#[error("failed to read resource '{uri}' on server '{server}': {detail}")]
ResourceReadFailed { server: String, uri: String, detail: String },

#[error("failed to get prompt '{name}' on server '{server}': {detail}")]
PromptGetFailed { server: String, name: String, detail: String },

#[error("MCP server '{0}' is disconnected")]
ServerDisconnected(String),
```

Place them after `ToolCallFailed` and before `TransportError`. The final enum should have 9 variants.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-mcp`
Expected: PASS (no new code references these yet, just new variants added)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-mcp/src/error.rs
git commit -m "feat: add ResourceReadFailed, PromptGetFailed, ServerDisconnected error variants"
```

### Task 2: Create McpManager struct and connection logic

**Files:**
- Create: `crates/vol-llm-mcp/src/manager.rs`
- Read for reference: `crates/vol-llm-mcp/src/session.rs` (current `McpSession` for porting connection logic)
- Test: `crates/vol-llm-mcp/src/manager.rs` (inline tests)

- [ ] **Step 1: Write the module skeleton**

Create `crates/vol-llm-mcp/src/manager.rs` with:

```rust
//! McpManager — manages MCP server connection lifecycles.
//!
//! Tracks per-server connection state, spawns background reconnect tasks
//! on failure, and caches discovered capabilities at connect time.

use rmcp::model::{
    CallToolRequestParams, ClientInfo, GetPromptRequestParams, JsonObject,
    Prompt, PromptMessage, ReadResourceRequestParams, Resource,
    ResourceTemplate, Tool,
};
use rmcp::service::{Peer, RoleClient, RunningService, ServiceError, ServiceExt};
use rmcp::transport::TokioChildProcess;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing;

use crate::config::McpServerConfig;
use crate::error::McpError;
use crate::session::{sanitize_name, McpToolInfo};

/// Connection state of a single MCP server.
#[derive(Debug, Clone, PartialEq)]
pub enum ServerStatus {
    Connected,
    Disconnected, // Gracefully disconnected
    Connecting,   // Connection in progress
    Error(String), // Failure detail
}

/// Per-server connection state.
struct ServerState {
    config: McpServerConfig,
    status: ServerStatus,
    retry_count: usize,
    running_service: Option<RunningService<RoleClient, ClientInfo>>,
    cancel_token: CancellationToken,
    cached_tools: Vec<McpToolInfo>,
    cached_resources: Vec<Resource>,
    cached_resource_templates: Vec<ResourceTemplate>,
    cached_prompts: Vec<Prompt>,
    reconnect_handle: Option<tokio::task::JoinHandle<()>>,
}

/// MCP connection lifecycle manager.
pub struct McpManager {
    servers: Arc<RwLock<HashMap<String, ServerState>>>,
    max_retries: usize,
    backoff_min: Duration,
    backoff_max: Duration,
}

impl ServerState {
    fn new(config: McpServerConfig) -> Self {
        Self {
            config,
            status: ServerStatus::Disconnected,
            retry_count: 0,
            running_service: None,
            cancel_token: CancellationToken::new(),
            cached_tools: Vec::new(),
            cached_resources: Vec::new(),
            cached_resource_templates: Vec::new(),
            cached_prompts: Vec::new(),
            reconnect_handle: None,
        }
    }

    fn clear_caches(&mut self) {
        self.cached_tools.clear();
        self.cached_resources.clear();
        self.cached_resource_templates.clear();
        self.cached_prompts.clear();
    }
}
```

- [ ] **Step 2: Write test for empty config**

Add inline tests at the bottom of `manager.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_empty_config() {
        let mgr = McpManager::new(vec![]);
        mgr.connect().await.unwrap();
        assert!(mgr.list_all_tools().await.is_empty());
    }
}
```

- [ ] **Step 3: Implement constructor and builder**

```rust
impl McpManager {
    pub fn new(configs: Vec<McpServerConfig>) -> Self {
        let servers = configs
            .into_iter()
            .map(|c| (sanitize_name(&c.name), ServerState::new(c)))
            .collect();
        Self {
            servers: Arc::new(RwLock::new(servers)),
            max_retries: 5,
            backoff_min: Duration::from_secs(1),
            backoff_max: Duration::from_secs(30),
        }
    }

    pub fn with_max_retries(mut self, max: usize) -> Self {
        self.max_retries = max;
        self
    }

    pub fn with_backoff(mut self, min: Duration, max: Duration) -> Self {
        self.backoff_min = min;
        self.backoff_max = max;
        self
    }
```

- [ ] **Step 4: Run test to verify empty config passes**

Run: `cargo test -p vol-llm-mcp --lib manager::tests::test_empty_config`
Expected: FAIL — `connect` and `list_all_tools` not yet implemented.

- [ ] **Step 5: Implement connect_single helper**

```rust
    async fn connect_single(
        config: &McpServerConfig,
        cancel_token: &CancellationToken,
    ) -> Result<(RunningService<RoleClient, ClientInfo>, Vec<Tool>, Vec<Resource>, Vec<ResourceTemplate>, Vec<Prompt>), McpError> {
        let mut command = Command::new(&config.command);
        command.args(&config.args);
        for (key, value) in &config.env {
            command.env(key, value);
        }
        command.stdin(Stdio::piped());
        command.stdout(Stdio::piped());
        command.stderr(Stdio::inherit());

        let child = TokioChildProcess::new(command).map_err(|e: std::io::Error| {
            McpError::ConnectionFailed {
                server: config.name.clone(),
                detail: e.to_string(),
            }
        })?;

        let client_info = ClientInfo::default();
        let service = tokio::time::timeout(
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
        })?;

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

- [ ] **Step 6: Implement connect()**

```rust
    pub async fn connect(&self) -> Result<(), McpError> {
        let server_names: Vec<String>;
        {
            let servers = self.servers.read().await;
            server_names = servers.keys().cloned().collect();
        }

        for name in server_names {
            self.connect_server(&name).await;
        }

        Ok(())
    }

    async fn connect_server(&self, name: &str) {
        let config;
        let max_retries;
        let backoff_min;
        let backoff_max;
        {
            let mut servers = self.servers.write().await;
            let Some(state) = servers.get_mut(name) else { return };
            state.status = ServerStatus::Connecting;
            state.cancel_token = CancellationToken::new();
            config = state.config.clone();
            max_retries = self.max_retries;
            backoff_min = self.backoff_min;
            backoff_max = self.backoff_max;
        }

        match Self::connect_single(&config, &CancellationToken::new()).await {
            Ok((service, tools, resources, resource_templates, prompts)) => {
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.running_service = Some(service);
                    state.cached_tools = tools.iter().map(|t| McpToolInfo {
                        name: t.name.to_string(),
                        description: t.description.as_ref().map(|s| s.to_string()),
                        input_schema: Some(t.schema_as_json_value()),
                    }).collect();
                    state.cached_resources = resources;
                    state.cached_resource_templates = resource_templates;
                    state.cached_prompts = prompts;
                    state.status = ServerStatus::Connected;
                    state.retry_count = 0;
                    tracing::info!(server = name, "MCP server connected");
                }
            }
            Err(e) => {
                tracing::error!(server = name, error = %e, "MCP server connection failed");
                let mut servers = self.servers.write().await;
                if let Some(state) = servers.get_mut(name) {
                    state.retry_count += 1;
                    if state.retry_count >= max_retries {
                        state.clear_caches();
                        state.status = ServerStatus::Error("max retries exceeded".to_string());
                        tracing::error!(server = name, retries = state.retry_count, "MCP server max retries exceeded");
                    } else {
                        state.status = ServerStatus::Error(e.to_string());
                        self.spawn_reconnect(name, max_retries, backoff_min, backoff_max);
                    }
                }
            }
        }
    }
```

- [ ] **Step 7: Implement spawn_reconnect (background reconnect loop)**

```rust
    fn spawn_reconnect(
        &self,
        name: &str,
        max_retries: usize,
        backoff_min: Duration,
        backoff_max: Duration,
    ) {
        let name = name.to_string();
        let servers = self.servers.clone();
        let backoff_min = backoff_min;
        let backoff_max = backoff_max;

        let handle = tokio::spawn(async move {
            loop {
                // Read current state to get config and cancel_token
                let (config, cancel_token, current_retry) = {
                    let mut srv = servers.write().await;
                    let Some(state) = srv.get_mut(&name) else { break };
                    if state.retry_count >= max_retries {
                        state.clear_caches();
                        state.status = ServerStatus::Error("max retries exceeded".to_string());
                        break;
                    }
                    let config = state.config.clone();
                    cancel_token_for_reconnect(&mut *state, &backoff_min, &backoff_max).await;
                    (config, state.cancel_token.clone(), state.retry_count)
                };

                if cancel_token.is_cancelled() {
                    break;
                }

                // Wait backoff
                let delay = exponential_backoff(current_retry, backoff_min, backoff_max);
                tokio::select! {
                    _ = tokio::time::sleep(delay) => {}
                    _ = cancel_token.cancelled() => break,
                }

                // Attempt reconnect (with 10s timeout)
                match tokio::time::timeout(
                    Duration::from_secs(10),
                    McpManager::connect_single(&config, &cancel_token),
                )
                .await
                {
                    Ok(Ok((service, tools, resources, resource_templates, prompts))) => {
                    Ok((service, tools, resources, resource_templates, prompts)) => {
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.running_service = Some(service);
                            state.cached_tools = tools.iter().map(|t| McpToolInfo {
                                name: t.name.to_string(),
                                description: t.description.as_ref().map(|s| s.to_string()),
                                input_schema: Some(t.schema_as_json_value()),
                            }).collect();
                            state.cached_resources = resources;
                            state.cached_resource_templates = resource_templates;
                            state.cached_prompts = prompts;
                            state.status = ServerStatus::Connected;
                            state.retry_count = 0;
                            tracing::info!(server = name, "MCP server reconnected");
                        }
                        break; // Success, exit reconnect loop
                    }
                    Ok(Err(e)) => {
                        tracing::warn!(server = name, error = %e, "MCP reconnect failed");
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.retry_count += 1;
                            state.status = ServerStatus::Error(e.to_string());
                            if state.retry_count >= max_retries {
                                state.clear_caches();
                                state.status = ServerStatus::Error("max retries exceeded".to_string());
                                tracing::error!(server = name, retries = state.retry_count, "MCP server max retries exceeded");
                                break;
                            }
                        }
                        // Loop continues — next iteration will retry
                    }
                    Err(_) => {
                        // Timeout
                        tracing::warn!(server = name, "MCP reconnect timed out");
                        let mut srv = servers.write().await;
                        if let Some(state) = srv.get_mut(&name) {
                            state.retry_count += 1;
                            state.status = ServerStatus::Error("connection timeout".to_string());
                            if state.retry_count >= max_retries {
                                state.clear_caches();
                                state.status = ServerStatus::Error("max retries exceeded".to_string());
                                break;
                            }
                        }
                    }
                }
            }
        });

        // Store handle
        let name_clone = name.clone();
        let servers_clone = self.servers.clone();
        tokio::spawn(async move {
            let mut srv = servers_clone.write().await;
            if let Some(state) = srv.get_mut(&name_clone) {
                state.reconnect_handle = Some(handle);
            }
        });
    }
```

Also add the helper functions:

```rust
fn exponential_backoff(retry_count: usize, min: Duration, max: Duration) -> Duration {
    let delay = min.mul_f64(2f64.powi(retry_count as i32));
    delay.min(max)
}

async fn cancel_token_for_reconnect(
    state: &mut ServerState,
    backoff_min: &Duration,
    backoff_max: &Duration,
) {
    // Cancel any previous reconnect attempt
    state.cancel_token.cancel();
    state.cancel_token = CancellationToken::new();
}
```

- [ ] **Step 8: Implement list_all_tools()**

```rust
    pub async fn list_all_tools(&self) -> Vec<(String, McpToolInfo)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state.cached_tools.iter().map(|tool| {
                    (server.clone(), tool.clone())
                })
            })
            .collect()
    }
```

- [ ] **Step 9: Run test to verify it passes**

Run: `cargo test -p vol-llm-mcp --lib manager::tests::test_empty_config`
Expected: PASS

- [ ] **Step 10: Commit**

```bash
git add crates/vol-llm-mcp/src/manager.rs
git commit -m "feat: add McpManager struct with connect, reconnect, and list_all_tools"
```

### Task 3: Implement remaining McpManager protocol methods

**Files:**
- Modify: `crates/vol-llm-mcp/src/manager.rs`

- [ ] **Step 1: Implement call_tool()**

```rust
    pub async fn call_tool(
        &self,
        server: &str,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<String, McpError> {
        let (peer, server_name) = {
            let servers = self.servers.read().await;
            let state = servers
                .get(server)
                .ok_or_else(|| McpError::ServerNotFound(server.to_string()))?;

            if state.status != ServerStatus::Connected {
                return Err(McpError::ServerDisconnected(server.to_string()));
            }

            let service = state
                .running_service
                .as_ref()
                .ok_or_else(|| McpError::ServerDisconnected(server.to_string()))?;

            (service.peer().clone(), state.config.name.clone())
        };

        let arguments = match args {
            serde_json::Value::Object(obj) => Some(JsonObject::from_iter(obj)),
            _ => Some(JsonObject::new()),
        };

        let params = match arguments {
            Some(args) => {
                CallToolRequestParams::new(tool_name.to_string()).with_arguments(args)
            }
            None => CallToolRequestParams::new(tool_name.to_string()),
        };

        let result = peer.call_tool(params).await.map_err(|e: ServiceError| {
            McpError::ToolCallFailed {
                server: server_name.clone(),
                tool: tool_name.to_string(),
                detail: e.to_string(),
            }
        })?;

        Ok(Self::format_call_tool_result(&result))
    }

    fn format_call_tool_result(result: &rmcp::model::CallToolResult) -> String {
        use rmcp::model::{RawContent, ResourceContents};

        if result.is_error == Some(true) {
            let text = result
                .content
                .iter()
                .filter_map(|c| match &c.raw {
                    RawContent::Text(text_block) => Some(text_block.text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            return format!(
                "MCP tool error: {}",
                if text.is_empty() { "unknown error" } else { &text }
            );
        }

        result
            .content
            .iter()
            .filter_map(|c| match &c.raw {
                RawContent::Text(text_block) => Some(text_block.text.clone()),
                RawContent::Image(_) => Some("[image content]".to_string()),
                RawContent::Resource(resource_block) => {
                    let uri = match &resource_block.resource {
                        ResourceContents::TextResourceContents { uri, .. } => uri,
                        ResourceContents::BlobResourceContents { uri, .. } => uri,
                    };
                    Some(format!("[resource: {}]", uri))
                }
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
```

- [ ] **Step 2: Implement resource protocol methods**

```rust
    pub async fn list_all_resources(&self) -> Vec<(String, Resource)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state.cached_resources.iter().map(|r| (server.clone(), r.clone()))
            })
            .collect()
    }

    pub async fn list_all_resource_templates(&self) -> Vec<(String, ResourceTemplate)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state.cached_resource_templates.iter().map(|t| (server.clone(), t.clone()))
            })
            .collect()
    }

    pub async fn read_resource(&self, uri: &str) -> Result<String, McpError> {
        // Find which server owns this resource
        let (peer, server_name) = {
            let servers = self.servers.read().await;
            for (server, state) in servers.iter() {
                if state.status != ServerStatus::Connected {
                    continue;
                }
                if state.cached_resources.iter().any(|r| r.uri == uri)
                    || state.cached_resource_templates.iter().any(|t| {
                        // Simple prefix match for templates
                        uri.starts_with(&t.uri_template)
                    })
                {
                    if let Some(service) = &state.running_service {
                        return Ok(Self::read_resource_from_peer(
                            service.peer(),
                            &state.config.name,
                            uri,
                        ).await?);
                    }
                }
            }
            return Err(McpError::ServerNotFound(uri.to_string()));
        };

        Self::read_resource_from_peer(&peer, &server_name, uri).await
    }

    async fn read_resource_from_peer(
        peer: &Peer<RoleClient>,
        server_name: &str,
        uri: &str,
    ) -> Result<String, McpError> {
        let params = ReadResourceRequestParams { uri: uri.to_string() };
        let result = peer.read_resource(params).await.map_err(|e: ServiceError| {
            McpError::ResourceReadFailed {
                server: server_name.to_string(),
                uri: uri.to_string(),
                detail: e.to_string(),
            }
        })?;

        // Flatten result contents into text
        let texts: Vec<String> = result
            .contents
            .into_iter()
            .filter_map(|c| match c {
                rmcp::model::AnyResourceContent::Text(t) => Some(t.text),
                rmcp::model::AnyResourceContent::Blob(b) => {
                    Some(base64_encode(&b.blob))
                }
            })
            .collect();

        Ok(texts.join("\n"))
    }
```

- [ ] **Step 3: Implement prompt protocol methods**

```rust
    pub async fn list_all_prompts(&self) -> Vec<(String, Prompt)> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .filter(|(_, state)| state.status == ServerStatus::Connected)
            .flat_map(|(server, state)| {
                state.cached_prompts.iter().map(|p| (server.clone(), p.clone()))
            })
            .collect()
    }

    pub async fn get_prompt(
        &self,
        name: &str,
        args: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<(Option<String>, Vec<PromptMessage>), McpError> {
        // Find which server owns this prompt
        let (peer, server_name) = {
            let servers = self.servers.read().await;
            for (server, state) in servers.iter() {
                if state.status != ServerStatus::Connected {
                    continue;
                }
                if state.cached_prompts.iter().any(|p| p.name == name) {
                    if let Some(service) = &state.running_service {
                        let peer = service.peer().clone();
                        return Ok(Self::get_prompt_from_peer(
                            &peer,
                            &state.config.name,
                            name,
                            args,
                        ).await?);
                    }
                }
            }
            return Err(McpError::ServerNotFound(name.to_string()));
        };

        Self::get_prompt_from_peer(&peer, &server_name, name, args).await
    }

    async fn get_prompt_from_peer(
        peer: &Peer<RoleClient>,
        server_name: &str,
        name: &str,
        args: Option<HashMap<String, serde_json::Value>>,
    ) -> Result<(Option<String>, Vec<PromptMessage>), McpError> {
        let arguments = args
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(k, v)| match v {
                serde_json::Value::String(s) => Some((k, s)),
                _ => Some((k, v.to_string())),
            })
            .collect();

        let params = GetPromptRequestParams {
            name: name.to_string(),
            arguments,
        };

        let result = peer.get_prompt(params).await.map_err(|e: ServiceError| {
            McpError::PromptGetFailed {
                server: server_name.to_string(),
                name: name.to_string(),
                detail: e.to_string(),
            }
        })?;

        Ok((result.description, result.messages))
    }

    pub async fn complete_prompt(
        &self,
        _name: &str,
        _argument_name: &str,
        _value: &str,
    ) -> Result<rmcp::model::CompleteResult, McpError> {
        // Note: rmcp's Peer<RoleClient> has .complete() method
        // Implementation depends on finding the owning server
        // Placeholder — wire up when rmcp API is confirmed
        unimplemented!("complete_prompt not yet wired")
    }
```

- [ ] **Step 4: Implement disconnect methods**

```rust
    pub async fn disconnect(&self) -> Result<(), McpError> {
        let server_names: Vec<String>;
        {
            let servers = self.servers.read().await;
            server_names = servers.keys().cloned().collect();
        }

        for name in server_names {
            self.disconnect_server(&name).await?;
        }

        Ok(())
    }

    pub async fn disconnect_server(&self, name: &str) -> Result<(), McpError> {
        let mut servers = self.servers.write().await;
        let state = servers
            .get_mut(name)
            .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;

        // Cancel reconnect
        state.cancel_token.cancel();

        // Close running service
        if let Some(mut service) = state.running_service.take() {
            if let Err(e) = service.close().await {
                tracing::warn!("Error closing MCP server '{}': {}", name, e);
            }
        }

        state.clear_caches();
        state.status = ServerStatus::Disconnected;

        tracing::info!(server = name, "MCP server disconnected");
        Ok(())
    }
```

- [ ] **Step 5: Implement server_status() and reconnect()**

```rust
    pub fn server_status(&self) -> HashMap<String, ServerStatus> {
        // Note: This is sync — we clone the statuses under read lock
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                let servers = self.servers.read().await;
                servers
                    .iter()
                    .map(|(name, state)| (name.clone(), state.status.clone()))
                    .collect()
            })
        })
    }

    pub async fn reconnect(&self, name: &str) -> Result<(), McpError> {
        // Cancel any in-progress reconnect
        {
            let mut servers = self.servers.write().await;
            let state = servers
                .get_mut(name)
                .ok_or_else(|| McpError::ServerNotFound(name.to_string()))?;

            state.cancel_token.cancel();

            // If currently connected, disconnect first
            if state.status == ServerStatus::Connected {
                if let Some(mut service) = state.running_service.take() {
                    let _ = service.close().await;
                }
                state.clear_caches();
            }

            state.retry_count = 0;
        }

        // Run fresh connect
        self.connect_server(name).await;

        // Check result
        let servers = self.servers.read().await;
        let state = servers.get(name).unwrap();
        match &state.status {
            ServerStatus::Connected => Ok(()),
            ServerStatus::Error(e) => Err(McpError::ConnectionFailed {
                server: name.to_string(),
                detail: e.clone(),
            }),
            _ => Err(McpError::ConnectionFailed {
                server: name.to_string(),
                detail: "reconnect did not complete".to_string(),
            }),
        }
    }
```

Note: `server_status()` needs a simpler sync approach. Replace with:

```rust
    pub async fn server_status_async(&self) -> HashMap<String, ServerStatus> {
        let servers = self.servers.read().await;
        servers
            .iter()
            .map(|(name, state)| (name.clone(), state.status.clone()))
            .collect()
    }
```

And keep a sync version using `try_read`:

```rust
    pub fn server_status(&self) -> HashMap<String, ServerStatus> {
        if let Ok(guard) = self.servers.try_read() {
            guard
                .iter()
                .map(|(name, state)| (name.clone(), state.status.clone()))
                .collect()
        } else {
            HashMap::new()
        }
    }
```

- [ ] **Step 6: Verify compilation**

Run: `cargo check -p vol-llm-mcp`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-mcp/src/manager.rs
git commit -m "feat: implement full MCP protocol on McpManager (tools, resources, prompts)"
```

### Task 4: Update lib.rs to export McpManager, remove McpSession

**Files:**
- Modify: `crates/vol-llm-mcp/src/lib.rs`

- [ ] **Step 1: Update lib.rs**

Replace the entire contents of `crates/vol-llm-mcp/src/lib.rs` with:

```rust
//! vol-llm-mcp: MCP Client protocol layer for ReAct Agent.
//!
//! Provides configuration parsing, connection lifecycle management,
//! and tool/resource/prompt discovery for MCP servers configured
//! via ~/.mcp.json and .mcp.json.

pub mod config;
pub mod error;
pub mod manager;
pub mod session; // Kept for sanitize_name and McpToolInfo re-export

pub use config::McpConfig;
pub use error::McpError;
pub use manager::{McpManager, ServerStatus};
pub use session::McpToolInfo;
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-mcp`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-mcp/src/lib.rs
git commit -m "refactor: export McpManager and ServerStatus from vol-llm-mcp"
```

### Task 5: Update McpTool to use McpManager

**Files:**
- Modify: `crates/vol-llm-tool/src/mcp_tool.rs`
- Test: `crates/vol-llm-tool/src/mcp_tool.rs` (existing behavior)

- [ ] **Step 1: Update McpTool to reference McpManager**

Replace the entire contents of `crates/vol-llm-tool/src/mcp_tool.rs` with:

```rust
//! McpTool — bridges MCP tools into the ExecutableTool trait.

use std::sync::Arc;

use async_trait::async_trait;
use vol_llm_mcp::McpManager;

use crate::tool::{ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity};

/// A tool that proxies execution to an MCP server via McpManager.
pub struct McpTool {
    manager: Arc<McpManager>,
    server_name: String,
    tool_name: String,
    display_name: &'static str,
    description: &'static str,
    parameters: serde_json::Value,
}

impl McpTool {
    /// Create a new McpTool from a manager and tool info.
    pub fn new(
        manager: Arc<McpManager>,
        server_name: &str,
        tool_name: &str,
        description: &str,
        parameters: serde_json::Value,
    ) -> Self {
        let sanitized = vol_llm_mcp::session::sanitize_name(server_name);
        let sanitized_tool = vol_llm_mcp::session::sanitize_name(tool_name);
        let display_name = format!("mcp__{}_{}", sanitized, sanitized_tool);

        // Leak strings to satisfy ExecutableTool::name() -> &'static str
        let display_name: &'static str = Box::leak(display_name.into_boxed_str());
        let description: &'static str = Box::leak(description.to_string().into_boxed_str());

        Self {
            manager,
            server_name: sanitized,
            tool_name: sanitized_tool,
            display_name,
            description,
            parameters,
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
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let result = self
            .manager
            .call_tool(&self.server_name, &self.tool_name, args.clone())
            .await;

        match result {
            Ok(content) => Ok(ToolResult::success(content)),
            Err(e) => Err(ToolError::ExecutionFailed(e.to_string())),
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-tool`
Expected: FAIL — `registry.rs` still references `McpSession`.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-tool/src/mcp_tool.rs
git commit -m "refactor: McpTool now holds Arc<McpManager> instead of Arc<McpSession>"
```

### Task 6: Update ToolRegistry::register_from_mcp to use McpManager

**Files:**
- Modify: `crates/vol-llm-tool/src/registry.rs`
- Test: `crates/vol-llm-tool/src/registry.rs` (existing tests)

- [ ] **Step 1: Update register_from_mcp signature and implementation**

In `crates/vol-llm-tool/src/registry.rs`, replace the `register_from_mcp` method:

```rust
    /// Discover and register all MCP tools from an McpManager.
    ///
    /// Queries the manager for tools from all connected servers,
    /// creates McpTool wrappers, and registers them.
    /// Returns the number of tools registered.
    pub async fn register_from_mcp(&mut self, manager: Arc<vol_llm_mcp::McpManager>) -> usize {
        use crate::mcp_tool::McpTool;

        let tools = manager.list_all_tools().await;
        let mut count = 0;
        for (server, tool_info) in tools {
            let description = tool_info.description.as_deref().unwrap_or_else(|| {
                &tool_info.name
            });
            let mcp_tool = McpTool::new(
                manager.clone(),
                &server,
                &tool_info.name,
                description,
                tool_info.input_schema.unwrap_or_else(|| {
                    serde_json::json!({ "type": "object", "properties": {} })
                }),
            );
            self.register_boxed(Box::new(mcp_tool));
            count += 1;
        }
        count
    }
```

- [ ] **Step 2: Update the existing test**

In `crates/vol-llm-tool/src/registry.rs`, find `test_register_from_mcp_empty_session` and replace with:

```rust
    #[tokio::test]
    async fn test_register_from_mcp_empty_manager() {
        use vol_llm_mcp::McpManager;

        let manager = Arc::new(McpManager::new(vec![]));
        manager.connect().await.unwrap();
        let mut registry = ToolRegistry::new();
        let count = registry.register_from_mcp(manager).await;
        assert_eq!(count, 0);
        assert!(registry.tool_names().is_empty());
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-tool`
Expected: PASS

- [ ] **Step 4: Run registry tests**

Run: `cargo test -p vol-llm-tool registry::`
Expected: ALL PASS (5 tests)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tool/src/registry.rs
git commit -m "refactor: register_from_mcp accepts Arc<McpManager>"
```

### Task 7: Update AgentConfig and AgentConfigBuilder to use McpManager

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:37` (mcp_session field)
- Modify: `crates/vol-llm-agent/src/react/agent.rs:75` (Default impl)
- Modify: `crates/vol-llm-agent/src/react/agent.rs:608-613` (disconnect in run())
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs:14` (import)
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs:27` (builder field)
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs:42` (builder init)
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs:107-146` (with_mcp_from_config)
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs:203` (build method)

- [ ] **Step 1: Update agent.rs — rename mcp_session to mcp_manager**

In `crates/vol-llm-agent/src/react/agent.rs`:

Change line 13:
```rust
use vol_llm_mcp::McpManager;
```

Change line 37:
```rust
    pub mcp_manager: Option<Arc<McpManager>>,
```

Change line 60 (in `AgentConfig::new`):
```rust
            mcp_manager: None,
```

Change line 75 (in `Default` impl):
```rust
            mcp_manager: None,
```

Change lines 608-613 (disconnect in `run()`):
```rust
        // Disconnect MCP manager
        if let Some(ref mcp_manager) = config.mcp_manager {
            mcp_manager.disconnect().await.ok();
        }
```

- [ ] **Step 2: Update config_builder.rs**

Change line 14:
```rust
use vol_llm_mcp::{McpConfig, McpManager};
```

Change line 27:
```rust
    mcp_manager: Option<Arc<McpManager>>,
```

Change line 42:
```rust
            mcp_manager: None,
```

Replace `with_mcp_from_config` (lines 107-146):

```rust
    pub async fn with_mcp_from_config(mut self, working_dir: Option<&Path>) -> Self {
        let config = match McpConfig::load(working_dir) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("MCP config load error: {}", e);
                return self;
            }
        };

        if config.servers().is_empty() {
            return self;
        }

        let manager = Arc::new(McpManager::new(config.servers().to_vec()));
        if let Err(e) = manager.connect().await {
            tracing::warn!("MCP manager connect error: {}", e);
            return self;
        }

        // Register MCP tools into the tool registry
        let tool_registry = match self.tool_registry.take() {
            Some(registry) => {
                let mut reg = match Arc::try_unwrap(registry) {
                    Ok(r) => r,
                    Err(arc) => (*arc).clone(),
                };
                reg.register_from_mcp(manager.clone()).await;
                Arc::new(reg)
            }
            None => {
                let mut registry = ToolRegistry::new();
                let tools = std::mem::take(&mut self.tools);
                for tool in tools {
                    registry.register_boxed(tool);
                }
                registry.register_from_mcp(manager.clone()).await;
                Arc::new(registry)
            }
        };
        self.tool_registry = Some(tool_registry);
        self.mcp_manager = Some(manager);

        self
    }
```

Change line 203 (in `build()`):
```rust
            mcp_manager: self.mcp_manager,
```

- [ ] **Step 3: Verify compilation of vol-llm-agent**

Run: `cargo check -p vol-llm-agent`
Expected: PASS

- [ ] **Step 4: Run agent tests**

Run: `cargo test -p vol-llm-agent`
Expected: ALL PASS (4 tests in agent.rs + 5 in config_builder.rs)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/config_builder.rs
git commit -m "refactor: AgentConfig uses McpManager instead of McpSession"
```

### Task 8: Add connection state and reconnect tests to manager.rs

**Files:**
- Modify: `crates/vol-llm-mcp/src/manager.rs` (inline tests)

- [ ] **Step 1: Add max retry exhaustion test**

Add to the `tests` module in `manager.rs`:

```rust
    #[tokio::test]
    async fn test_max_retry_exhaustion() {
        // Use a command that doesn't exist — every connect attempt will fail
        let config = McpServerConfig {
            name: "failing-server".to_string(),
            command: "nonexistent-command-that-will-fail".to_string(),
            args: vec![],
            env: HashMap::new(),
        };

        let mgr = McpManager::new(vec![config])
            .with_max_retries(2)
            .with_backoff(Duration::from_millis(10), Duration::from_millis(50));

        mgr.connect().await;

        // Allow some time for background reconnect attempts
        tokio::time::sleep(Duration::from_millis(300)).await;

        let status = mgr.server_status_async().await;
        let failing_status = status.get("failing-server").expect("server should exist");

        // After 2 retries + initial attempt = 3 total attempts, should be in Error state
        assert!(
            matches!(failing_status, ServerStatus::Error(msg) if msg.contains("max retries")),
            "expected max retries error, got: {:?}",
            failing_status
        );

        // No tools should be available
        let tools = mgr.list_all_tools().await;
        assert!(tools.is_empty());
    }
```

- [ ] **Step 2: Add manual reconnect after exhaustion test**

```rust
    #[tokio::test]
    async fn test_manual_reconnect_after_exhaustion() {
        let config = McpServerConfig {
            name: "failing-server".to_string(),
            command: "nonexistent-command".to_string(),
            args: vec![],
            env: HashMap::new(),
        };

        let mgr = McpManager::new(vec![config])
            .with_max_retries(1)
            .with_backoff(Duration::from_millis(10), Duration::from_millis(50));

        mgr.connect().await;
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Should be in Error state after exhausting retries
        let status = mgr.server_status_async().await;
        assert!(matches!(status.get("failing-server"), Some(ServerStatus::Error(_))));

        // Manual reconnect should retry (and fail again, since command is invalid)
        let result = mgr.reconnect("failing-server").await;
        assert!(result.is_err()); // Still fails, but retry counter was reset
    }
```

- [ ] **Step 3: Add disconnected tools excluded test**

```rust
    #[tokio::test]
    async fn test_disconnected_tools_excluded() {
        // Empty config — no tools
        let mgr = McpManager::new(vec![]);
        mgr.connect().await.unwrap();
        assert!(mgr.list_all_tools().await.is_empty());

        // Now test with a config that will fail
        let config = McpServerConfig {
            name: "bad-server".to_string(),
            command: "does-not-exist".to_string(),
            args: vec![],
            env: HashMap::new(),
        };

        let mgr = McpManager::new(vec![config])
            .with_max_retries(0) // No retries — fail immediately
            .with_backoff(Duration::from_millis(10), Duration::from_millis(50));

        mgr.connect().await;
        let tools = mgr.list_all_tools().await;
        assert!(tools.is_empty(), "disconnected server tools should be excluded");
    }
```

- [ ] **Step 4: Run all manager tests**

Run: `cargo test -p vol-llm-mcp --lib manager`
Expected: ALL PASS (4 tests: empty_config, max_retry_exhaustion, manual_reconnect_after_exhaustion, disconnected_tools_excluded)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-mcp/src/manager.rs
git commit -m "test: add connection state and reconnect tests"
```

### Task 9: Update and run all vol-llm-mcp tests, verify example still works

**Files:**
- Read for reference: `crates/vol-llm-agents/examples/docs_rs_mcp_example.rs`
- Test: `crates/vol-llm-mcp/src/config.rs` (existing tests)
- Test: `crates/vol-llm-mcp/src/session.rs` (existing sanitize_name tests)

- [ ] **Step 1: Run all vol-llm-mcp tests**

Run: `cargo test -p vol-llm-mcp`
Expected: ALL PASS (existing config tests + session sanitize_name tests + new manager tests)

If any existing tests fail due to McpSession references, update them:

For `config.rs` tests — these should still pass (config parsing is unchanged).

For `session.rs` tests — sanitize_name tests should still pass (session.rs is still present).

- [ ] **Step 2: Verify the docs-rs MCP example still compiles**

Run: `cargo check --example docs_rs_mcp_example -p vol-llm-agents`
Expected: PASS

Note: The example uses `with_mcp_from_config()` which now internally creates `McpManager`. The example code itself needs no changes — the API surface is identical.

- [ ] **Step 3: Run all vol-llm-tool tests**

Run: `cargo test -p vol-llm-tool`
Expected: ALL PASS (5 registry tests)

- [ ] **Step 4: Run all vol-llm-agent tests**

Run: `cargo test -p vol-llm-agent`
Expected: ALL PASS (agent tests + config_builder tests)

- [ ] **Step 5: Commit**

```bash
git status
# If any additional files were modified:
git add <files>
git commit -m "chore: verify all tests pass and example compiles with McpManager"
```
