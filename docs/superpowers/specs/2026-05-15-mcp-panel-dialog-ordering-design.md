# MCP Panel Tool Dialog and Ordering Fix

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make MCP Panel tool Call/Read/Get buttons functional and fix tool list shuffling on re-render.

**Architecture:** Extract 3 new components from McpPanel into separate files, wire them to render when state is set. Replace HashMap with BTreeMap for stable server ordering.

**Tech Stack:** Dioxus 0.6 WASM, existing McpState signal, existing JsonRpcClient

---

## File Structure

| File | Action | Responsibility |
|------|--------|----------------|
| `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs` | Create | ToolCallDialog — show tool name, editable args, Call button, result/error output |
| `crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs` | Create | ResourceViewer — show URI, Read button, content output |
| `crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs` | Create | PromptViewer — show prompt name, args, Get button, result output |
| `crates/vol-llm-ui/src/web/components/mcp_panel.rs` | Modify | Import 3 new components, render them conditionally, replace HashMap with BTreeMap |

## Component Boundaries

Each viewer component receives: `Signal<McpState>` + `AppState` (for rpc_client). It reads/writes the relevant field from McpState directly (tool_call_dialog, resource_viewer, prompt_viewer). This avoids duplicating state — the McpState signal owns all data.

### Task 1: Create mcp_tool_dialog.rs

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`

```rust
use dioxus::prelude::*;
use crate::state::McpState;
use crate::web::components::app::AppState;

