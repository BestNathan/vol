---
type: source
source_type: code
date: 2026-05-15
ingested: 2026-05-15
tags: [dioxus, mcp, web, component, tool-call]
---

# ToolCallDialog Component

**Authors/Creators:** BestNathan
**Date:** 2026-05-15
**Link:** crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs

## TL;DR
New Dioxus component `ToolCallDialog` that renders a modal dialog for invoking MCP tools with editable JSON arguments, displaying results and errors inline.

## Key Takeaways
- Component receives `Signal<McpState>` and `AppState` (for `rpc_client`) as props
- When `McpState.tool_call_dialog` is `None`, returns empty `rsx!{}`
- Displays server/tool name header, editable JSON textarea, Call button, and result/error panels
- Validates JSON arguments before calling `client.mcp_call_tool()`
- Tracks loading state with "Calling..." indicator during async RPC call
- Close button sets `tool_call_dialog` to `None`
- Uses `write_unchecked()` for mutations, matching existing Dioxus patterns in the codebase
- Registered in `crates/vol-llm-ui/src/web/components/mod.rs`

## Detailed Summary

The `ToolCallDialog` component (`crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`) is a Dioxus 0.6 WASM component that provides the interactive tool invocation UI for the MCP panel. It:

1. **Destructures dialog state** from `Signal<McpState>` via `.map()` and `let Some(...) else` pattern (not `?` operator, since Dioxus 0.6 `Element` is `Result<VNode, RenderError>`, not `Option`)
2. **Renders a modal overlay** with fixed positioning, dark backdrop, centered card
3. **Editable JSON textarea** — users can modify tool arguments before calling; oninput updates the signal directly
4. **Call button** — validates JSON via `serde_json::from_str`, sets loading state, then invokes `rpc_client.mcp_call_tool()` with a callback that writes result/error back to the signal
5. **Result panel** — green-bordered box with monospace pre-formatted text
6. **Error panel** — red-bordered box for both JSON parse errors and RPC errors

The component was registered in `mod.rs` alongside `mcp_panel`. The initial `?` operator pattern (used in sibling components `mcp_prompt_viewer.rs` and `mcp_resource_viewer.rs`) was replaced with `let Some(...) else { return rsx! {}; }` because Dioxus 0.6's `Element` type is `Result<VNode, RenderError>`, making `?` on `Option` incompatible.

## Entities Mentioned
- [[vol-llm-ui-crate]]: Component lives in web/components module
- [[mcp-state-types]]: Uses McpState and McpToolCallState types

## Concepts Covered
- [[dioxus-web-pattern]]: New component added to the web frontend
- [[mcp-client-integration]]: Tool invocation via JsonRpcClient.mcp_call_tool()

## Notes
- Sibling components `mcp_prompt_viewer.rs` and `mcp_resource_viewer.rs` have the same `?` operator issue but were not modified (pre-existing, same compilation environment)
- Component uses exact same dark theme color palette as other MCP components (#1a1a2e, #252540, #3a3a55, #4080ff, #40c040, #c04040)
