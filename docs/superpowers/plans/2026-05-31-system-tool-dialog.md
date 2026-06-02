# System Tool Dialog & Fixed Ordering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Sort system tools alphabetically and add a modal dialog with SchemaForm for tool execution, replacing the broken inline "Run" button.

**Architecture:** Copy the MCP `ToolCallDialog` pattern — modal overlay + SchemaForm + Execute/Cancel. Server-side sort in `ToolHandler::List`. Client-side new `SystemToolDialog` component reuses existing `SchemaForm`. Run buttons in `tools_tab.rs` open the dialog instead of calling `client.tool_call` directly.

**Tech Stack:** Rust, Dioxus 0.6, Tailwind CSS v4, JSON Schema

---

### Task 1: Sort tools alphabetically on server side

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/tool.rs:46-57`

- [ ] **Step 1: Add sort after collect**

Replace lines 46-57:

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
                    a.get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("")
                        .cmp(b.get("name").and_then(|v| v.as_str()).unwrap_or(""))
                });
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Tool(ToolOperation::List),
                    Payload::Tool(ToolPayload::ListResult { tools }),
                )])
            }
```

- [ ] **Step 2: Verify compiles and test**

```bash
cargo test -p vol-llm-agent-channel 2>&1 | tail -5
```

Expected: all tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/tool.rs
git commit -m "feat(tool): sort tools alphabetically by name in list response"
```

---

### Task 2: Create SystemToolDialog component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/tool_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

- [ ] **Step 1: Create the dialog component**

New file `crates/vol-llm-ui/src/web/components/tool_dialog.rs`:

```rust
use dioxus::prelude::*;
use crate::web::components::app::AppState;
use super::schema_form::SchemaForm;

pub struct SystemToolDialogState {
    pub open: bool,
    pub tool_name: String,
    pub description: Option<String>,
    pub parameters: Option<serde_json::Value>,
    pub result: Option<String>,
    pub error: Option<String>,
    pub loading: bool,
}

impl SystemToolDialogState {
    pub fn new() -> Self {
        Self {
            open: false,
            tool_name: String::new(),
            description: None,
            parameters: None,
            result: None,
            error: None,
            loading: false,
        }
    }
}

