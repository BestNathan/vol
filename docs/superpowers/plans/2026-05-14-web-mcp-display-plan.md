# MCP Web UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a new "MCP" tab to the Dioxus web UI that displays and interacts with all MCP-related data (servers, tools, resources, prompts) via JSON-RPC.

**Architecture:** New `mcp.*` JSON-RPC methods are added to the backend (`vol-llm-agent-channel`). A shared `Arc<McpManager>` from `vol-llm-mcp` is passed through `JsonRpcServer` to each `JsonRpcConnection`. The frontend adds a new `McpPanel` component with 4 sub-tabs.

**Tech Stack:** Rust, axum, tokio, Dioxus 0.6, rmcp 1.6, serde_json, WebSocket JSON-RPC

---

### Task 1: Add MCP request types to JSON-RPC serde helpers

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs`

- [ ] **Step 1: Add MCP variants to JsonRpcRequest enum**

Add these variants to the `JsonRpcRequest` enum (after `SessionEntries`):

```rust
// ... existing variants ...
SessionEntries {
    id: u64,
    session_id: String,
},
McpListServers { id: u64 },
McpListTools { id: u64, server: Option<String> },
McpCallTool { id: u64, server: String, tool_name: String, arguments: serde_json::Value },
McpListResources { id: u64, server: Option<String> },
McpListResourceTemplates { id: u64, server: Option<String> },
McpReadResource { id: u64, uri: String },
McpListPrompts { id: u64, server: Option<String> },
McpGetPrompt { id: u64, name: String, arguments: Option<std::collections::HashMap<String, serde_json::Value>> },
McpReconnect { id: u64, server: String },
McpServerStatus { id: u64 },
/// Fallback for unknown/unrecognized methods.
Unknown {
    id: Option<u64>,
    method: String,
},
```

- [ ] **Step 2: Add match arms in parse_jsonrpc_request**

Add these match arms in the `parse_jsonrpc_request` function (before the `_ =>` catch-all):

```rust
"mcp.list_servers" => Ok(JsonRpcRequest::McpListServers { id }),
"mcp.list_tools" => {
    let server = params.get("server").and_then(|v| v.as_str()).map(|s| s.to_string());
    Ok(JsonRpcRequest::McpListTools { id, server })
}
"mcp.call_tool" => {
    let server = params.get("server").and_then(|v| v.as_str()).ok_or_else(|| "mcp.call_tool: missing 'server'".to_string())?.to_string();
    let tool_name = params.get("tool_name").and_then(|v| v.as_str()).ok_or_else(|| "mcp.call_tool: missing 'tool_name'".to_string())?.to_string();
    let arguments = params.get("arguments").cloned().unwrap_or(serde_json::json!({}));
    Ok(JsonRpcRequest::McpCallTool { id, server, tool_name, arguments })
}
"mcp.list_resources" => {
    let server = params.get("server").and_then(|v| v.as_str()).map(|s| s.to_string());
    Ok(JsonRpcRequest::McpListResources { id, server })
}
"mcp.list_resource_templates" => {
    let server = params.get("server").and_then(|v| v.as_str()).map(|s| s.to_string());
    Ok(JsonRpcRequest::McpListResourceTemplates { id, server })
}
"mcp.read_resource" => {
    let uri = params.get("uri").and_then(|v| v.as_str()).ok_or_else(|| "mcp.read_resource: missing 'uri'".to_string())?.to_string();
    Ok(JsonRpcRequest::McpReadResource { id, uri })
}
"mcp.list_prompts" => {
    let server = params.get("server").and_then(|v| v.as_str()).map(|s| s.to_string());
    Ok(JsonRpcRequest::McpListPrompts { id, server })
}
"mcp.get_prompt" => {
    let name = params.get("name").and_then(|v| v.as_str()).ok_or_else(|| "mcp.get_prompt: missing 'name'".to_string())?.to_string();
    let arguments = params.get("arguments").and_then(|v| serde_json::from_value(v.clone()).ok());
    Ok(JsonRpcRequest::McpGetPrompt { id, name, arguments })
}
"mcp.reconnect" => {
    let server = params.get("server").and_then(|v| v.as_str()).ok_or_else(|| "mcp.reconnect: missing 'server'".to_string())?.to_string();
    Ok(JsonRpcRequest::McpReconnect { id, server })
}
"mcp.server_status" => Ok(JsonRpcRequest::McpServerStatus { id }),
```

- [ ] **Step 3: Verify the build compiles**

Run: `cargo check -p vol-llm-agent-channel`
Expected: Compiles successfully (the new enum variants are not yet matched in connection.rs, so there may be non-exhaustive pattern warnings — that's fine for now, we handle it in Task 3).

### Task 2: Add vol-llm-mcp dependency to agent-channel

**Files:**
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`

- [ ] **Step 1: Add vol-llm-mcp dependency**

Add to the `[dependencies]` section:

```toml
vol-llm-mcp = { workspace = true }
```

- [ ] **Step 2: Verify build**

Run: `cargo check -p vol-llm-agent-channel`
Expected: Compiles successfully.

