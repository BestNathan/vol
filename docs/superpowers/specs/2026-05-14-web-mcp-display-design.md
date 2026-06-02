# Design Spec: MCP Web UI Support

## Architecture Overview

Add a new "MCP" tab to the Dioxus web UI that displays and interacts with all MCP-related data (servers, tools, resources, prompts). The web UI communicates via JSON-RPC over WebSocket — new `mcp.*` methods are added to the backend. The `McpManager` from `vol-llm-mcp` is shared across all WebSocket connections as an `Arc`.

```
Browser (Dioxus)                    Backend (axum + tokio)
┌──────────────────────┐            ┌──────────────────────────┐
│  McpPanel            │            │  JsonRpcServer           │
│  ├── Servers subtab  │◄─JSON-RPC─►│  ├── Arc<McpManager>─────┤
│  ├── Tools subtab    │  WebSocket │  │  ├── Server A: Connected│
│  ├── Resources subtab│            │  │  ├── Server B: Error   │
│  └── Prompts subtab  │            │  │  └── Server C: Connecting│
│                      │            │  ├── AgentDispatcher       │
│  JsonRpcClient       │            │  └── ConnectionHolder      │
│  mcp_list_servers()──┼───────────►│                          │
│  mcp_call_tool()─────┼───────────►│  McpManager (shared)     │
└──────────────────────┘            └──────────────────────────┘
```

## Backend Changes

### 1. Shared McpManager in JsonRpcServer

**File: `vol-llm-agent-channel/src/jsonrpc/server.rs`**

- `JsonRpcServer::new` accepts an `Option<Arc<McpManager>>` parameter
- Store as field on `JsonRpcServer`
- Pass `Arc<McpManager>` clone into `handle_ws` → `JsonRpcConnection`

**File: `vol-llm-agent-channel/src/jsonrpc/connection.rs`**

- Add `mcp_manager: Option<Arc<McpManager>>` field to `JsonRpcConnection`
- Add 10 handler methods for the new `mcp.*` methods
- Wire new request variants in `handle_text_frame` match

**File: `vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs`**

- Add 10 variants to `JsonRpcRequest` enum:
  - `McpListServers { id }`
  - `McpListTools { id, server: Option<String> }`
  - `McpCallTool { id, server, tool_name, arguments }`
  - `McpListResources { id, server: Option<String> }`
  - `McpListResourceTemplates { id, server: Option<String> }`
  - `McpReadResource { id, uri }`
  - `McpListPrompts { id, server: Option<String> }`
  - `McpGetPrompt { id, name, arguments: Option<HashMap<String, Value>> }`
  - `McpReconnect { id, server }`
  - `McpServerStatus { id }`
- Add corresponding match arms in `parse_jsonrpc_request`

### 2. Example service update

**File: `vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`**

- Load `McpConfig::load(Some(working_dir))`
- Create `McpManager::new(configs)`
- Pass `Arc::new(manager)` to `JsonRpcServer::new`
- Spawn `manager.connect()` in background tokio task

### 3. New MCP data types for wire format

New types needed for serialization (in `vol-llm-agent-channel` or a new shared module):

```rust
#[derive(Serialize, Deserialize)]
struct McpServerWire { name: String, status: String, command: String }
#[derive(Serialize, Deserialize)]
struct McpToolWire { server: String, name: String, description: Option<String>, input_schema: Option<Value> }
#[derive(Serialize, Deserialize)]
struct McpResourceWire { server: String, name: String, uri: String, mime_type: Option<String>, description: Option<String> }
#[derive(Serialize, Deserialize)]
struct McpResourceTemplateWire { server: String, name: String, uri_template: String, description: Option<String> }
#[derive(Serialize, Deserialize)]
struct McpPromptWire { server: String, name: String, description: Option<String>, arguments: Option<Vec<McpPromptArgWire>> }
#[derive(Serialize, Deserialize)]
struct McpPromptArgWire { name: String, description: Option<String>, required: bool }
```

### 4. Method response formats

| Method | Result JSON |
|--------|------------|
| `mcp.list_servers` | `{ servers: [{name, status, command}] }` |
| `mcp.list_tools` | `{ tools: [{server, name, description, input_schema}] }` |
| `mcp.call_tool` | `{ result: string }` |
| `mcp.list_resources` | `{ resources: [{server, name, uri, mime_type, description}] }` |
| `mcp.list_resource_templates` | `{ templates: [{server, name, uri_template, description}] }` |
| `mcp.read_resource` | `{ content: string }` |
| `mcp.list_prompts` | `{ prompts: [{server, name, description, arguments}] }` |
| `mcp.get_prompt` | `{ description: Option<string>, messages: [...] }` |
| `mcp.reconnect` | `{ success: bool, status: string }` |
| `mcp.server_status` | `{ servers: [{name, status}] }` |

### 5. ServerStatus mapping

The `ServerStatus` enum (`Connected`, `Disconnected`, `Connecting`, `Error(String)`) maps to string labels for the wire: `"connected"`, `"disconnected"`, `"connecting"`, `"error"`.

## Frontend Changes

### 1. New state types

**File: `vol-llm-ui/src/state/mod.rs`**

