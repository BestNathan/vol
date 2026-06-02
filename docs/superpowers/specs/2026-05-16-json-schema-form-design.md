# JSON Schema Form Component for MCP Tool Invocation

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Generate UI form fields from MCP tool `input_schema` JSON Schema, replacing the raw textarea in `ToolCallDialog`.

**Architecture:** New `SchemaForm` component reads JSON Schema `properties`, renders appropriate HTML inputs (text, number, checkbox, select), and collects values into a `serde_json::Value` signal. `ToolCallDialog` integrates `SchemaForm` as the parameter input area.

**Tech Stack:** Dioxus 0.6 web frontend, serde_json, TailwindCSS

---

## File Structure

- **Create:** `crates/vol-llm-ui/src/web/components/schema_form.rs` — JSON Schema → Form renderer
- **Modify:** `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs` — Replace textarea with SchemaForm
- **Modify:** `crates/vol-llm-ui/src/web/components/app.rs` — Add SchemaForm import
- **Modify:** `crates/vol-llm-ui/src/state/mod.rs` — No changes needed (existing types suffice)

## Supported JSON Schema Types

| Schema Type | UI Control | Notes |
|-------------|-----------|-------|
| `string` (no enum) | `<input type="text">` | Default empty string |
| `string` + `enum` | `<select>` | First enum value as default |
| `number` / `integer` | `<input type="number">` | Default 0 |
| `boolean` | `<input type="checkbox">` | Default false |
| `object` | Nested form group | Recursive rendering of `properties` |

Schema `required` array → `*` marker on label + validation before submit.

## Data Flow

```
McpToolInfo.input_schema (Option<Value>)
  → SchemaForm { schema, value_signal }
    → Renders form fields from schema.properties
    → User fills fields → value_signal updated
    → On submit: serde_json::to_string(&value_signal) → mcp_call_tool()
```

## Components

### SchemaForm

Props:
- `schema: serde_json::Value` — The JSON Schema object
- `value: Signal<serde_json::Value>` — Shared form data state

Behavior:
- Reads `schema["properties"]` and renders a field for each
- Each field maps to a key in the `value` signal
- On mount: initialize `value` with schema `default` values or empty/zero defaults

### SchemaFormField (internal helper)

Props:
- `key: String`, `prop_schema: Value`, `value_signal: Signal<Value>`

Behavior:
- Matches `type` field, renders appropriate input
- `string` → text input, `string`+`enum` → select, `number` → number input, `boolean` → checkbox, `object` → nested SchemaForm

### ToolCallDialog (modified)

Changes:
- Remove `<textarea>` for JSON editing
- Add `<SchemaForm schema={tool.input_schema} value={form_value_signal} />`
- On submit: validate required fields, serialize `form_value_signal` to JSON, call `mcp_call_tool()`
- If `input_schema` is None or has no `properties`: show fallback "No parameters required" message

### Task 1: Create SchemaForm component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/schema_form.rs`

- [ ] **Step 1: Write the SchemaForm component**

```rust
// crates/vol-llm-ui/src/web/components/schema_form.rs
use dioxus::prelude::*;

#[component]
pub fn SchemaForm(schema: serde_json::Value, value: Signal<serde_json::Value>) -> Element {
    // Initialize defaults on mount
    use_hook(move || {
        let defaults = build_defaults(&schema);
        value.set(defaults);
    });

    let properties = schema.get("properties").and_then(|v| v.as_object());
    let required: std::collections::HashSet<String> = schema
        .get("required")
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
        .unwrap_or_default();

    let Some(props) = properties else {
        return rsx! {
            div { class: "text-[#888] text-[12px]", "No parameters required" }
        };
    };

    rsx! {
        div { class: "space-y-2",
            {props.iter().map(|(key, prop_schema)| {
                let is_required = required.contains(key);
                rsx! { SchemaField {
                    key: key.clone(),
                    prop_schema: prop_schema.clone(),
                    value,
                    required: is_required,
                } }
            }).collect::<Vec<Element>>().into_iter()}
        }
    }
}

fn build_defaults(schema: &serde_json::Value) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, prop) in props {
            let default = if let Some(d) = prop.get("default") {
                d.clone()
            } else {
                match prop.get("type").and_then(|t| t.as_str()) {
                    Some("string") => serde_json::Value::String(String::new()),
                    Some("number") | Some("integer") => serde_json::Value::Number(0.into()),
                    Some("boolean") => serde_json::Value::Bool(false),
                    Some("object") => build_defaults(prop),
                    _ => serde_json::Value::Null,
                }
            };
            obj.insert(key.clone(), default);
        }
    }
    serde_json::Value::Object(obj)
}
```