### Task 3: Add McpManager to JsonRpcServer and JsonRpcConnection

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/server.rs`
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

- [ ] **Step 1: Add McpManager field to JsonRpcServer**

In `server.rs`, add to `JsonRpcServer` struct:

```rust
use std::sync::Arc;
use vol_llm_mcp::manager::McpManager;
```

Add field to struct:

```rust
pub struct JsonRpcServer {
    router: AgentRouter,
    dispatchers: HashMap<String, Arc<AgentDispatcher>>,
    holders: HashMap<String, Arc<ConnectionHolder>>,
    working_dir: String,
    store_dir: String,
    mcp_manager: Option<Arc<McpManager>>,
}
```

Update `new()` to accept the parameter:

```rust
pub async fn new(
    agents: Vec<AgentRegistration>,
    working_dir: String,
    store_dir: String,
    mcp_manager: Option<Arc<McpManager>>,
) -> Self {
    // ... existing code ...
    Self { router, dispatchers, holders, working_dir, store_dir, mcp_manager }
}
```

- [ ] **Step 2: Pass McpManager into handle_ws**

In `into_axum_router`, pass the clone to `handle_ws`:

```rust
async fn handle_ws(socket: WebSocket, server: Arc<JsonRpcServer>) {
    let session_store = Arc::new(vol_session::FileSessionEntryStore::new(&server.store_dir));
    let conn = JsonRpcConnection::new(
        socket,
        server.router.clone(),
        server.dispatchers.clone(),
        server.holders.clone(),
        server.working_dir.clone(),
        server.store_dir.clone(),
        session_store,
        server.mcp_manager.clone(),
    );
    let conn_arc = Arc::new(conn);
    conn_arc.run().await;
}
```

- [ ] **Step 3: Add McpManager field to JsonRpcConnection**

In `connection.rs`, add import:

```rust
use vol_llm_mcp::manager::McpManager;
```

Add field to struct:

```rust
mcp_manager: Option<Arc<McpManager>>,
```

Update `new()` signature and body:

```rust
pub fn new(
    ws: WebSocket,
    router: AgentRouter,
    dispatchers: HashMap<String, Arc<AgentDispatcher>>,
    holders: HashMap<String, Arc<ConnectionHolder>>,
    working_dir: String,
    store_dir: String,
    session_store: Arc<vol_session::FileSessionEntryStore>,
    mcp_manager: Option<Arc<McpManager>>,
) -> Self {
    // ... existing fields ...
    Self {
        // ... existing fields ...
        mcp_manager,
    }
}
```

- [ ] **Step 4: Wire new request variants in handle_text_frame**

In `handle_text_frame`, add match arms before `JsonRpcRequest::Unknown`:

```rust
JsonRpcRequest::McpListServers { id } => {
    self.handle_mcp_list_servers(*id).await
}
JsonRpcRequest::McpListTools { id, server } => {
    self.handle_mcp_list_tools(*id, server.clone()).await
}
JsonRpcRequest::McpCallTool { id, server, tool_name, arguments } => {
    self.handle_mcp_call_tool(*id, server.clone(), tool_name.clone(), arguments.clone()).await
}
JsonRpcRequest::McpListResources { id, server } => {
    self.handle_mcp_list_resources(*id, server.clone()).await
}
JsonRpcRequest::McpListResourceTemplates { id, server } => {
    self.handle_mcp_list_resource_templates(*id, server.clone()).await
}
JsonRpcRequest::McpReadResource { id, uri } => {
    self.handle_mcp_read_resource(*id, uri.clone()).await
}
JsonRpcRequest::McpListPrompts { id, server } => {
    self.handle_mcp_list_prompts(*id, server.clone()).await
}
JsonRpcRequest::McpGetPrompt { id, name, arguments } => {
    self.handle_mcp_get_prompt(*id, name.clone(), arguments.clone()).await
}
JsonRpcRequest::McpReconnect { id, server } => {
    self.handle_mcp_reconnect(*id, server.clone()).await
}
JsonRpcRequest::McpServerStatus { id } => {
    self.handle_mcp_server_status(*id).await
}
```

- [ ] **Step 5: Verify build**

Run: `cargo check -p vol-llm-agent-channel`
Expected: Compiles with errors about missing handler methods — that's expected, we add them in Task 4.

### Task 4: Implement MCP handler methods in JsonRpcConnection

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

Add these imports at the top:

```rust
use vol_llm_mcp::manager::ServerStatus;
```

Add a helper method for status string conversion:

```rust
fn server_status_to_str(status: &ServerStatus) -> String {
    match status {
        ServerStatus::Connected => "connected".into(),
        ServerStatus::Disconnected => "disconnected".into(),
        ServerStatus::Connecting => "connecting".into(),
        ServerStatus::Error(e) => format!("error: {}", e),
    }
}
```

- [ ] **Step 1: Add mcp_manager guard helper**

Add this helper method to `JsonRpcConnection`:

```rust
fn mcp_manager(&self) -> Result<Arc<McpManager>, String> {
    self.mcp_manager.clone().ok_or_else(|| "MCP not configured".to_string())
}
```

- [ ] **Step 2: Implement handle_mcp_list_servers**

```rust
async fn handle_mcp_list_servers(&self, id: u64) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    let status = mgr.server_status_async().await;
    let servers: Vec<serde_json::Value> = status.iter().map(|(name, s)| {
        serde_json::json!({
            "name": name,
            "status": server_status_to_str(s),
        })
    }).collect();
    to_jsonrpc_response(id, serde_json::json!({ "servers": servers }))
}
```

- [ ] **Step 3: Implement handle_mcp_server_status**

```rust
async fn handle_mcp_server_status(&self, id: u64) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    let status = mgr.server_status_async().await;
    let servers: Vec<serde_json::Value> = status.iter().map(|(name, s)| {
        serde_json::json!({
            "name": name,
            "status": server_status_to_str(s),
        })
    }).collect();
    to_jsonrpc_response(id, serde_json::json!({ "servers": servers }))
}
```

- [ ] **Step 4: Implement handle_mcp_list_tools**

```rust
async fn handle_mcp_list_tools(&self, id: u64, server_filter: Option<String>) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    let tools = mgr.list_all_tools().await;
    let tools_json: Vec<serde_json::Value> = tools.iter()
        .filter(|(s, _)| server_filter.as_ref().map_or(true, |f| s == f))
        .map(|(server, tool)| {
            serde_json::json!({
                "server": server,
                "name": tool.name,
                "description": tool.description,
                "input_schema": tool.input_schema,
            })
        }).collect();
    to_jsonrpc_response(id, serde_json::json!({ "tools": tools_json }))
}
```

- [ ] **Step 5: Implement handle_mcp_call_tool**

```rust
async fn handle_mcp_call_tool(&self, id: u64, server: String, tool_name: String, arguments: serde_json::Value) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    match mgr.call_tool(&server, &tool_name, arguments).await {
        Ok(result) => to_jsonrpc_response(id, serde_json::json!({ "result": result })),
        Err(e) => to_jsonrpc_error(Some(id), -32000, e.to_string()),
    }
}
```

- [ ] **Step 6: Implement handle_mcp_list_resources**

```rust
async fn handle_mcp_list_resources(&self, id: u64, server_filter: Option<String>) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    let resources = mgr.list_all_resources().await;
    let resources_json: Vec<serde_json::Value> = resources.iter()
        .filter(|(s, _)| server_filter.as_ref().map_or(true, |f| s == f))
        .map(|(server, r)| {
            serde_json::json!({
                "server": server,
                "name": r.name,
                "uri": r.uri,
                "mime_type": r.mime_type,
                "description": r.description,
            })
        }).collect();
    to_jsonrpc_response(id, serde_json::json!({ "resources": resources_json }))
}
```

- [ ] **Step 7: Implement handle_mcp_list_resource_templates**

```rust
async fn handle_mcp_list_resource_templates(&self, id: u64, server_filter: Option<String>) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    let templates = mgr.list_all_resource_templates().await;
    let templates_json: Vec<serde_json::Value> = templates.iter()
        .filter(|(s, _)| server_filter.as_ref().map_or(true, |f| s == f))
        .map(|(server, t)| {
            serde_json::json!({
                "server": server,
                "name": t.name,
                "uri_template": t.uri_template,
                "description": t.description,
            })
        }).collect();
    to_jsonrpc_response(id, serde_json::json!({ "templates": templates_json }))
}
```

- [ ] **Step 8: Implement handle_mcp_read_resource**

```rust
async fn handle_mcp_read_resource(&self, id: u64, uri: String) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    match mgr.read_resource(&uri).await {
        Ok(content) => to_jsonrpc_response(id, serde_json::json!({ "content": content })),
        Err(e) => to_jsonrpc_error(Some(id), -32000, e.to_string()),
    }
}
```

- [ ] **Step 9: Implement handle_mcp_list_prompts**

```rust
async fn handle_mcp_list_prompts(&self, id: u64, server_filter: Option<String>) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    let prompts = mgr.list_all_prompts().await;
    let prompts_json: Vec<serde_json::Value> = prompts.iter()
        .filter(|(s, _)| server_filter.as_ref().map_or(true, |f| s == f))
        .map(|(server, p)| {
            let args = p.arguments.as_ref().map(|args| {
                args.iter().map(|a| {
                    serde_json::json!({
                        "name": a.name,
                        "description": a.description,
                        "required": a.required,
                    })
                }).collect::<Vec<_>>()
            });
            serde_json::json!({
                "server": server,
                "name": p.name,
                "description": p.description,
                "arguments": args,
            })
        }).collect();
    to_jsonrpc_response(id, serde_json::json!({ "prompts": prompts_json }))
}
```

- [ ] **Step 10: Implement handle_mcp_get_prompt**

```rust
async fn handle_mcp_get_prompt(&self, id: u64, name: String, arguments: Option<std::collections::HashMap<String, serde_json::Value>>) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    match mgr.get_prompt(&name, arguments).await {
        Ok((desc, messages)) => {
            let msgs = messages.iter().map(|m| {
                serde_json::json!({
                    "role": m.role,
                    "content": m.content,
                })
            }).collect::<Vec<_>>();
            to_jsonrpc_response(id, serde_json::json!({
                "description": desc,
                "messages": msgs,
            }))
        }
        Err(e) => to_jsonrpc_error(Some(id), -32000, e.to_string()),
    }
}
```

- [ ] **Step 11: Implement handle_mcp_reconnect**

```rust
async fn handle_mcp_reconnect(&self, id: u64, server: String) -> String {
    let mgr = match self.mcp_manager() {
        Ok(m) => m,
        Err(e) => return to_jsonrpc_error(Some(id), -32000, e),
    };
    match mgr.reconnect(&server).await {
        Ok(()) => {
            let status = mgr.server_status_async().await;
            let status_str = status.get(&server)
                .map(|s| server_status_to_str(s))
                .unwrap_or_else(|| "unknown".into());
            to_jsonrpc_response(id, serde_json::json!({ "success": true, "status": status_str }))
        }
        Err(e) => to_jsonrpc_response(id, serde_json::json!({ "success": false, "status": format!("error: {}", e) })),
    }
}
```

- [ ] **Step 12: Verify build**

Run: `cargo check -p vol-llm-agent-channel`
Expected: Compiles successfully.

### Task 5: Update jsonrpc_agent_service example to create McpManager

**Files:**
- Modify: `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`

- [ ] **Step 1: Add imports**

Add at the top:

```rust
use vol_llm_mcp::{McpConfig, McpManager};
use std::sync::Arc;
```

- [ ] **Step 2: Create and connect McpManager before server creation**

After the `// Create dispatcher` section and before `// Wrap holder in Arc`, add:

