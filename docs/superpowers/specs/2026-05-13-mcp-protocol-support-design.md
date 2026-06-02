# MCP Protocol Support Design

## Architecture

The design extends the existing JSON-RPC WebSocket server (`vol-llm-agent-channel`) and web frontend (`vol-llm-ui`) with MCP tool discovery, management, and direct calling capabilities.

### Server-Side (vol-llm-agent-channel)

`JsonRpcConnection` receives an `Option<Arc<McpManager>>` field. When `None`, all MCP methods return appropriate errors. The JSON-RPC routing dispatches new `mcp.*` methods to handler functions in a separate `mcp_handlers.rs` module, keeping connection.rs focused on routing logic.

Status change events (`mcp.server_status_changed`) are pushed to all subscribed WebSocket clients via the existing subscription mechanism.

### Client-Side (vol-llm-ui)

DTO types decouple wire format from `rmcp::model`. The web client (`client.rs`) gains async RPC methods that serialize DTOs and deserialize responses. An `McpState` signal manages server/tool/call state. The MCP tab component tree renders server list → expandable tool list → call form → result.

### Component Tree

```
McpPanel
├── McpServerItem (per server)
│   ├── Status indicator (connected/disconnected/error)
│   ├── Lifecycle buttons (connect/disconnect/reconnect)
│   └── McpToolList (expanded, lazy-loaded)
│       ├── McpToolItem (per tool)
│       │   ├── Tool name, description
│       │   ├── Schema inspector (collapsible)
│       │   ├── Call form (dynamic or JSON textarea)
│       │   └── Call result display
│       └── NoToolsPlaceholder
└── NoServersPlaceholder
```

## Data Flow

```
[Web UI]                          [JSON-RPC Server]                    [McpManager]
   |                                      |                                  |
   |-- mcp.list_servers ---------------->|                                  |
   |<-- { servers: [McpServerInfo] } -----|<-- servers() -------------------|
   |                                      |                                  |
   |-- mcp.connect(server) -------------->|                                  |
   |<-- { status } -----------------------|-- connect_server(server) ------>|
   |<-- mcp.server_status_changed (push)--|                                  |
   |                                      |                                  |
   |-- mcp.list_tools ------------------->|                                  |
   |<-- { tools: [McpToolInfo] } ---------|<-- list_all_tools() ------------|
   |                                      |                                  |
   |-- mcp.call_tool(server, name, args)->|                                  |
   |<-- { result, duration_ms } ----------|-- call_tool(server, name, args)->|
```

## DTO Types

```rust
pub struct McpServerInfo {
    pub name: String,
    pub status: String, // "connected" | "disconnected" | "error"
    pub error: Option<String>,
    pub tool_count: usize,
}

pub struct McpToolInfo {
    pub server: String,
    pub name: String,
    pub description: Option<String>,
    pub input_schema: Option<serde_json::Value>,
}

pub struct McpCallResult {
    pub result: String,
    pub duration_ms: u64,
}
```

## Error Handling

All MCP operations return JSON-RPC error responses with standardized codes:
- `-32601` Method not found (routing layer)
- `-32001` McpManager not available
- `-32002` Server not found
- `-32003` Server disconnected
- `-32004` Tool call failed

Errors are displayed inline in the UI next to the relevant server or tool entry.

## File Changes

**New files:**
- `crates/vol-llm-agent-channel/src/jsonrpc/mcp_handlers.rs` — MCP JSON-RPC handler implementations
- `crates/vol-llm-ui/src/web/dto.rs` — DTO type definitions (McpServerInfo, McpToolInfo, McpCallResult)
- `crates/vol-llm-ui/src/web/components/mcp_panel.rs` — MCP tab component tree

**Modified files:**
- `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs` — add `mcp_manager` field, route `mcp.*` methods
- `crates/vol-llm-agent-channel/src/jsonrpc/mod.rs` — add `mcp_handlers` module declaration
- `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs` — add MCP method parsing variants
- `crates/vol-llm-ui/src/web/client.rs` — add MCP RPC methods and DTO types
- `crates/vol-llm-ui/src/web/components/mod.rs` — add `mcp_panel` module export
- `crates/vol-llm-ui/src/web/components/app.rs` — add `ActiveTab::Mcp` routing, MCP tab button
- `crates/vol-llm-ui/src/state/mod.rs` — add `ActiveTab::Mcp`, `McpState`, `McpServerEntry` types

## Form Generation Logic

Tool call forms use a simple schema complexity heuristic:
- **Dynamic form**: schema has ≤ 10 properties, depth ≤ 2, no `allOf`/`oneOf`/`anyOf`, no `$ref`
- **JSON textarea**: everything else

Dynamic form renders one input per top-level property:
- `string` → text input
- `number`/`integer` → number input
- `boolean` → checkbox
- `enum` → select dropdown
- Other types → text input with JSON hint

## Testing Strategy

- Server-side: unit tests for each MCP handler function with mocked `McpManager`
- Client-side: unit tests for DTO serialization/deserialization
- Integration: verify JSON-RPC message flow through the WebSocket layer
- UI: verify MCP tab renders correctly in each server state (empty, connected, error, disconnected)
