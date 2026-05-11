# Design: ReAct Agent MCP Tool Integration

## Overview

Enable the ReAct agent to discover and invoke external MCP servers configured via `~/.mcp.json` and `.mcp.json`. A new `vol-llm-mcp` crate provides the MCP Client protocol layer, and MCP tools are registered into the existing `ToolRegistry` as `McpTool` instances.

## Architecture

### Dependency Graph

```
vol-llm-mcp → rmcp              (MCP Client protocol)
vol-llm-tool → vol-llm-mcp      (McpTool uses McpSession)
vol-llm-agent → vol-llm-tool, vol-llm-mcp  (builds McpSession, registers MCP tools)
```

`vol-llm-mcp` is the bottom layer — pure MCP protocol, no dependency on tool/agent crates.

### Module Structure

**`vol-llm-mcp` (new crate):**
```
crates/vol-llm-mcp/
├── Cargo.toml
└── src/
    ├── lib.rs              → public re-exports
    ├── config.rs           → .mcp.json parsing + merge logic
    ├── session.rs          → McpSession: connect/list_tools/call_tool/disconnect
    ├── transport.rs        → Transport creation (STDIO)
    └── error.rs            → McpError enum
```

**`vol-llm-tool` (existing crate, new files):**
```
crates/vol-llm-tool/
├── src/
│   ├── mcp_tool.rs         → McpTool: implements ExecutableTool trait
│   └── lib.rs              → re-exports McpTool
```

**`vol-llm-agent` (existing crate, modifications):**
```
crates/vol-llm-agent/
└── src/react/
    ├── config_builder.rs   → new .with_mcp_from_config() method
    └── agent.rs            → McpSession lifecycle (disconnect on run complete)
```

## Components

### 1. McpConfig (`vol-llm-mcp/config.rs`)

Parses and merges MCP server configurations from two sources:

| Source | Path | Priority |
|--------|------|----------|
| Project-level | `.mcp.json` (working directory root) | Higher |
| User-level | `~/.mcp.json` | Fallback |

Schema (strictly follows Claude Desktop format):
```json
{
  "mcpServers": {
    "server-name": {
      "command": "npx",
      "args": ["-y", "@some/mcp-server"],
      "env": { "API_KEY": "value" }
    }
  }
}
```

Merge semantics: per-key replacement. If both files define a server with the same name, the project-level definition wins.

Key functions:
- `McpConfig::load(working_dir: Option<&Path>) -> Result<Self, McpError>`
- `McpConfig::servers(&self) -> Vec<McpServerConfig>`

### 2. McpSession (`vol-llm-mcp/session.rs`)

Core protocol type. Manages connections to all configured MCP servers.

```rust
pub struct McpSession {
    connections: HashMap<String, ServerConnection>,
}

pub struct ServerConnection {
    config: McpServerConfig,
    client: rmcp::Client<...>,  // rmcp client instance
    tools: Vec<McpToolInfo>,    // cached tool list from initialization
}
```

Lifecycle:
1. `McpSession::connect(configs)` — for each config, create transport → spawn child → MCP initialize handshake → cache tools
2. `session.list_tools(server)` — return cached tool list (no RPC)
3. `session.call_tool(server, tool, args)` — RPC call via rmcp transport
4. `session.disconnect()` — graceful shutdown of all child processes

Initialization is per-server with isolation: if one server fails, others continue.

### 3. McpTool (`vol-llm-tool/mcp_tool.rs`)

Implements `ExecutableTool`, proxying execution to `McpSession`.

```rust
pub struct McpTool {
    session: Arc<McpSession>,
    server_name: String,      // e.g. "weather"
    tool_name: String,        // e.g. "get_forecast"
    display_name: String,     // "mcp__weather_get_forecast" (static)
    description: String,      // (static)
    parameters: serde_json::Value,  // (static)
}
```

The `display_name`, `description`, and `parameters` are computed once during construction and stored as owned strings to satisfy `ExecutableTool::name() -> &'static str`. A `Leak`-based approach is used: during `McpTool::new()`, the display name string is leaked to get a `&'static str`. This is acceptable because the number of MCP tools is bounded and small.

Name sanitization: `server_name` is sanitized to only contain `[a-zA-Z0-9_-]` characters. Regex `[^a-zA-Z0-9_-]` → `_`, consecutive underscores merged.