```rust
    // Create MCP manager and connect
    let mcp_manager = {
        let configs = McpConfig::load(Some(std::path::Path::new(".")))
            .map(|c| c.servers().to_vec())
            .unwrap_or_else(|e| {
                tracing::warn!("Failed to load MCP config: {}", e);
                vec![]
            });
        tracing::info!("Loaded {} MCP server configurations", configs.len());
        let manager = McpManager::new(configs);
        let manager_for_connect = manager.clone();
        tokio::spawn(async move {
            manager_for_connect.connect().await;
        });
        Arc::new(manager)
    };
```

- [ ] **Step 3: Pass McpManager to JsonRpcServer::new**

Change the server creation to pass `Some(mcp_manager)`:

```rust
    let server = JsonRpcServer::new(
        vec![AgentRegistration {
            agent_id: "general-assistant".to_string(),
            dispatcher,
            holder,
        }],
        ".".to_string(),
        "/tmp/vol-llm-store".to_string(),
        Some(mcp_manager),
    ).await;
```

- [ ] **Step 4: Update log message**

Add to the tracing info block:

```rust
    tracing::info!("           mcp.* (list_servers, list_tools, call_tool, etc.)");
```

- [ ] **Step 5: Verify build**

Run: `cargo check -p vol-llm-agent-channel --examples`
Expected: Compiles successfully.