#[component]
pub fn SystemToolDialog(mut signal: Signal<SystemToolDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();

    let mut form_value: Signal<serde_json::Value> =
        use_signal(|| serde_json::Value::Object(serde_json::Map::new()));

    // Read current dialog state
    let s = signal.read();
    if !s.open {
        return rsx! {};
    }
    let tool_name = s.tool_name.clone();
    let description = s.description.clone();
    let parameters = s.parameters.clone();
    let result = s.result.clone();
    let error = s.error.clone();
    let loading = s.loading;
    drop(s);

    // Initialize form defaults when parameters change
    let params_for_effect = parameters.clone();
    use_effect(move || {
        if let Some(ref schema) = params_for_effect {
            let defaults = build_form_defaults(schema);
            form_value.set(defaults);
        } else {
            form_value.set(serde_json::Value::Object(serde_json::Map::new()));
        }
    });

    rsx! {
        div {
            class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            onclick: move |_| {
                signal.write_unchecked().open = false;
            },
            div {
                class: "w-[95vw] sm:w-[600px] max-h-[85vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
                onclick: move |evt: Event<MouseData>| { evt.stop_propagation(); },
                // Header
                div { class: "flex items-center justify-between flex-shrink-0 px-4 pt-3 pb-2 border-b border-[#3a3a55]",
                    div { class: "min-w-0",
                        div { class: "text-[14px] font-semibold text-[#e0e0e0] truncate", "{tool_name}" }
                        if let Some(ref desc) = description {
                            div { class: "text-[11px] text-[#888] truncate mt-0.5", "{desc}" }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px] flex-shrink-0 ml-2",
                        onclick: move |_| { signal.write_unchecked().open = false; },
                        "x"
                    }
                }
                // Content
                div { class: "flex-1 min-h-0 overflow-y-auto px-4 pb-4 space-y-2",
                    // Schema form
                    if let Some(ref schema) = parameters {
                        SchemaForm { schema: schema.clone(), value: form_value }
                    } else {
                        div { class: "text-[#888] text-[12px] mt-2", "No parameters required" }
                    }

                    // Execute button
                    if !loading {
                        button {
                            class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                            onclick: move |_| {
                                let name = tool_name.clone();
                                let sig = signal.clone();
                                let client = rpc_client.clone();
                                let json_str = serde_json::to_string(&*form_value.read())
                                    .unwrap_or_else(|_| "{}".to_string());
                                let parsed: serde_json::Value = match serde_json::from_str(&json_str) {
                                    Ok(v) => v,
                                    Err(e) => {
                                        sig.write_unchecked().error = Some(format!("Invalid JSON: {e}"));
                                        return;
                                    }
                                };
                                sig.write_unchecked().loading = true;
                                sig.write_unchecked().error = None;
                                sig.write_unchecked().result = None;
                                client.tool_call(&name, &parsed, move |r| {
                                    match r {
                                        Ok(val) => {
                                            let content = val
                                                .get("result")
                                                .and_then(|r| r.get("content"))
                                                .and_then(|c| c.as_str())
                                                .unwrap_or_else(|| {
                                                    val.get("result")
                                                        .map(|r| r.to_string())
                                                        .as_deref()
                                                        .unwrap_or("(no output)")
                                                });
                                            sig.write_unchecked().result = Some(content.to_string());
                                        }
                                        Err(e) => {
                                            sig.write_unchecked().error = Some(e);
                                        }
                                    }
                                    sig.write_unchecked().loading = false;
                                });
                            },
                            "Execute"
                        }
                    } else {
                        div { class: "mt-2 text-[#888] text-[13px]", "Running..." }
                    }

                    // Result
                    if let Some(ref result_text) = result {
                        div { class: "bg-[#1a2a1a] border border-[#40c040] rounded p-2 mt-2",
                            div { class: "text-[11px] text-[#40c040] font-semibold mb-1", "Result" }
                            pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-words overflow-x-auto", "{result_text}" }
                        }
                    }

                    // Error
                    if let Some(ref error_text) = error {
                        div { class: "bg-[#2a1a1a] border border-[#c04040] rounded p-2 mt-2",
                            div { class: "text-[11px] text-[#c04040] font-semibold mb-1", "Error" }
                            div { class: "text-[12px] text-[#e0e0e0] break-words", "{error_text}" }
                        }
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

- [ ] **Step 2: Register module in mod.rs**

In `crates/vol-llm-ui/src/web/components/mod.rs`, add:

```rust
pub mod tool_dialog;
```

- [ ] **Step 3: Verify compiles**

```bash
cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | grep "^error" | head -5
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tool_dialog.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat(ui): add SystemToolDialog component with SchemaForm for tool execution"
```

---

### Task 3: Wire Run buttons to open dialog

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_tab.rs`

- [ ] **Step 1: Add dialog state and SystemToolDialog to ToolsTabContent**

In `ToolsTabContent`, add the dialog state signal and render the dialog:

After the existing `tool_state` signal (around line 72), add:

```rust
let dialog_state = use_signal(|| super::tool_dialog::SystemToolDialogState::new());
```

At the end of the `rsx!` block (after the closing `}` of the outer div), add:

```rust
SystemToolDialog { signal: dialog_state }
```

You'll need to import:
```rust
use super::tool_dialog::{SystemToolDialog, SystemToolDialogState};
```

- [ ] **Step 2: Rewrite Run button onclick in mobile tool cards**

Replace the Run button in the mobile card section:

```rust
// Old (inside mobile tool card loop):
button {
    class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
    onclick: {
        let client = client.clone();
        let ts = tool_state;
        let name = tool.name.clone();
        move |_| {
            let args_val = serde_json::json!({});
            let ts = ts;
            client.tool_call(&name, &args_val, move |result| {
                safe_write(ts, |s| {
                    match result {
                        Ok(val) => s.call_result = Some(
                            serde_json::to_string_pretty(&val).unwrap_or_else(|_| val.to_string())
                        ),
                        Err(e) => s.call_result = Some(format!("Error: {e}")),
                    }
                });
            });
        }
    },
    "Run"
}
```

Replace with:

```rust
// New: open dialog
button {
    class: "px-2 py-0.5 text-[11px] bg-[#4080ff] text-white rounded hover:bg-[#5090ff] flex-shrink-0 ml-2",
    onclick: {
        let ds = dialog_state;
        let name = tool.name.clone();
        let desc = tool.description.clone();
        let params = tool.parameters.clone();
        move |_| {
            ds.with_mut(|s| {
                s.open = true;
                s.tool_name = name.clone();
                s.description = desc.clone();
                s.parameters = params.clone();
                s.result = None;
                s.error = None;
                s.loading = false;
            });
        }
    },
    "Run"
}
```

- [ ] **Step 3: Rewrite Run button onclick in desktop tool rows**

Same replacement — find the other Run button in the `hidden sm:block` section and apply the same change.

- [ ] **Step 4: Remove unused call_result display**

Remove the inline `call_result` display block (the `if let Some(ref result) = call_result` section) since results now appear in the dialog. Remove the `call_result` from the state reads too if it's only used there.

- [ ] **Step 5: Verify compiles**

```bash
cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | grep "^error" | head -5
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_tab.rs
git commit -m "feat(ui): wire Run buttons to SystemToolDialog instead of inline tool_call"
```

---

### Task 4: Visual verification

- [ ] **Step 1: Build CSS and check**

```bash
npx --prefix crates/vol-llm-ui @tailwindcss/cli \
  -i crates/vol-llm-ui/assets/input.css \
  -o crates/vol-llm-ui/assets/tailwind.css --minify
```

- [ ] **Step 2: Start dev servers and test**

```bash
# Terminal 1: make web-css
# Terminal 2: make web-dev  
# Terminal 3: make web-backend
```

Open `http://localhost:8080`, go to Tools tab:
1. Verify tools are sorted alphabetically
2. Click "Run" on a tool → dialog opens with SchemaForm
3. Fill in arguments → click Execute → result appears
4. Click "Run" on a tool with no parameters → form is empty → Execute works
5. Click ✕ or backdrop → dialog closes
6. Test on small screen (< 480px) — card layout "Run" also opens dialog

- [ ] **Step 3: Commit any visual fixes**

```bash
git add -A && git commit -m "fix(ui): visual verification fixes for system tool dialog"
```

---

## Testing Strategy

- **Server**: `cargo test -p vol-llm-agent-channel` — existing tests cover ToolHandler
- **Client**: WASM build check + browser visual verification
- **Edge cases**: tool with no parameters, tool with complex nested parameters, tool execution error
