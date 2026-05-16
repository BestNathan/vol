---
type: source
source_type: code
date: 2026-05-16
ingested: 2026-05-16
tags: [mcp, ui, dioxus, web, schema-form]
---

# Task 1: Add input_schema Field to McpToolCallState

**Authors/Creators:** vol-llm-ui team
**Date:** 2026-05-16
**Link:** `crates/vol-llm-ui/src/state/mod.rs`, `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

## TL;DR
Added `input_schema: Option<serde_json::Value>` field to `McpToolCallState` and updated the `ToolCard` onclick handler to pass the tool's input schema through to the dialog state. Removed a debug `console.log` line.

## Key Takeaways
- `McpToolCallState` now carries `input_schema` alongside `arguments_json`
- The `ToolCard` "Call" button clones `t.input_schema` into the dialog state
- This enables the future `SchemaForm` component to auto-generate form fields from JSON Schema instead of using a raw JSON textarea
- Debug `web_sys::console::log_1` line removed from onclick handler
- Cargo check with `--features web` passes cleanly

## Detailed Summary
The `McpToolCallState` struct (gated with `#[cfg(all(feature = "web", not(feature = "tui")))]`) previously tracked only `arguments_json: String` — a pretty-printed JSON string used as the textarea content in `ToolCallDialog`. This field was initialized from the tool's `input_schema` by serializing it, which meant the original schema structure was lost after dialog creation.

The new `input_schema: Option<serde_json::Value>` field preserves the raw JSON Schema Value alongside the serialized string. When the user clicks "Call" on a tool in the MCP Tools panel, the `ToolCard` component now passes both `arguments_json` (derived from `input_schema` for backwards compatibility) and `input_schema` itself (the raw Value).

## Entities Mentioned
- [[vol-llm-ui-crate]]: Crate containing the state types and ToolCard component

## Concepts Covered
- [[mcp-state-types]]: McpToolCallState gained input_schema field
- [[dioxus-web-pattern]]: ToolCard component updated

## Notes
This is Task 1 of a multi-step plan to replace the raw JSON textarea with a SchemaForm component. Tasks 2-4 will create the SchemaForm component and integrate it into ToolCallDialog.
