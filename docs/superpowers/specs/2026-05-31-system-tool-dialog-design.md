# System Tool Dialog & Fixed Ordering Design

**Date**: 2026-05-31
**Status**: Draft
**Author**: BestNathan

## Requirements

### Goals

1. **Fixed tool ordering**: Tools in the Tools tab are sorted alphabetically by name, not arbitrary HashMap order.
2. **Dialog-based tool execution**: Clicking "Run" on a system tool opens a modal dialog with a SchemaForm for arguments (same pattern as MCP's `McpToolDialog`), replacing the current inline empty-`{}` call.
3. **Reuse SchemaForm**: Use the existing `SchemaForm` component from `mcp_tool_dialog.rs` for parameter input.

### Non-Goals

- Changing `tool.call` RPC endpoint behavior (it already works correctly)
- Adding tool call history entries for manual runs (history remains agent-execution-only)
- Changing MCP tool dialog behavior

---

## Architecture

```
Before (broken):
  Click "Run" → client.tool_call(name, {}, cb) → raw JSON shown inline in call_result

After:
  Click "Run" → open SystemToolDialog modal
    → SchemaForm renders fields from tool.parameters JSON Schema
    → user fills args → click Execute
    → client.tool_call(name, args, cb) → result shown in dialog
    → click Close / ✕ to dismiss
```

### Component Diagram

```
ToolsTabContent
  ├── Mobile: tool cards with "Run" button
  ├── Desktop: tool rows with "Run" button
  ├── Call result (inline, kept for backward compat)
  └── SystemToolDialog (new, modal overlay)
        ├── Header: tool name + description + close button
        ├── SchemaForm (reused): renders fields from parameters JSON Schema
        └── Result area: success/error output after execution
```

---

## Key Changes

### 1. Server-side: sort tools in ToolHandler

**File:** `crates/vol-llm-agent-channel/src/domain/tool.rs`, line 46-57

```rust
(ToolOperation::List, Payload::Tool(ToolPayload::List)) => {
    let mut tools: Vec<serde_json::Value> = self
        .tool_registry
        .definitions()
        .iter()
        .map(|d| {
            serde_json::json!({
                "name": d.name,
                "description": d.description,
                "parameters": d.parameters,
            })
        })
        .collect();
    tools.sort_by(|a, b| {
        a.get("name").and_then(|v| v.as_str()).unwrap_or("")
            .cmp(b.get("name").and_then(|v| v.as_str()).unwrap_or(""))
    });
    Ok(vec![AgentServerMessage::new_result(/* ... */)])
}
```

### 2. Client-side: SystemToolDialog component (new)

**File:** `crates/vol-llm-ui/src/web/components/tool_dialog.rs` (new)

Reuses `SchemaForm` from `mcp_tool_dialog.rs`. Dialog state managed locally via signals:

```rust
struct ToolDialogState {
    open: bool,
    tool_name: String,
    description: String,
    parameters: Option<serde_json::Value>,
    result: Option<String>,
    error: Option<String>,
    loading: bool,
}
```

Dialog layout:
```
div.modal-overlay (fixed inset-0 z-50)
  div.dialog (w-[95vw] sm:w-[600px] max-h-[85vh] flex flex-col)
    div.header: tool name + description + close btn
    div.body: SchemaForm { schema: parameters }
    div.result (if present): pre with result text
    div.footer: [Execute] [Cancel]
```

### 3. Client-side: wire Run button to dialog

**File:** `crates/vol-llm-ui/src/web/components/tools_tab.rs`

Both mobile card and desktop row "Run" buttons change from:
```rust
// Old: direct RPC call with empty args
button { onclick: move |_| {
    client.tool_call(&name, &json!({}), move |result| { /* inline display */ });
}, "Run" }
```

To:
```rust
// New: open dialog
button { onclick: move |_| {
    dialog_state.with_mut(|s| {
        s.open = true;
        s.tool_name = name;
        s.description = desc;
        s.parameters = params;
        s.result = None;
        s.error = None;
    });
}, "Run" }
```

---

## Crate / File Structure

```
crates/vol-llm-agent-channel/src/domain/
└── tool.rs                    # MODIFIED: sort tools alphabetically

crates/vol-llm-ui/src/web/components/
├── tools_tab.rs               # MODIFIED: Run button opens dialog instead of inline call
├── tool_dialog.rs             # NEW: SystemToolDialog component
└── mod.rs                     # MODIFIED: register new component module
```

---

## Testing Strategy

- Start dev server, open Tools tab, verify tools appear in alphabetical order
- Click "Run" on a tool → verify dialog opens with SchemaForm
- Fill in arguments → click Execute → verify result appears in dialog
- Click Cancel / ✕ → verify dialog closes
- Test with a tool that has no parameters → SchemaForm empty, Execute should work with `{}`
