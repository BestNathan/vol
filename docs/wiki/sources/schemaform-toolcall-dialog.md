---
type: source
source_type: code
date: 2026-05-16
ingested: 2026-05-16
tags: [dioxus, mcp, web, component, schema-form, tool-call]
---

# SchemaForm Integration into ToolCallDialog

**Authors/Creators:** BestNathan
**Date:** 2026-05-16
**Link:** crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs

## TL;DR
Task 3: `ToolCallDialog` rewritten to use the `SchemaForm` component instead of a raw JSON textarea. Form values are held in a local `Signal<serde_json::Value>` initialized from the tool's JSON Schema.

## Key Takeaways
- Raw `<textarea>` for JSON editing replaced with `<SchemaForm>` component
- Form state managed via `use_signal` with `build_form_defaults()` for initialization
- `use_effect` re-initializes form when the input schema changes (cloned to avoid borrow-after-move)
- `build_form_defaults()` generates sensible defaults: empty string for strings, 0 for numbers, false for booleans, recursive for objects
- Call button serializes form value via `serde_json::to_string` then parses back to `serde_json::Value` before RPC call
- `SchemaForm` imported from `super::schema_form::SchemaForm`
- `cargo check -p vol-llm-ui --no-default-features --features web` passes cleanly

## Detailed Summary

The `ToolCallDialog` component (`crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`) was rewritten to integrate the `SchemaForm` component created in Task 2. Key changes:

1. **Removed**: Raw `<textarea>` with `arguments_json` string state, manual JSON validation
2. **Added**: `SchemaForm` component receiving `input_schema` from `McpToolCallState`, `form_value` signal for form state
3. **Form initialization**: `build_form_defaults()` reads JSON Schema `properties`, creates default values based on type (empty string, zero, false, recursive object, null for unknown)
4. **Effect hook**: `use_effect` with cloned schema reference re-initializes form when dialog opens with a new schema
5. **Call flow**: Form value serialized to string then parsed to `serde_json::Value` before passing to `rpc_client.mcp_call_tool()`
6. **Error handling**: Invalid form data caught during parsing, error displayed in existing error panel
7. **Removed**: `web_sys::console::log_1` debug log, `arguments_json` destructuring

The `build_form_defaults()` function is defined locally (not shared with `SchemaForm`) because the effect hook needs it at the component level for schema-change re-initialization. `SchemaForm` has its own `build_defaults()` for its `use_hook` initialization.

## Entities Mentioned
- [[vol-llm-ui-crate]]: Component lives in web/components module
- [[mcp-state-types]]: Uses McpToolCallState with input_schema field

## Concepts Covered
- [[dioxus-web-pattern]]: SchemaForm integration into ToolCallDialog
- [[schema-form-pattern]]: Auto-generated form from JSON Schema with type-specific field rendering

## Notes
- `form_value` declared as `let mut` because `use_effect` captures it by mutable borrow (Dioxus signal pattern)
- `input_schema` cloned into `input_schema_for_effect` before `use_effect` to avoid move-then-borrow error
- `arguments_json` field on `McpToolCallState` is no longer read by the dialog but remains in the struct (may be cleaned up separately)
