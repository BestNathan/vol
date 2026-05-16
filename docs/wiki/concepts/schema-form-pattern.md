---
type: concept
category: pattern
tags: [dioxus, web, form, json-schema, component]
created: 2026-05-16
updated: 2026-05-16 (schemaform-toolcall-dialog)
source_count: 2
---

# Schema Form Pattern

**Category:** Component pattern
**Related:** [[dioxus-web-pattern]], [[tool-call-dialog-component]], [[mcp-state-types]]

## Definition

Auto-generates interactive form fields from a JSON Schema object. Used to render tool parameter inputs in the `ToolCallDialog` without manually coding each field.

## Key Points

- `SchemaForm` component accepts `schema: serde_json::Value` and `value: Signal<serde_json::Value>`
- Reads `properties` from the schema to generate one field per property
- Reads `required` array to mark fields with a red `*` indicator
- Field types supported: `string` (text input or enum select), `number`/`integer` (number input), `boolean` (checkbox), `object` (nested SchemaForm)
- Unknown types render a "Unsupported type" message
- `build_defaults()` initializes form state: uses schema `default` if present, otherwise type-appropriate zero value (empty string, 0, false, recursive object, null)
- Uses `use_hook` for one-time initialization on mount
- `SchemaField` is an internal `#[component]` that dispatches to type-specific render functions
- Type-specific render functions: `render_string_field`, `render_number_field`, `render_boolean_field`, `render_object_field`

## How It Works

1. Parent component passes JSON Schema (from MCP tool `inputSchema`) and a mutable signal
2. `SchemaForm` clones schema, calls `build_defaults()` in `use_hook` to seed the signal
3. Iterates over `schema.properties`, renders `SchemaField` for each property
4. `SchemaField` inspects `type` field, dispatches to appropriate renderer
5. Each renderer reads current value from signal, renders appropriate HTML input, writes back on change via `value.write_unchecked()[field_key] = ...`
6. For nested objects, `render_object_field` recursively renders another `SchemaForm` with the same shared signal

## Example

```rust
// In ToolCallDialog
let mut form_value: Signal<serde_json::Value> = use_signal(|| serde_json::Value::Object(serde_json::Map::new()));
use_effect(move || {
    if let Some(ref schema) = input_schema {
        form_value.set(build_form_defaults(schema));
    }
});

rsx! {
    SchemaForm { schema: schema.clone(), value: form_value }
}
```

## build_form_defaults vs build_defaults

Two separate default-building functions exist:
- `build_form_defaults()` in `mcp_tool_dialog.rs` — used by the dialog's `use_effect` to re-initialize form when schema changes
- `build_defaults()` in `schema_form.rs` — used by `SchemaForm`'s `use_hook` for initial mount

Both are identical implementations. They could be unified into a shared utility in the future.

## Files
- `crates/vol-llm-ui/src/web/components/schema_form.rs` — `SchemaForm`, `SchemaField`, render functions, `build_defaults()`
- `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs` — `ToolCallDialog`, `build_form_defaults()`

## Related Concepts
- [[dioxus-web-pattern]]: Dioxus WASM component architecture
- [[tool-call-dialog-component]]: ToolCallDialog component using SchemaForm
- [[mcp-state-types]]: McpToolCallState with input_schema field