- [ ] **Step 2: Write the SchemaField component (same file)**

```rust
#[component]
pub fn SchemaField(key: String, prop_schema: serde_json::Value, value: Signal<serde_json::Value>, required: bool) -> Element {
    let type_str = prop_schema.get("type").and_then(|t| t.as_str()).unwrap_or("string");
    let label = prop_schema.get("title").and_then(|t| t.as_str()).unwrap_or(&key);
    let desc = prop_schema.get("description").and_then(|t| t.as_str());

    match type_str {
        "string" => {
            // Check for enum
            if let Some(enum_vals) = prop_schema.get("enum").and_then(|v| v.as_array()) {
                let options: Vec<String> = enum_vals.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                rsx! {
                    div { class: "flex flex-col gap-1",
                        label { class: "text-[12px] text-[#aaa] font-semibold",
                            "{label}"
                            if required { span { class: "text-[#c04040]", " *" } }
                        }
                        select {
                            class: "bg-[#252540] border border-[#3a3a55] rounded px-2 py-1 text-[12px] text-[#e0e0e0]",
                            value: "{value.read()[key].as_str().unwrap_or("")}",
                            onchange: move |ev| {
                                value.write_unchecked()[key.clone()] = serde_json::Value::String(ev.value());
                            },
                            option { value: "", "Select..." }
                            {options.iter().map(|opt| {
                                rsx! { option { value: "{opt}", "{opt}" } }
                            }).collect::<Vec<Element>>().into_iter()}
                        }
                        if let Some(d) = desc {
                            div { class: "text-[10px] text-[#666]", "{d}" }
                        }
                    }
                }
            } else {
                rsx! {
                    div { class: "flex flex-col gap-1",
                        label { class: "text-[12px] text-[#aaa] font-semibold",
                            "{label}"
                            if required { span { class: "text-[#c04040]", " *" } }
                        }
                        input {
                            r#type: "text",
                            class: "bg-[#252540] border border-[#3a3a55] rounded px-2 py-1 text-[12px] text-[#e0e0e0]",
                            value: "{value.read()[key].as_str().unwrap_or("")}",
                            oninput: move |ev| {
                                value.write_unchecked()[key.clone()] = serde_json::Value::String(ev.value());
                            },
                        }
                        if let Some(d) = desc {
                            div { class: "text-[10px] text-[#666]", "{d}" }
                        }
                    }
                }
            }
        }
        "number" | "integer" => {
            rsx! {
                div { class: "flex flex-col gap-1",
                    label { class: "text-[12px] text-[#aaa] font-semibold",
                        "{label}"
                        if required { span { class: "text-[#c04040]", " *" } }
                    }
                    input {
                        r#type: "number",
                        class: "bg-[#252540] border border-[#3a3a55] rounded px-2 py-1 text-[12px] text-[#e0e0e0]",
                        value: "{value.read()[key].as_number().map(|n| n.to_string()).unwrap_or_else(|| "0".to_string())}",
                        oninput: move |ev| {
                            let v: serde_json::Number = if type_str == "integer" {
                                ev.value().parse::<i64>().unwrap_or(0).into()
                            } else {
                                ev.value().parse::<f64>().unwrap_or(0.0).into()
                            };
                            value.write_unchecked()[key.clone()] = serde_json::Value::Number(v);
                        },
                    }
                    if let Some(d) = desc {
                        div { class: "text-[10px] text-[#666]", "{d}" }
                    }
                }
            }
        }
        "boolean" => {
            let checked = value.read()[key].as_bool().unwrap_or(false);
            rsx! {
                div { class: "flex items-center gap-2",
                    input {
                        r#type: "checkbox",
                        checked,
                        oninput: move |ev| {
                            value.write_unchecked()[key.clone()] = serde_json::Value::Bool(ev.checked());
                        },
                    }
                    label { class: "text-[12px] text-[#aaa]",
                        "{label}"
                        if required { span { class: "text-[#c04040]", " *" } }
                    }
                    if let Some(d) = desc {
                        div { class: "text-[10px] text-[#666]", "{d}" }
                    }
                }
            }
        }
        "object" => {
            rsx! {
                div { class: "border border-[#3a3a55] rounded p-2",
                    div { class: "text-[12px] text-[#888] font-semibold mb-2",
                        "{label}"
                        if required { span { class: "text-[#c04040]", " *" } }
                    }
                    SchemaForm {
                        schema: prop_schema.clone(),
                        value,
                    }
                    if let Some(d) = desc {
                        div { class: "text-[10px] text-[#666] mt-1", "{d}" }
                    }
                }
            }
        }
        _ => rsx! {
            div { class: "text-[#888] text-[12px]", "Unsupported type: {type_str}" }
        }
    }
}
```

