# JSON Schema Form Component Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the raw JSON textarea in `ToolCallDialog` with a form auto-generated from the tool's `input_schema` JSON Schema.

**Architecture:** New `SchemaForm` component reads `properties` from a JSON Schema and renders typed input fields (text, number, checkbox, select). A `Signal<serde_json::Value>` collects form data. `ToolCallDialog` passes the schema to `SchemaForm` and serializes the form value for the RPC call. `McpToolCallState` gains an `input_schema` field to carry the schema from `ToolCard` to the dialog.

**Tech Stack:** Dioxus 0.6 (web), serde_json, TailwindCSS

---

### Task 1: Add `input_schema` field to `McpToolCallState`

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add `input_schema` field to `McpToolCallState`**

In `crates/vol-llm-ui/src/state/mod.rs`, find `McpToolCallState` and add the `input_schema` field:

```rust
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Clone, Debug)]
pub struct McpToolCallState {
    pub server: String,
    pub tool_name: String,
    pub arguments_json: String,
    pub result: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
    pub input_schema: Option<serde_json::Value>,
}
```

- [ ] **Step 2: Update `ToolCard` onclick handler to pass `input_schema`**

In `crates/vol-llm-ui/src/web/components/mcp_panel.rs`, find the `ToolCard` component's onclick handler (around line 236-246). Replace the `McpToolCallState` construction to include `input_schema`:

```rust
onclick: move |_| {
    let t = tool.clone();
    signal.write_unchecked().tool_call_dialog = Some(crate::state::McpToolCallState {
        server: t.server.clone(),
        tool_name: t.name.clone(),
        arguments_json: t.input_schema.as_ref().map(|v| serde_json::to_string_pretty(v).unwrap_or_default()).unwrap_or_else(|| "{}".to_string()),
        result: None,
        error: None,
        loading: false,
        input_schema: t.input_schema.clone(),
    });
},
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: Clean compilation

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "feat: add input_schema field to McpToolCallState for SchemaForm"
```

### Task 2: Create `SchemaForm` component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/schema_form.rs`

- [ ] **Step 1: Create `schema_form.rs` with `build_defaults`, `SchemaForm`, and `SchemaField`**

Create the file `crates/vol-llm-ui/src/web/components/schema_form.rs` with this complete content:

```rust
use dioxus::prelude::*;

/// Build a default `serde_json::Value::Object` from a JSON Schema.
fn build_defaults(schema: &serde_json::Value) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    let Some(props) = schema.get("properties").and_then(|v| v.as_object()) else {
        return serde_json::Value::Object(obj);
    };
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
    serde_json::Value::Object(obj)
}

/// Renders a form from a JSON Schema.
///
/// `schema` — the JSON Schema object (with `properties` and optional `required`).
/// `value` — shared signal holding the current form data as `serde_json::Value`.
#[component]
pub fn SchemaForm(schema: serde_json::Value, value: Signal<serde_json::Value>) -> Element {
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

/// Renders a single form field based on its JSON Schema property definition.
#[component]
fn SchemaField(key: String, prop_schema: serde_json::Value, value: Signal<serde_json::Value>, required: bool) -> Element {
    let type_str = prop_schema.get("type").and_then(|t| t.as_str()).unwrap_or("string");
    let label = prop_schema.get("title").and_then(|t| t.as_str()).unwrap_or(&key);
    let desc = prop_schema.get("description").and_then(|t| t.as_str());

    match type_str {
        "string" => render_string_field(&key, label, desc, &prop_schema, value, required),
        "number" | "integer" => render_number_field(&key, label, desc, type_str, value, required),
        "boolean" => render_boolean_field(&key, label, desc, value, required),
        "object" => render_object_field(&key, label, desc, &prop_schema, value, required),
        _ => rsx! {
            div { class: "text-[#888] text-[12px]", "Unsupported type: {type_str}" }
        },
    }
}

fn render_string_field(key: &str, label: &str, desc: Option<&str>, prop_schema: &serde_json::Value, value: Signal<serde_json::Value>, required: bool) -> Element {
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
                        value.write_unchecked()[key.to_string()] = serde_json::Value::String(ev.value());
                    },
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
                        value.write_unchecked()[key.to_string()] = serde_json::Value::String(ev.value());
                    },
                }
                if let Some(d) = desc {
                    div { class: "text-[10px] text-[#666]", "{d}" }
                }
            }
        }
    }
}

fn render_number_field(key: &str, label: &str, desc: Option<&str>, type_str: &str, value: Signal<serde_json::Value>, required: bool) -> Element {
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
                    let key = key.to_string();
                    let v: serde_json::Number = if type_str == "integer" {
                        ev.value().parse::<i64>().unwrap_or(0).into()
                    } else {
                        ev.value().parse::<f64>().unwrap_or(0.0).into()
                    };
                    value.write_unchecked()[key] = serde_json::Value::Number(v);
                },
            }
            if let Some(d) = desc {
                div { class: "text-[10px] text-[#666]", "{d}" }
            }
        }
    }
}

fn render_boolean_field(key: &str, label: &str, desc: Option<&str>, value: Signal<serde_json::Value>, required: bool) -> Element {
    rsx! {
        div { class: "flex items-center gap-2",
            input {
                r#type: "checkbox",
                checked: value.read()[key].as_bool().unwrap_or(false),
                oninput: move |ev| {
                    value.write_unchecked()[key.to_string()] = serde_json::Value::Bool(ev.checked());
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

fn render_object_field(key: &str, label: &str, desc: Option<&str>, prop_schema: &serde_json::Value, value: Signal<serde_json::Value>, required: bool) -> Element {
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
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: Clean compilation

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/schema_form.rs
git commit -m "feat: add SchemaForm component for JSON Schema → form rendering"
```