#[component]
pub fn ToolCallDialog(mut signal: Signal<McpState>, app_state: AppState) -> Element {
    let rpc_client = app_state.rpc_client.clone();

    let (server, tool_name, args, result, error, loading) = {
        let s = signal.read();
        let dialog = s.tool_call_dialog.as_ref()?;
        (
            dialog.server.clone(),
            dialog.tool_name.clone(),
            dialog.arguments_json.clone(),
            dialog.result.clone(),
            dialog.error.clone(),
            dialog.loading,
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
                textarea {
                    class: "w-full h-32 bg-[#252540] border border-[#3a3a55] rounded p-2 text-[12px] text-[#e0e0e0] font-mono resize-none",
                    value: "{args}",
                    oninput: move |ev| {
                        let new_args = ev.value();
                        signal.write_unchecked().tool_call_dialog.as_mut().unwrap().arguments_json = new_args;
                    },
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
                                (d.server.clone(), d.tool_name.clone(), d.arguments_json.clone())
                            };
                            // Validate JSON first
                            let parsed: serde_json::Value = match serde_json::from_str(&args) {
                                Ok(v) => v,
                                Err(e) => {
                                    s.write_unchecked().tool_call_dialog.as_mut().unwrap().error = Some(format!("Invalid JSON: {e}"));
                                    return;
                                }
                            };
                            let mut sig = s;
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
```

### Task 2: Create mcp_resource_viewer.rs

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs`

```rust
use dioxus::prelude::*;
use crate::state::McpState;
use crate::web::components::app::AppState;

#[component]
pub fn ResourceViewer(mut signal: Signal<McpState>, app_state: AppState) -> Element {
    let rpc_client = app_state.rpc_client.clone();

    let (uri, content, error, loading) = {
        let s = signal.read();
        let viewer = s.resource_viewer.as_ref()?;
        (
            viewer.uri.clone(),
            viewer.content.clone(),
            viewer.error.clone(),
            viewer.loading,
        )
    };

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[500px] max-w-[90vw] max-h-[80vh] flex flex-col",
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[14px] font-semibold text-[#e0e0e0] truncate", "Resource: {uri}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px] ml-2",
                        onclick: move |_| { signal.write_unchecked().resource_viewer = None; },
                        "x"
                    }
                }
                if !loading && content.is_none() {
                    button {
                        class: "px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let s = signal.clone();
                            let client = rpc_client.clone();
                            let u = { let r = s.read(); r.resource_viewer.as_ref().unwrap().uri.clone() };
                            let mut sig = s;
                            sig.write_unchecked().resource_viewer.as_mut().unwrap().loading = true;
                            sig.write_unchecked().resource_viewer.as_mut().unwrap().error = None;
                            client.mcp_read_resource(&u, move |r| {
                                match r {
                                    Ok(c) => { sig.write_unchecked().resource_viewer.as_mut().unwrap().content = Some(c); }
                                    Err(e) => { sig.write_unchecked().resource_viewer.as_mut().unwrap().error = Some(e); }
                                }
                                sig.write_unchecked().resource_viewer.as_mut().unwrap().loading = false;
                            });
                        },
                        "Read"
                    }
                } else if loading {
                    div { class: "text-[#888] text-[13px]", "Loading..." }
                }
                if let Some(ref content) = content {
                    div { class: "mt-3 bg-[#252540] border border-[#3a3a55] rounded p-2 max-h-64 overflow-y-auto flex-1",
                        pre { class: "text-[12px] text-[#e0e0e0] font-mono whitespace-pre-wrap break-all", "{content}" }
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
```

### Task 3: Create mcp_prompt_viewer.rs

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs`

```rust
use dioxus::prelude::*;
use crate::state::McpState;
use crate::web::components::app::AppState;

#[component]
pub fn PromptViewer(mut signal: Signal<McpState>, app_state: AppState) -> Element {
    let rpc_client = app_state.rpc_client.clone();

    let (server, prompt_name, args, result, error, loading) = {
        let s = signal.read();
        let viewer = s.prompt_viewer.as_ref()?;
        (
            viewer.server.clone(),
            viewer.prompt_name.clone(),
            viewer.args_json.clone(),
            viewer.result.clone(),
            viewer.error.clone(),
            viewer.loading,
        )
    };

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[500px] max-w-[90vw] max-h-[80vh] flex flex-col",
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[14px] font-semibold text-[#e0e0e0]", "Prompt: {prompt_name}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px]",
                        onclick: move |_| { signal.write_unchecked().prompt_viewer = None; },
                        "x"
                    }
                }
                div { class: "text-[12px] text-[#888]", "Server: {server}" }
                textarea {
                    class: "w-full h-24 bg-[#252540] border border-[#3a3a55] rounded p-2 text-[12px] text-[#e0e0e0] font-mono resize-none mt-2",
                    value: "{args}",
                    oninput: move |ev| {
                        signal.write_unchecked().prompt_viewer.as_mut().unwrap().args_json = ev.value();
                    },
                }
                if !loading {
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[13px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            signal.write_unchecked().prompt_viewer.as_mut().unwrap().error = Some("mcp.get_prompt not implemented yet".to_string());
                        },
                        "Get"
                    }
                } else {
                    div { class: "mt-2 text-[#888] text-[13px]", "Loading..." }
                }
                if let Some(ref result) = result {
                    div { class: "mt-3 bg-[#1a2a1a] border border-[#40c040] rounded p-2 max-h-48 overflow-y-auto",
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
```

### Task 4: Wire up McpPanel, fix HashMap ordering, and register modules

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

Changes to **mod.rs** — add 3 new module declarations and re-exports:
```rust
pub mod mcp_tool_dialog;
pub mod mcp_resource_viewer;
pub mod mcp_prompt_viewer;
```

Changes to **mcp_panel.rs**:
1. No mod declarations needed here (components are in mod.rs)
2. Import the new components at the top of the file:
```rust
use super::mcp_tool_dialog::ToolCallDialog;
use super::mcp_resource_viewer::ResourceViewer;
use super::mcp_prompt_viewer::PromptViewer;
```
3. In `McpPanel` rendering, after the sub-tab content `match` block, add conditional renders before the closing `div`:
```rust
if signal.read().tool_call_dialog.is_some() {
    rsx! { ToolCallDialog { signal, app_state } }
}
if signal.read().resource_viewer.is_some() {
    rsx! { ResourceViewer { signal, app_state } }
}
if signal.read().prompt_viewer.is_some() {
    rsx! { PromptViewer { signal, app_state } }
}
```
4. In `ToolList`, `ResourceList`, `PromptList`, replace `HashMap` with `BTreeMap` for stable alphabetical server ordering:
```rust
// ToolList line ~202:
let mut groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();

// ResourceList lines ~268, ~273 — also replace HashSet with BTreeSet:
let mut resource_groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
let mut template_groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
let all_servers: std::collections::BTreeSet<String> = resource_groups.keys()
    .chain(template_groups.keys())
    .cloned()
    .collect();

// PromptList line ~354:
let mut groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
```

5. In `PromptViewer` (Task 3 code) — fix the destructuring to use separate let bindings instead of invalid named tuple syntax:
```rust
let viewer = s.prompt_viewer.as_ref()?;
let server = viewer.server.clone();
let prompt_name = viewer.prompt_name.clone();
let args = viewer.args_json.clone();
let result = viewer.result.clone();
let error = viewer.error.clone();
let loading = viewer.loading;
```
