# MCP Panel Dialog and Ordering Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make MCP Panel tool Call/Read/Get buttons functional and fix tool list shuffling on re-render.

**Architecture:** Create 3 new Dioxus component files (ToolCallDialog, ResourceViewer, PromptViewer), register them in mod.rs, wire them conditionally in McpPanel, and replace HashMap/BTreeMap in grouping logic.

**Tech Stack:** Dioxus 0.6 WASM, Signal<McpState>, JsonRpcClient, Tailwind CSS

---

### Task 1: Create ToolCallDialog component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs`

- [ ] **Step 1: Write the component file**

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
                        signal.write_unchecked().tool_call_dialog.as_mut().unwrap().arguments_json = ev.value();
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

- [ ] **Step 2: Verify it compiles as a standalone file**

Run: `cargo check -p vol-llm-ui --features web 2>&1 | grep mcp_tool_dialog`
Expected: No errors specific to this file (it will have mod.rs errors until Task 3 registers it)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_tool_dialog.rs
git commit -m "feat: add ToolCallDialog component for MCP panel tool invocation"
```

### Task 2: Create ResourceViewer and PromptViewer components

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs`
- Create: `crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs`

- [ ] **Step 1: Write ResourceViewer**

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

- [ ] **Step 2: Write PromptViewer**

```rust
use dioxus::prelude::*;
use crate::state::McpState;
use crate::web::components::app::AppState;

#[component]
pub fn PromptViewer(mut signal: Signal<McpState>, app_state: AppState) -> Element {
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

- [ ] **Step 3: Verify both files exist**

Run: `ls crates/vol-llm-ui/src/web/components/mcp_{resource_viewer,prompt_viewer}.rs`
Expected: Both files listed

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_resource_viewer.rs crates/vol-llm-ui/src/web/components/mcp_prompt_viewer.rs
git commit -m "feat: add ResourceViewer and PromptViewer components for MCP panel"
```

### Task 3: Register new modules and wire up McpPanel

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

- [ ] **Step 1: Register new modules in mod.rs**

Find the line `pub mod mcp_panel;` in `crates/vol-llm-ui/src/web/components/mod.rs` and add after it:

```rust
pub mod mcp_tool_dialog;
pub mod mcp_resource_viewer;
pub mod mcp_prompt_viewer;
```

- [ ] **Step 2: Add imports to mcp_panel.rs**

At the top of `crates/vol-llm-ui/src/web/components/mcp_panel.rs`, after the existing imports, add:

```rust
use super::mcp_tool_dialog::ToolCallDialog;
use super::mcp_resource_viewer::ResourceViewer;
use super::mcp_prompt_viewer::PromptViewer;
```

- [ ] **Step 3: Add conditional dialog renders in McpPanel**

Find the closing of the `match active { ... }` block inside `McpPanel` (the div wrapping the match). After the match block but still inside the outer `div { class: "flex-1 overflow-y-auto p-2", ... }`, add:

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

The full McpPanel rsx! body should look like:

```rust
rsx! {
    div { class: "flex-1 overflow-y-auto p-2",
        if loading {
            div { class: "text-[#666] text-center p-4 text-[13px]", "Loading MCP data..." }
        } else {
            div {
                div { class: "flex gap-1 mb-2",
                    McpSubtabButton { signal, subtab: McpSubtab::Servers, label: "Servers" }
                    McpSubtabButton { signal, subtab: McpSubtab::Tools, label: "Tools" }
                    McpSubtabButton { signal, subtab: McpSubtab::Resources, label: "Resources" }
                    McpSubtabButton { signal, subtab: McpSubtab::Prompts, label: "Prompts" }
                }
                match active {
                    McpSubtab::Servers => rsx! { ServerList { signal, app_state } },
                    McpSubtab::Tools => rsx! { ToolList { signal } },
                    McpSubtab::Resources => rsx! { ResourceList { signal } },
                    McpSubtab::Prompts => rsx! { PromptList { signal } },
                }
                if signal.read().tool_call_dialog.is_some() {
                    rsx! { ToolCallDialog { signal, app_state } }
                }
                if signal.read().resource_viewer.is_some() {
                    rsx! { ResourceViewer { signal, app_state } }
                }
                if signal.read().prompt_viewer.is_some() {
                    rsx! { PromptViewer { signal, app_state } }
                }
            }
        }
    }
}
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-ui --features web 2>&1 | tail -5`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mod.rs crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "feat: wire dialog components into McpPanel and register modules"
```

### Task 4: Fix HashMap ordering instability

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

- [ ] **Step 1: Replace HashMap with BTreeMap in ToolList**

Find this line in `ToolList` (around line 202):
```rust
let mut groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
```
Replace with:
```rust
let mut groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
```

- [ ] **Step 2: Replace HashMap/HashSet with BTreeMap/BTreeSet in ResourceList**

Find these lines in `ResourceList` (around lines 268-281):
```rust
let mut resource_groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
```
Replace with:
```rust
let mut resource_groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
```

```rust
let mut template_groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
```
Replace with:
```rust
let mut template_groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
```

```rust
let all_servers: std::collections::HashSet<String> = resource_groups.keys()
    .chain(template_groups.keys())
    .cloned()
    .collect();
```
Replace with:
```rust
let all_servers: std::collections::BTreeSet<String> = resource_groups.keys()
    .chain(template_groups.keys())
    .cloned()
    .collect();
```

- [ ] **Step 3: Replace HashMap with BTreeMap in PromptList**

Find this line in `PromptList` (around line 354):
```rust
let mut groups: std::collections::HashMap<String, Vec<_>> = std::collections::HashMap::new();
```
Replace with:
```rust
let mut groups: std::collections::BTreeMap<String, Vec<_>> = std::collections::BTreeMap::new();
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-ui --features web 2>&1 | tail -5`
Expected: No errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "fix: use BTreeMap/BTreeSet for stable MCP list ordering"
```

### Task 5: Final verification

**Files:** No code changes.

- [ ] **Step 1: Run full workspace check**

Run: `cargo check --workspace --features web -p vol-llm-ui 2>&1 | tail -10`
Expected: Clean compile (warnings OK)

- [ ] **Step 2: Serve and verify in browser**

Run: `dx serve --package vol-llm-ui --bin vol-llm-ui-web --no-default-features --features web --addr 0.0.0.0`
Expected: Server starts, no compile errors

Manual test steps after serve starts:
1. Open browser to http://localhost:8080
2. Click MCP tab
3. Click Tools sub-tab — verify server order is alphabetical and stable across re-renders
4. Click a tool's "Call" button — verify dialog appears with tool name and editable JSON args
5. Click "Call" in dialog — verify loading state, then result or error displayed
6. Click "x" to close dialog
7. Click Resources sub-tab — verify server order is alphabetical
8. Click a resource's "Read" button — verify dialog appears, click Read to fetch content
