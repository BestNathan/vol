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
- Displays server/tool name header, SchemaForm for structured parameter input, Call button, and result/error panels
- Form values held in local `Signal<serde_json::Value>`, initialized from JSON Schema via `build_form_defaults()`
- Call button serializes form value and invokes `client.mcp_call_tool()` with a callback that writes result/error back to the signal
- Tracks loading state with "Calling..." indicator during async RPC call
- Close button sets `tool_call_dialog` to `None`
- Uses `write_unchecked()` for mutations, matching existing Dioxus patterns in the codebase
- Registered in `crates/vol-llm-ui/src/web/components/mod.rs`

## Detailed Summary

The `ToolCallDialog` component (`crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`) is a Dioxus 0.6 WASM component that provides the interactive tool invocation UI for the MCP panel. It:

1. **Destructures dialog state** from `Signal<McpState>` via `.map()` and `let Some(...) else` pattern (not `?` operator, since Dioxus 0.6 `Element` is `Result<VNode, RenderError>`, not `Option`)
2. **Renders a modal overlay** with fixed positioning, dark backdrop, centered card
3. **SchemaForm** — auto-generated form fields from the tool's JSON Schema (text inputs, number inputs, checkboxes, enum dropdowns, nested objects) [[schema-form-pattern]]
4. **Call button** — serializes form state via `serde_json::to_string`, parses back to `serde_json::Value`, then invokes `rpc_client.mcp_call_tool()` with a callback that writes result/error back to the signal
5. **Result panel** — green-bordered box with monospace pre-formatted text
6. **Error panel** — red-bordered box for both form serialization errors and RPC errors

As of 2026-05-16, the component was rewritten to use `SchemaForm` instead of a raw JSON textarea. See [[schemaform-toolcall-dialog]] for the integration details.

## Entities Mentioned
- [[vol-llm-ui-crate]]: Component lives in web/components module
- [[mcp-state-types]]: Uses McpState and McpToolCallState types

## Concepts Covered
- [[dioxus-web-pattern]]: New component added to the web frontend
- [[mcp-client-integration]]: Tool invocation via JsonRpcClient.mcp_call_tool()
- [[schema-form-pattern]]: Auto-generated form from JSON Schema

## Notes
- Sibling components `mcp_prompt_viewer.rs` and `mcp_resource_viewer.rs` have the same `?` operator issue but were not modified (pre-existing, same compilation environment)
- Component uses exact same dark theme color palette as other MCP components (#1a1a2e, #252540, #3a3a55, #4080ff, #40c040, #c04040)
- As of 2026-05-16, rewritten to use SchemaForm — see [[schemaform-toolcall-dialog]] for integration details