### Task 6: Add MCP state types to vol-llm-ui

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add MCP wire types**

Add these types after the existing `AgentListEntry` struct (around line 486):

```rust
/// MCP server info returned by mcp.list_servers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerInfo {
    pub name: String,
    pub status: String,
}

/// MCP tool info returned by mcp.list_tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolInfo {
    pub server: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

/// MCP resource info returned by mcp.list_resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceInfo {
    pub server: String,
    pub name: String,
    pub uri: String,
    pub mime_type: Option<String>,
    pub description: Option<String>,
}

/// MCP resource template info returned by mcp.list_resource_templates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpResourceTemplateInfo {
    pub server: String,
    pub name: String,
    pub uri_template: String,
    pub description: Option<String>,
}

/// MCP prompt info returned by mcp.list_prompts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptInfo {
    pub server: String,
    pub name: String,
    pub description: Option<String>,
    pub arguments: Option<Vec<McpPromptArgInfo>>,
}

/// MCP prompt argument definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpPromptArgInfo {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}
```

- [ ] **Step 2: Add McpSubtab enum and ActiveTab::Mcp**

Add `Mcp` to the `ActiveTab` enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab { Conversation, Sessions, Tools, Workspace, Skills, Mcp, Logs, Agents }
```

Update the `toggle()` method:

```rust
impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Sessions,
            ActiveTab::Sessions => ActiveTab::Tools,
            ActiveTab::Tools => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Mcp,
            ActiveTab::Mcp => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Agents,
            ActiveTab::Agents => ActiveTab::Conversation,
        }
    }
}
```

- [ ] **Step 3: Add McpSubtab enum**

Add after `ActiveTab`:

```rust
/// Sub-tabs within the MCP panel.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum McpSubtab { Servers, Tools, Resources, Prompts }
```

- [ ] **Step 4: Add McpState struct for web**

Add after the existing `SessionsState` impl (around line 527):

```rust
/// Local state for McpPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct McpState {
    pub servers: Vec<McpServerInfo>,
    pub tools: Vec<McpToolInfo>,
    pub resources: Vec<McpResourceInfo>,
    pub resource_templates: Vec<McpResourceTemplateInfo>,
    pub prompts: Vec<McpPromptInfo>,
    pub loading: bool,
    pub error: Option<String>,
    pub active_subtab: McpSubtab,
    pub tool_call_dialog: Option<McpToolCallState>,
    pub resource_viewer: Option<McpResourceViewerState>,
    pub prompt_viewer: Option<McpPromptViewerState>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl McpState {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(), tools: Vec::new(), resources: Vec::new(),
            resource_templates: Vec::new(), prompts: Vec::new(),
            loading: true, error: None, active_subtab: McpSubtab::Servers,
            tool_call_dialog: None, resource_viewer: None, prompt_viewer: None,
        }
    }
}