### 4. ToolRegistry Extension (`vol-llm-tool/registry.rs`)

New method:
```rust
impl ToolRegistry {
    pub async fn register_from_mcp(&mut self, session: Arc<McpSession>);
}
```

Iterates all servers → all tools → creates `McpTool` → registers via `register_boxed()`.

### 5. AgentConfigBuilder Extension (`vol-llm-agent/react/config_builder.rs`)

New builder method:
```rust
impl AgentConfigBuilder {
    pub async fn with_mcp_from_config(
        self,
        working_dir: Option<&Path>,
    ) -> Result<Self, McpError>;
}
```

Internal flow:
1. `McpConfig::load(working_dir)` → parse + merge configs
2. `McpSession::connect(configs)` → establish connections
3. `registry.register_from_mcp(session)` → register all MCP tools
4. Store `Arc<McpSession>` in builder for later cleanup

### 6. Agent Lifecycle (`vol-llm-agent/react/agent.rs`)

`McpSession` is stored in `AgentConfig` as an optional field:
```rust
pub struct AgentConfig {
    // ... existing fields ...
    pub mcp_session: Option<Arc<McpSession>>,
}
```

After `run()` completes (success or failure), `mcp_session.disconnect()` is called in the cleanup phase (after agent_task, interceptor, and listener joins).

## Error Handling

### McpError Enum

```rust
pub enum McpError {
    ConfigParse { path: String, detail: String },
    ServerNotFound(String),
    ConnectionFailed { server: String, detail: String },
    InitializeTimeout { server: String },
    ToolCallFailed { server: String, tool: String, detail: String },
    TransportError(String),
}
```

### Error Flow Table

| Stage | Error | Handling |
|-------|-------|----------|
| Config parse | JSON invalid | Log error, return empty config, agent continues |
| Server connect | command not found, permission denied | Log error, emit event, skip server, agent continues |
| Server connect | initialize timeout (10s) | Log error, emit event, skip server, agent continues |
| Tool call | server crashed / disconnected | Return `ToolResult::failure(...)`, error injected into ReAct context |
| Tool call | tool not found on server | Return `ToolResult::failure(...)`, error injected into ReAct context |

## Data Flow

### Initialization
```
AgentConfig::builder()
  .with_llm(llm)
  .with_mcp_from_config(None)        ← auto-discover configs
  .with_tools(tool_registry)         ← native tools already registered
  .build()
    │
    ├─ McpConfig::load(None)
    │   ├─ try .mcp.json
    │   ├─ try ~/.mcp.json
    │   └─ merge (project > global)
    │
    ├─ McpSession::connect(configs)
    │   ├─ server "weather" → STDIO spawn → initialize → cache tools ✓
    │   ├─ server "github" → STDIO spawn → initialize → cache tools ✓
    │   └─ server "broken" → spawn failed → skip ✗
    │
    └─ registry.register_from_mcp(session)
        ├─ "mcp__weather_get_temperature" → McpTool
        ├─ "mcp__weather_get_forecast" → McpTool
        └─ "mcp__github_search_repos" → McpTool
```

### Tool Execution
```
LLM: calls mcp__weather_get_temperature({"location": "Tokyo"})
  ↓
ReActAgent: config.tools.execute(call, ctx)
  ↓
ToolRegistry: lookup("mcp__weather_get_temperature") → McpTool
  ↓
McpTool.execute(args, ctx)
  ↓
McpSession.call_tool("weather", "get_temperature", args)
  ↓
rmcp transport (STDIO stdin/stdout)
  ↓
Child process responds → result string
  ↓
ToolResult::success(content) → injected into ReAct context
```

## Testing Strategy

- **Unit tests**: `McpConfig` parse + merge logic with fixture JSON files
- **Unit tests**: `McpTool` name sanitization
- **Integration tests**: Mock MCP server (simple STDIO echo process) → verify `list_tools` and `call_tool`
- **Integration tests**: Agent with MCP tools → verify full ReAct loop with tool call

No real external MCP servers in tests — use a stub process that responds to MCP protocol messages via STDIO.

## Constraints Followed

- No circular dependencies: `vol-llm-mcp` has no dependency on `vol-llm-tool`
- Existing `ToolRegistry` API extended, not replaced
- No modifications to ReAct loop logic — MCP tools are transparent to the agent
- Strict `~/.mcp.json` schema compliance