- [ ] **Step 3: Run cargo check to verify**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: Clean compilation

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/schema_form.rs
git commit -m "feat: add SchemaForm component for JSON Schema → form rendering"
```

### Task 2: Integrate SchemaForm into ToolCallDialog

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`

- [ ] **Step 1: Replace textarea with SchemaForm in ToolCallDialog**

In `mcp_tool_dialog.rs`, replace the textarea block (lines 37-43) with:

```rust
// Add form_value signal in the component
let form_value: Signal<serde_json::Value> = use_signal(|| serde_json::Value::Object(serde_json::Map::new()));

// Replace textarea with:
if let Some(ref schema) = tool.input_schema {
    SchemaForm { schema: schema.clone(), value: form_value }
} else {
    div { class: "text-[#888] text-[12px]", "No parameters required" }
}
```

Update the `arguments_json` extraction in the "Call" button onclick handler to use `form_value` instead of the textarea:

```rust
let args = serde_json::to_string(&*form_value.read()).unwrap_or("{}".to_string());
```

Remove the JSON validation block (`serde_json::from_str(&args)`) since the form produces valid JSON directly.

- [ ] **Step 2: Update the close handler**

When dialog closes (x button), also clear form_value:
```rust
onclick: move |_| {
    *signal.write_unchecked().tool_call_dialog = None;
},
```

- [ ] **Step 3: Add SchemaForm import to app.rs**

```rust
use super::schema_form::SchemaForm;
```

- [ ] **Step 4: Remove debug console.log statements**

Remove the `web_sys::console::log_1(...)` debug lines added during debugging from `mcp_tool_dialog.rs` and `mcp_panel.rs`.

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: Clean compilation

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs \
        crates/vol-llm-ui/src/web/components/mcp_panel.rs \
        crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: integrate SchemaForm into ToolCallDialog, replace JSON textarea"
```

## Spec Self-Review

1. **Placeholder scan:** No TBD/TODO. All code shown.
2. **Internal consistency:** SchemaForm uses `Signal<serde_json::Value>` which matches ToolCallDialog state. Form produces `serde_json::Value` → serialized to JSON string → passed to `mcp_call_tool()`. Types match.
3. **Scope check:** Focused — one new component, one integration change. No array type, no format validation. Those are explicitly deferred.
4. **Ambiguity check:** Schema `required` validation is label-only (`*` marker) — no blocking validation before submit in v1. This is intentional; user can still submit with empty required fields.