/// State for the tool call dialog.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct McpToolCallState {
    pub server: String,
    pub tool_name: String,
    pub arguments_json: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

/// State for the resource viewer.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct McpResourceViewerState {
    pub uri: String,
    pub content: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

/// State for the prompt viewer.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct McpPromptViewerState {
    pub server: String,
    pub prompt_name: String,
    pub args_json: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}
```

- [ ] **Step 5: Verify build**

Run: `cargo check -p vol-llm-ui --features web --no-default-features`
Expected: Compiles successfully.

### Task 7: Add MCP JSON-RPC client methods to vol-llm-ui

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add MCP import types**

Add at the top, after existing imports:

```rust
use crate::state::{McpPromptInfo, McpResourceInfo, McpResourceTemplateInfo, McpServerInfo, McpToolInfo};
```

- [ ] **Step 2: Add mcp_list_servers method**

Add to `impl JsonRpcClient`:

```rust
pub fn mcp_list_servers(&self, cb: impl FnOnce(Result<Vec<McpServerInfo>, String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.list_servers",
        "params": {},
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        match result.get("servers").and_then(|v| v.as_array()) {
            Some(servers) => {
                let parsed: Vec<McpServerInfo> = servers.iter()
                    .filter_map(|s| serde_json::from_value(s.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no servers in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 3: Add mcp_list_tools method**

```rust
pub fn mcp_list_tools(&self, server: Option<&str>, cb: impl FnOnce(Result<Vec<McpToolInfo>, String>) + 'static) {
    let id = self.alloc_id();
    let mut params = serde_json::Map::new();
    if let Some(s) = server { params.insert("server".to_string(), serde_json::json!(s)); }
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.list_tools",
        "params": params,
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        match result.get("tools").and_then(|v| v.as_array()) {
            Some(tools) => {
                let parsed: Vec<McpToolInfo> = tools.iter()
                    .filter_map(|t| serde_json::from_value(t.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no tools in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 4: Add mcp_call_tool method**

```rust
pub fn mcp_call_tool(&self, server: &str, tool_name: &str, arguments: serde_json::Value, cb: impl FnOnce(Result<String, String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.call_tool",
        "params": { "server": server, "tool_name": tool_name, "arguments": arguments },
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        if let Some(error) = result.get("error") {
            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            cb(Err(msg.to_string()));
        } else if let Some(content) = result.get("result").and_then(|v| v.as_str()) {
            cb(Ok(content.to_string()));
        } else {
            cb(Err("no result in response".to_string()));
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 5: Add mcp_list_resources method**

```rust
pub fn mcp_list_resources(&self, server: Option<&str>, cb: impl FnOnce(Result<Vec<McpResourceInfo>, String>) + 'static) {
    let id = self.alloc_id();
    let mut params = serde_json::Map::new();
    if let Some(s) = server { params.insert("server".to_string(), serde_json::json!(s)); }
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.list_resources",
        "params": params,
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        match result.get("resources").and_then(|v| v.as_array()) {
            Some(resources) => {
                let parsed: Vec<McpResourceInfo> = resources.iter()
                    .filter_map(|r| serde_json::from_value(r.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no resources in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 6: Add mcp_list_resource_templates method**

```rust
pub fn mcp_list_resource_templates(&self, server: Option<&str>, cb: impl FnOnce(Result<Vec<McpResourceTemplateInfo>, String>) + 'static) {
    let id = self.alloc_id();
    let mut params = serde_json::Map::new();
    if let Some(s) = server { params.insert("server".to_string(), serde_json::json!(s)); }
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.list_resource_templates",
        "params": params,
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        match result.get("templates").and_then(|v| v.as_array()) {
            Some(templates) => {
                let parsed: Vec<McpResourceTemplateInfo> = templates.iter()
                    .filter_map(|t| serde_json::from_value(t.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no templates in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 7: Add mcp_read_resource method**

```rust
pub fn mcp_read_resource(&self, uri: &str, cb: impl FnOnce(Result<String, String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.read_resource",
        "params": { "uri": uri },
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        if let Some(error) = result.get("error") {
            let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
            cb(Err(msg.to_string()));
        } else if let Some(content) = result.get("content").and_then(|v| v.as_str()) {
            cb(Ok(content.to_string()));
        } else {
            cb(Err("no content in response".to_string()));
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 8: Add mcp_list_prompts method**

```rust
pub fn mcp_list_prompts(&self, server: Option<&str>, cb: impl FnOnce(Result<Vec<McpPromptInfo>, String>) + 'static) {
    let id = self.alloc_id();
    let mut params = serde_json::Map::new();
    if let Some(s) = server { params.insert("server".to_string(), serde_json::json!(s)); }
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.list_prompts",
        "params": params,
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        match result.get("prompts").and_then(|v| v.as_array()) {
            Some(prompts) => {
                let parsed: Vec<McpPromptInfo> = prompts.iter()
                    .filter_map(|p| serde_json::from_value(p.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no prompts in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 9: Add mcp_reconnect method**

```rust
pub fn mcp_reconnect(&self, server: &str, cb: impl FnOnce(Result<bool, String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "mcp.reconnect",
        "params": { "server": server },
        "id": id,
    });
    let json = match serde_json::to_string(&msg) {
        Ok(j) => j,
        Err(e) => { cb(Err(e.to_string())); return; }
    };
    if let Err(e) = self.send_raw(&json) {
        cb(Err(format!("send failed: {e:?}")));
        return;
    }
    let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
        let success = result.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
        cb(Ok(success));
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 10: Verify build**

Run: `cargo check -p vol-llm-ui --features web --no-default-features`
Expected: Compiles successfully.

### Task 8: Create McpPanel component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

- [ ] **Step 1: Register the module**

In `mod.rs`, add:

```rust
pub mod mcp_panel;
```

And add to exports:

```rust
pub use mcp_panel::McpPanel;
```

- [ ] **Step 2: Create the McpPanel component**

Create `mcp_panel.rs` with the following structure. This is a large file — write it completely:

```rust
//! MCP Panel — displays servers, tools, resources, and prompts.

use dioxus::prelude::*;

use crate::state::{McpState, McpSubtab};
use crate::web::components::app::AppState;

#[component]
pub fn McpPanel() -> Element {
    let app_state: AppState = use_context();
    let signal = use_signal(|| McpState::new());

    // Load data on mount
    use_hook(move || {
        let client = app_state.rpc_client.clone();
        let sig = signal.clone();
        async move {
            let s1 = sig.clone();
            client.mcp_list_servers(move |result| {
                match result {
                    Ok(servers) => s1.write_unchecked().servers = servers,
                    Err(e) => s1.write_unchecked().error = Some(e),
                }
                s1.write_unchecked().loading = false;
            });

            let s2 = sig.clone();
            client.mcp_list_tools(None, move |result| {
                if let Ok(tools) = result { s2.write_unchecked().tools = tools; }
            });

            let s3 = sig.clone();
            client.mcp_list_resources(None, move |result| {
                if let Ok(resources) = result { s3.write_unchecked().resources = resources; }
            });

            let s4 = sig.clone();
            client.mcp_list_resource_templates(None, move |result| {
                if let Ok(templates) = result { s4.write_unchecked().resource_templates = templates; }
            });

            let s5 = sig.clone();
            client.mcp_list_prompts(None, move |result| {
                if let Ok(prompts) = result { s5.write_unchecked().prompts = prompts; }
            });
        }
    });

    let s = signal.read();
    let active = s.active_subtab;
    let loading = s.loading;
    drop(s);

    rsx! {
        div { class: "flex-1 overflow-y-auto p-2",
            if loading {
                div { class: "text-[#666] text-center p-4 text-[13px]", "Loading MCP data..." }
            } else {
                div {
                    // Sub-tab buttons
                    div { class: "flex gap-1 mb-2",
                        McpSubtabButton { signal, subtab: McpSubtab::Servers, label: "Servers" }
                        McpSubtabButton { signal, subtab: McpSubtab::Tools, label: "Tools" }
                        McpSubtabButton { signal, subtab: McpSubtab::Resources, label: "Resources" }
                        McpSubtabButton { signal, subtab: McpSubtab::Prompts, label: "Prompts" }
                    }
                    // Sub-tab content
                    match active {
                        McpSubtab::Servers => rsx! { ServerList { signal, app_state } },
                        McpSubtab::Tools => rsx! { ToolList { signal } },
                        McpSubtab::Resources => rsx! { ResourceList { signal } },
                        McpSubtab::Prompts => rsx! { PromptList { signal } },
                    }
                }
            }
        }
    }
}

#[component]
fn McpSubtabButton(signal: Signal<McpState>, subtab: McpSubtab, label: String) -> Element {
    let active = signal.read().active_subtab == subtab;
    let class = if active {
        "px-3 py-1 bg-[#1a1a2e] text-[#e0e0e0] rounded text-[12px] cursor-pointer border border-[#80a0ff]"
    } else {
        "px-3 py-1 bg-transparent text-[#888] rounded text-[12px] cursor-pointer hover:text-[#ccc] hover:bg-[#2a2a44]"
    };
    let mut sig = signal;
    rsx! {
        button {
            class,
            onclick: move |_| { sig.write_unchecked().active_subtab = subtab; },
            "{label}"
        }
    }
}

#[component]
fn ServerList(signal: Signal<McpState>, app_state: AppState) -> Element {
    let servers = signal.read().servers.clone();
    let error = signal.read().error.clone();
    drop(signal);

    if servers.is_empty() && error.is_none() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No MCP servers configured" }
        };
    }

    rsx! {
        div { class: "font-mono text-[13px]",
            {servers.into_iter().map(|s| {
                let signal = signal.clone();
                let app = app_state.clone();
                rsx! { ServerRow { signal, server: s, app_state: app } }
            }).collect::<Vec<Element>>().into_iter()}
            if let Some(ref e) = error {
                div { class: "text-[#c04040] p-2 text-[12px]", "{e}" }
            }
        }
    }
}

#[component]
fn ServerRow(signal: Signal<McpState>, app_state: AppState, server: crate::state::McpServerInfo) -> Element {
    let status_color = match server.status.as_str() {
        "connected" => "#40c040",
        "connecting" => "#f0c040",
        "disconnected" => "#888",
        _ => "#c04040",
    };
    let show_reconnect = server.status != "connected" && server.status != "connecting";

    rsx! {
        div { class: "flex items-center justify-between py-1.5 border-b border-[#2a2a44]",
            div { class: "flex items-center gap-2",
                span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: {status_color};" }
                span { class: "text-[13px] text-[#e0e0e0]", "{server.name}" }
                span { class: "text-[11px] text-[#666]", "{server.status}" }
            }
            if show_reconnect {
                button {
                    class: "px-2 py-0.5 bg-[#2a2a44] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                    onclick: move |_| {
                        let srv = server.name.clone();
                        let client = app_state.rpc_client.clone();
                        let sig = signal.clone();
                        client.mcp_reconnect(&srv, move |result| {
                            if let Ok(true) = result {
                                let sig2 = sig.clone();
                                client.mcp_list_servers(move |r| {
                                    if let Ok(servers) = r {
                                        let mut s = sig2.write_unchecked();
                                        s.servers = servers;
                                        s.error = None;
                                    }
                                });
                                let sig2 = sig.clone();
                                client.mcp_list_tools(None, move |r| {
                                    if let Ok(tools) = r { sig2.write_unchecked().tools = tools; }
                                });
                            }
                        });
                    },
                    "Reconnect"
                }
            }
        }
    }
}

#[component]
fn ToolList(signal: Signal<McpState>) -> Element {
    let tools = signal.read().tools.clone();
    if tools.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No tools available" }
        };
    }

    // Group by server
    let mut groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for t in &tools {
        groups.entry(t.server.clone()).or_default().push(t.clone());
    }

    rsx! {
        div { class: "font-mono text-[13px]",
            {groups.into_iter().map(|(server, tools)| {
                rsx! {
                    div { class: "mb-2",
                        div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({tools.len()} tools)" }
                        {tools.into_iter().map(|t| {
                            let signal = signal.clone();
                            rsx! { ToolCard { signal, tool: t } }
                        }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

#[component]
fn ToolCard(signal: Signal<McpState>, tool: crate::state::McpToolInfo) -> Element {
    rsx! {
        div { class: "bg-[#252540] rounded p-2 mb-1",
            div { class: "flex items-center justify-between",
                div {
                    div { class: "text-[13px] text-[#e0e0e0]", "{tool.name}" }
                    if let Some(ref desc) = tool.description {
                        div { class: "text-[11px] text-[#888]", "{desc}" }
                    }
                }
                button {
                    class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                    onclick: move |_| {
                        let t = tool.clone();
                        let mut sig = signal;
                        sig.write_unchecked().tool_call_dialog = Some(crate::state::McpToolCallState {
                            server: t.server.clone(),
                            tool_name: t.name.clone(),
                            arguments_json: "{}".to_string(),
                            result: None,
                            error: None,
                            loading: false,
                        });
                    },
                    "Call"
                }
            }
        }
    }
}

#[component]
fn ResourceList(signal: Signal<McpState>) -> Element {
    let resources = signal.read().resources.clone();
    if resources.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No resources available" }
        };
    }

    let mut groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for r in &resources {
        groups.entry(r.server.clone()).or_default().push(r.clone());
    }

    rsx! {
        div { class: "font-mono text-[13px]",
            {groups.into_iter().map(|(server, resources)| {
                rsx! {
                    div { class: "mb-2",
                        div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({resources.len()} resources)" }
                        {resources.into_iter().map(|r| {
                            let signal = signal.clone();
                            rsx! { ResourceRow { signal, resource: r } }
                        }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

#[component]
fn ResourceRow(signal: Signal<McpState>, resource: crate::state::McpResourceInfo) -> Element {
    rsx! {
        div { class: "flex items-center justify-between py-1 border-b border-[#2a2a44]",
            div { class: "flex-1 min-w-0",
                div { class: "text-[13px] text-[#e0e0e0] truncate", "{resource.name}" }
                div { class: "text-[11px] text-[#666] truncate", "{resource.uri}" }
            }
            button {
                class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0] ml-2 flex-shrink-0",
                onclick: move |_| {
                    let r = resource.clone();
                    let mut sig = signal;
                    sig.write_unchecked().resource_viewer = Some(crate::state::McpResourceViewerState {
                        uri: r.uri.clone(),
                        content: None,
                        error: None,
                        loading: false,
                    });
                },
                "Read"
            }
        }
    }
}

#[component]
fn PromptList(signal: Signal<McpState>) -> Element {
    let prompts = signal.read().prompts.clone();
    if prompts.is_empty() {
        return rsx! {
            div { class: "text-[#666] text-center p-4 text-[13px]", "No prompts available" }
        };
    }

    let mut groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
    for p in &prompts {
        groups.entry(p.server.clone()).or_default().push(p.clone());
    }

    rsx! {
        div { class: "font-mono text-[13px]",
            {groups.into_iter().map(|(server, prompts)| {
                rsx! {
                    div { class: "mb-2",
                        div { class: "text-[12px] text-[#888] font-semibold mb-1", "{server} ({prompts.len()} prompts)" }
                        {prompts.into_iter().map(|p| {
                            let signal = signal.clone();
                            rsx! { PromptRow { signal, prompt: p } }
                        }).collect::<Vec<Element>>().into_iter()}
                    }
                }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

#[component]
fn PromptRow(signal: Signal<McpState>, prompt: crate::state::McpPromptInfo) -> Element {
    rsx! {
        div { class: "flex items-center justify-between py-1 border-b border-[#2a2a44]",
            div {
                div { class: "text-[13px] text-[#e0e0e0]", "{prompt.name}" }
                if let Some(ref desc) = prompt.description {
                    div { class: "text-[11px] text-[#888]", "{desc}" }
                }
            }
            button {
                class: "px-2 py-0.5 bg-[#3a3a55] text-[#aaa] rounded text-[11px] cursor-pointer hover:text-[#e0e0e0]",
                onclick: move |_| {
                    let p = prompt.clone();
                    let mut sig = signal;
                    sig.write_unchecked().prompt_viewer = Some(crate::state::McpPromptViewerState {
                        server: p.server.clone(),
                        prompt_name: p.name.clone(),
                        args_json: "{}".to_string(),
                        result: None,
                        error: None,
                        loading: false,
                    });
                },
                "Get"
            }
        }
    }
}
```

- [ ] **Step 3: Verify build**

Run: `cargo check -p vol-llm-ui --features web --no-default-features`
Expected: Compiles successfully.

### Task 9: Wire McpPanel into the tab system

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Add McpPanel import**

Add to imports:

```rust
use super::mcp_panel::McpPanel;
```

- [ ] **Step 2: Add MCP tab button in TabBar**

Add after the Skills TabButton:

```rust
TabButton { state: state.clone(), tab: ActiveTab::Mcp, label: "MCP" }
```

So the full TabBar becomes:

```rust
fn TabBar() -> Element {
    let state: AppState = use_context();
    rsx! {
        div { class: "flex bg-[#252540] border-b border-[#333355] flex-shrink-0 sm:overflow-x-auto",
            TabButton { state: state.clone(), tab: ActiveTab::Conversation, label: "Conversation" }
            TabButton { state: state.clone(), tab: ActiveTab::Sessions, label: "Sessions" }
            TabButton { state: state.clone(), tab: ActiveTab::Tools, label: "Tools" }
            TabButton { state: state.clone(), tab: ActiveTab::Workspace, label: "Workspace" }
            TabButton { state: state.clone(), tab: ActiveTab::Skills, label: "Skills" }
            TabButton { state: state.clone(), tab: ActiveTab::Mcp, label: "MCP" }
            TabButton { state: state.clone(), tab: ActiveTab::Logs, label: "Logs" }
            TabButton { state: state.clone(), tab: ActiveTab::Agents, label: "Agents" }
        }
    }
}
```

- [ ] **Step 3: Add MCP case in TabContent**

Add in the match:

```rust
ActiveTab::Mcp => rsx! { McpPanel {} },
```

So the full TabContent becomes:

```rust
fn TabContent() -> Element {
    let state: AppState = use_context();
    let active = *state.active_tab.read();
    match active {
        ActiveTab::Conversation => rsx! { ConversationView {} },
        ActiveTab::Tools => rsx! { ToolsTabContent {} },
        ActiveTab::Workspace => rsx! { FileContentView {} },
        ActiveTab::Skills => rsx! { SkillsPanel {} },
        ActiveTab::Mcp => rsx! { McpPanel {} },
        ActiveTab::Logs => rsx! { LogViewer {} },
        ActiveTab::Agents => rsx! { AgentsPanel {} },
        ActiveTab::Sessions => rsx! { SessionsPanel {} },
    }
}
```

- [ ] **Step 4: Verify build**

Run: `cargo check -p vol-llm-ui --features web --no-default-features`
Expected: Compiles successfully.

### Task 10: Full build verification and commit

- [ ] **Step 1: Full workspace build**

Run: `cargo check --workspace --features web --no-default-features` (for vol-llm-ui) and `cargo check -p vol-llm-agent-channel --examples`
Expected: All compiles.

- [ ] **Step 2: Commit all changes**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs
git add crates/vol-llm-agent-channel/src/jsonrpc/server.rs
git add crates/vol-llm-agent-channel/src/jsonrpc/connection.rs
git add crates/vol-llm-agent-channel/Cargo.toml
git add crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs
git add crates/vol-llm-ui/src/state/mod.rs
git add crates/vol-llm-ui/src/web/client.rs
git add crates/vol-llm-ui/src/web/components/mcp_panel.rs
git add crates/vol-llm-ui/src/web/components/mod.rs
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: add MCP tab to web UI with server/tool/resource/prompt display and interaction"
```