### Task 3: Integrate `SchemaForm` into `ToolCallDialog`

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Rewrite `ToolCallDialog` to use `SchemaForm`**

Replace the entire content of `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs` with:

```rust
use dioxus::prelude::*;
use crate::state::McpDialogState;
use crate::web::components::app::AppState;
use super::schema_form::SchemaForm;

#[component]
pub fn ToolCallDialog(mut signal: Signal<McpDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();

    let form_value: Signal<serde_json::Value> = use_signal(|| serde_json::Value::Object(serde_json::Map::new()));

    let maybe_dialog = {
        let s = signal.read();
        s.tool_call_dialog.as_ref().map(|d| (
            d.server.clone(),
            d.tool_name.clone(),
            d.input_schema.clone(),
        ))
    };

    let Some((server, tool_name, input_schema)) = maybe_dialog else {
        return rsx! {};
    };

    // Re-initialize form when schema changes
    use_effect(move || {
        if let Some(ref schema) = input_schema {
            let defaults = build_form_defaults(schema);
            form_value.set(defaults);
        } else {
            form_value.set(serde_json::Value::Object(serde_json::Map::new()));
        }
    });

    let (result, error, loading) = {
        let s = signal.read();
        (
            s.tool_call_dialog.as_ref().and_then(|d| d.result.clone()),
            s.tool_call_dialog.as_ref().and_then(|d| d.error.clone()),
            s.tool_call_dialog.as_ref().map(|d| d.loading).unwrap_or(false),
        )
    };

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[500px] max-w-[90vw] max-h-[80vh] flex flex-col",
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[14px] font-semibold text-[#e0e0e0]", "{server} / {tool_name}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px]",
                        onclick: move |_| { signal.write_unchecked().tool_call_dialog = None; },
                        "x"
                    }
                }
                if let Some(ref schema) = input_schema {
                    SchemaForm { schema: schema.clone(), value: form_value }
                } else {
                    div { class: "text-[#888] text-[12px]", "No parameters required" }
                }
                if !loading {
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let s = signal.clone();
                            let client = rpc_client.clone();
                            let (srv, tool, args) = {
                                let r = s.read();
                                let d = r.tool_call_dialog.as_ref().unwrap();
                                let form_json = serde_json::to_string(&*form_value.read()).unwrap_or("{}".to_string());
                                (d.server.clone(), d.tool_name.clone(), form_json)
                            };
                            let parsed: serde_json::Value = match serde_json::from_str(&args) {
                                Ok(v) => v,
                                Err(e) => {
                                    s.write_unchecked().tool_call_dialog.as_mut().unwrap().error = Some(format!("Invalid form data: {e}"));
                                    return;
                                }
                            };
                            let sig = s;
                            sig.write_unchecked().tool_call_dialog.as_mut().unwrap().loading = true;
                            sig.write_unchecked().tool_call_dialog.as_mut().unwrap().error = None;
                            sig.write_unchecked().tool_call_dialog.as_mut().unwrap().result = None;
                            client.mcp_call_tool(&srv, &tool, parsed, move |r| {
                                match r {
                                    Ok(content) => {
                                        sig.write_unchecked().tool_call_dialog.as_mut().unwrap().result = Some(content);
                                    }
                                    Err(e) => {
                                        sig.write_unchecked().tool_call_dialog.as_mut().unwrap().error = Some(e);
                                    }
                                }
                                sig.write_unchecked().tool_call_dialog.as_mut().unwrap().loading = false;
                            });
                        },
                        "Call"
                    }
                } else {
                    div { class: "mt-2 text-[#888] text-[13px]", "Calling..." }
                }
                if let Some(ref result) = result {
                    div { class: "mt-3 bg-[#1a2a1a] border border-[#40c040] rounded p-2 max-h-48 overflow-y-auto",
                        div { class: "text-[11px] text-[#40c040] font-semibold mb-1", "Result" }
                        pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-all", "{result}" }
                    }
                }
                if let Some(ref error) = error {
                    div { class: "mt-3 bg-[#2a1a1a] border border-[#c04040] rounded p-2",
                        div { class: "text-[11px] text-[#c04040] font-semibold mb-1", "Error" }
                        div { class: "text-[12px] text-[#e0e0e0]", "{error}" }
                    }
                }
            }
        }
    }
}

fn build_form_defaults(schema: &serde_json::Value) -> serde_json::Value {
    let mut obj = serde_json::Map::new();
    let Some(props) = schema.get("properties").and_then(|v| v.as_object()) else {
        return serde_json::Value::Object(obj);
    };
    for (key, prop) in props {
        let default = if let Some(d) = prop.get("default") {
            d.clone()
        } else {
            match prop.get("type").and_then(|t| t.as_str()) {
                Some("string") => serde_json::Value::String(String::new()),
                Some("number") | Some("integer") => serde_json::Value::Number(0.into()),
                Some("boolean") => serde_json::Value::Bool(false),
                Some("object") => build_form_defaults(prop),
                _ => serde_json::Value::Null,
            }
        };
        obj.insert(key.clone(), default);
    }
    serde_json::Value::Object(obj)
}
```

- [ ] **Step 2: Remove debug console.log from `mcp_panel.rs`**

In `crates/vol-llm-ui/src/web/components/mcp_panel.rs`, remove the debug line from the `ToolCard` onclick handler:

Find and remove this line:
```rust
web_sys::console::log_1(&format!("ToolCard click: {}/{}", tool.server, tool.name).into());
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: Clean compilation

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs \
        crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "feat: integrate SchemaForm into ToolCallDialog, replace JSON textarea"
```
