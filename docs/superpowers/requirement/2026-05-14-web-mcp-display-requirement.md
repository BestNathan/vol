# Requirements: MCP Web UI Support

## Background

The project has a fully functional MCP client layer (`vol-llm-mcp`) that manages connections to MCP servers, discovers tools/resources/prompts, and supports tool calling. However, the web frontend (`vol-llm-ui` Dioxus app) has no visibility into MCP state — no way to see which servers are connected, what tools are available, or interact with MCP resources. The backend JSON-RPC service exposes agent, session, and file methods but nothing MCP-related.

## Goals

1. Add a new "MCP" tab to the web UI that displays all MCP-related information (servers, tools, resources, prompts)
2. Expose JSON-RPC methods on the backend so the web UI can query MCP state and invoke MCP operations
3. Enable users to call MCP tools, read MCP resources, and retrieve MCP prompts directly from the web UI
4. Allow users to manually reconnect disconnected MCP servers from the UI

## Non-Goals

- Adding/removing/editing MCP server configurations from the UI (config stays in `.mcp.json`)
- Modifying the MCP backend data model or session management
- Real-time MCP server status push via events (status is polled via JSON-RPC)
- TUI changes — this feature is web-only

## Scope

### Included

**Backend JSON-RPC methods:**
- `mcp.list_servers` — returns server name, status, config summary
- `mcp.list_tools` — returns tools grouped by server (name, description, input schema)
- `mcp.call_tool` — calls a tool on a named server with arguments
- `mcp.list_resources` — returns resources grouped by server (name, URI, MIME type, description)
- `mcp.list_resource_templates` — returns resource templates grouped by server
- `mcp.read_resource` — reads a resource by URI from a connected server
- `mcp.list_prompts` — returns prompts grouped by server (name, description, arguments)
- `mcp.get_prompt` — retrieves a prompt with optional arguments
- `mcp.reconnect` — manually reconnect a disconnected server
- `mcp.server_status` — returns connection status for all servers

**Web UI — MCP Tab:**
- **Servers panel**: list of configured servers with status badge (Connected/Disconnected/Connecting/Error), server name, reconnect button for non-Connected servers
- **Tools panel**: tools grouped by server, expandable tool cards showing name, description, input schema with a "Call" button that opens a tool call dialog
- **Resources panel**: resources grouped by server, clickable to read content, resource templates listed with URI patterns
- **Prompts panel**: prompts grouped by server, expandable showing name, description, arguments with a "Get" button
- **Tool call dialog**: form for tool arguments (auto-generated from input schema), submit button, result display area
- **Resource viewer**: displays resource content inline
- **Prompt viewer**: form for prompt arguments, displays retrieved prompt messages

**State:**
- New `McpState` struct with servers, tools, resources, prompts, server status
- New `ActiveTab::Mcp` variant added to the tab cycle

### Excluded

- Server configuration editing (no add/remove/edit forms)
- MCP server environment variable management
- TUI MCP display
- Streaming MCP responses (tool calls are synchronous)
- MCP tool call history (only shows last result)

## Constraints

- Uses existing JSON-RPC WebSocket transport (`vol-llm-agent-channel`)
- MCP backend already uses `McpManager` from `vol-llm-mcp` — no changes to MCP session management
- Dioxus web app pattern: components use `Signal<T>` for local state, `EventBus` for reactive updates
- Tab cycle: Conversation → Sessions → Tools → Workspace → Skills → MCP → Logs → Agents
- rsproxy mirror required for Docker builds (cannot access crates.io)

## Success Criteria

1. `mcp.list_servers` JSON-RPC call returns at least server name and connection status
2. `mcp.list_tools` returns all tools from all connected servers with name and description
3. `mcp.call_tool` successfully calls a tool and returns the result string
4. `mcp.list_resources` returns resources from connected servers
5. `mcp.read_resource` returns resource content for a given URI
6. `mcp.list_prompts` returns prompts from connected servers
7. `mcp.get_prompt` retrieves a prompt with arguments
8. `mcp.reconnect` triggers a reconnect and updates status
9. Web UI renders the MCP tab with all four sub-panels (Servers, Tools, Resources, Prompts)
10. Server status badges correctly reflect connection state
11. Tool call dialog accepts arguments and displays results
12. Resource viewer displays content inline
13. Tab cycle includes MCP tab and cycles correctly

## Edge Cases

- **No MCP servers configured**: MCP tab shows empty state message ("No MCP servers configured")
- **All servers disconnected**: tools/resources/prompts panels show "not connected" state with reconnect option
- **Server connects mid-session**: user clicks reconnect, status updates, panels refresh with new data
- **Tool call fails**: error message displayed in tool call result area
- **Resource read fails**: error message shown inline
- **Large tool schemas**: input schema display should be scrollable/truncated
- **Concurrent reconnects**: server status should reflect "Connecting" state during reconnect

## Open Questions

None.