- Add `McpServerInfo`, `McpToolInfo`, `McpResourceInfo`, `McpResourceTemplateInfo`, `McpPromptInfo` derive structs (Serialize + Deserialize)
- Add `McpSubtab` enum: `Servers`, `Tools`, `Resources`, `Prompts`
- Add `ActiveTab::Mcp` variant (between `Skills` and `Logs` in toggle cycle)
- Add `McpState` struct with local state for the panel
- Update `ActiveTab::toggle()` to include MCP in cycle

### 2. JSON-RPC client methods

**File: `vol-llm-ui/src/web/client.rs`**

Add methods following the existing pattern (`alloc_id`, `send_raw`, callback in `pending`):

- `mcp_list_servers(cb: FnOnce(Result<Vec<McpServerInfo>, String>))`
- `mcp_list_tools(cb: FnOnce(Result<Vec<McpToolInfo>, String>))`
- `mcp_call_tool(server, tool_name, args, cb: FnOnce(Result<String, String>))`
- `mcp_list_resources(cb: FnOnce(Result<Vec<McpResourceInfo>, String>))`
- `mcp_list_resource_templates(cb: FnOnce(Result<Vec<McpResourceTemplateInfo>, String>))`
- `mcp_read_resource(uri, cb: FnOnce(Result<String, String>))`
- `mcp_list_prompts(cb: FnOnce(Result<Vec<McpPromptInfo>, String>))`
- `mcp_get_prompt(name, args, cb)`
- `mcp_reconnect(server, cb)`
- `mcp_server_status(cb)`

### 3. McpPanel component

**New file: `vol-llm-ui/src/web/components/mcp_panel.rs`**

Uses `Signal<McpState>` for local state. On mount, calls `mcp_list_servers` and `mcp_list_tools` to populate initial data.

Sub-tab layout (inline tabs within the panel):

```
┌─ MCP ─────────────────────────────────────────┐
│ [Servers] [Tools] [Resources] [Prompts]      │
├───────────────────────────────────────────────┤
│                                               │
│  Servers view:                                │
│  ┌─────────────────────────────────────────┐  │
│  │ ● docs-rs       connected               │  │
│  │ ● github        connected               │  │
│  │ ● failing-srv   error: max retries...   │  │
│  │                   [Reconnect]           │  │
│  └─────────────────────────────────────────┘  │
│                                               │
│  Tools view (grouped by server):              │
│  ▼ docs-rs (3 tools)                          │
│    ┌─────────────────────────────────────┐    │
│    │ search_crates      [Call]           │    │
│    │ Search crates by keyword            │    │
│    └─────────────────────────────────────┘    │
│    ┌─────────────────────────────────────┐    │
│    │ search_in_crate  [Call]             │    │
│    │ Search within a crate's docs        │    │
│    └─────────────────────────────────────┘    │
│                                               │
│  Resources view (grouped by server):          │
│  ▼ docs-rs (2 resources)                      │
│    crate_readme [Read]                         │
│    crate_doc    [Read]                         │
│                                               │
│  Prompts view (grouped by server):            │
│  ▼ docs-rs (1 prompt)                         │
│    crate_overview  [Get]                       │
│                                               │
└───────────────────────────────────────────────┘
```

### 4. Tool Call Dialog

When user clicks "Call" on a tool:
- Opens a modal/dialog overlay
- If tool has `input_schema`, renders a simple form with text inputs for each property
- Submit button calls `mcp_call_tool` via RPC client
- Result displayed below the form in a scrollable area
- Close button dismisses the dialog

### 5. Resource Viewer

When user clicks "Read" on a resource:
- Calls `mcp_read_resource` via RPC client
- Content displayed inline in a scrollable, monospace text area
- Error shown if resource read fails

### 6. Prompt Viewer

When user clicks "Get" on a prompt:
- If prompt has arguments, renders form inputs
- Calls `mcp_get_prompt` via RPC client
- Result messages displayed in a simple formatted view

### 7. Tab integration

**File: `vol-llm-ui/src/web/components/app.rs`**

- Import `McpPanel` component
- Add `TabButton { state, tab: ActiveTab::Mcp, label: "MCP" }` in `TabBar`
- Add `ActiveTab::Mcp => rsx! { McpPanel {} }` in `TabContent`

### 8. Component registration

**New file: `vol-llm-ui/src/web/components/mod.rs`** — add `pub mod mcp_panel;`

## Error Handling

- **No servers configured**: MCP tab shows "No MCP servers configured" empty state
- **Server disconnected**: tools/resources/prompts panels show "not connected" badge with reconnect option
- **RPC failures**: error displayed in panel header area, retry button available
- **Tool call failures**: error message shown in tool call dialog result area
- **Concurrent operations**: loading state disabled for each sub-panel independently

## Data Flow

1. `McpPanel` mounts → calls `client.mcp_list_servers` and `client.mcp_list_tools`
2. Responses populate `Signal<McpState>` fields
3. User clicks "Reconnect" → calls `client.mcp_reconnect` → refreshes data on success
4. User clicks "Call" → opens dialog → submits → calls `client.mcp_call_tool` → displays result
5. No polling — data is loaded on mount and manually refreshed
