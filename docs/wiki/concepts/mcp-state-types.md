---
type: concept
category: pattern
tags: [mcp, state, web, ui, dioxus]
created: 2026-05-14
updated: 2026-05-16 (mcp-toolcall-input-schema)
source_count: 3
---

# MCP State Types Pattern

**Category:** UI state pattern for MCP server management
**Related:** [[vol-llm-ui-crate]], [[dioxus-web-pattern]], [[mcp-client-integration]], [[mcp-manager-lifecycle]], [[mcp-transport-pattern]]

## Definition

State types and wire structures for displaying Model Context Protocol (MCP) servers, tools, resources, and prompts in the Dioxus web frontend. Added as part of the multi-step plan to build an MCP management panel into vol-llm-ui.

## Key Points

- `ActiveTab::Mcp` variant inserted between `Skills` and `Logs` in the tab toggle cycle
- `McpSubtab` enum provides four sub-tabs: `Servers`, `Tools`, `Resources`, `Prompts`
- Wire types are serializable structs matching JSON-RPC responses from mcp.list_servers, mcp.list_tools, mcp.list_resources, mcp.list_resource_templates, mcp.list_prompts
- Local state structs (web-only, gated with `#[cfg(feature = "web")]`) include loading states, error handling, and viewer dialogs

## Wire Types

All serializable with serde, used for JSON-RPC deserialization:

| Type | Source RPC | Purpose |
|------|-----------|---------|
| `McpServerInfo` | mcp.list_servers | Server name and status |
| `McpToolInfo` | mcp.list_tools | Tool with server, name, description, input_schema |
| `McpResourceInfo` | mcp.list_resources | Resource with server, name, URI, MIME type |
| `McpResourceTemplateInfo` | mcp.list_resource_templates | URI template with server, name, description |
| `McpPromptInfo` | mcp.list_prompts | Prompt with server, name, description, arguments |
| `McpPromptArgInfo` | (nested in McpPromptInfo) | Prompt argument with name, description, required flag |

## Local State Types

Gated with `#[cfg(all(feature = "web", not(feature = "tui")))]`:

| Type | Purpose |
|------|---------|
| `McpServerRowState` | Display row with reconnecting flag |
| `McpState` | Panel state: lists, loading, error, active subtab, dialogs |
| `McpToolCallState` | Tool call dialog with JSON args, input_schema, result, error |
| `McpResourceViewerState` | Resource viewer with URI, content, loading state |
| `McpPromptViewerState` | Prompt viewer with server, args, result, loading state |

## Architecture

```
ActiveTab::Mcp
    -> McpState (panel-level state)
        -> McpSubtab (active subtab)
            -> Servers -> Vec<McpServerInfo>
            -> Tools -> Vec<McpToolInfo>
            -> Resources -> Vec<McpResourceInfo> + Vec<McpResourceTemplateInfo>
            -> Prompts -> Vec<McpPromptInfo>
        -> Dialog states (tool call, resource viewer, prompt viewer)
```

## How It Works

1. `McpState::new()` initializes with `loading: true` and empty data lists
2. JSON-RPC client calls mcp.list_servers, mcp.list_tools, etc. to populate the lists
3. User switches subtabs via `McpSubtab` enum
4. Interactive features (calling tools, viewing resources, running prompts) open dialog states
5. Each dialog tracks its own loading/error/result lifecycle

## Related Concepts
- [[vol-llm-ui-crate]]: Crate containing the state types
- [[dioxus-web-pattern]]: Web frontend architecture using per-component signals
- [[mcp-client-integration]]: Bridging MCP server tools into the agent
- [[mcp-manager-lifecycle]]: McpManager connection lifecycle
- [[mcp-transport-pattern]]: Multi-transport startup pattern for MCP servers
- [[event-bus-pattern]]: EventBus routing used for cross-component communication
- [[tool-call-dialog-component]]: ToolCallDialog component renders McpToolCallState as a modal dialog
- [[mcp-toolcall-input-schema]]: McpToolCallState gained `input_schema: Option<serde_json::Value>` field for SchemaForm support
