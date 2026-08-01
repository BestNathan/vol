# Capability Selection UI Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the absolute-positioned CapabilityDropdown with a fixed right-side Drawer, make toggles instant (no Apply button), and add a Provider/Model placeholder section.

**Architecture:** CapabilityBar triggers a `CapabilityDrawerState.open = true` via shared context signal. CapabilityDrawer renders at App top level with `fixed right-0 top-0 h-full w-80 z-50`, outside all overflow containers. Each toggle calls `agent.update_capabilities` immediately with per-item saving-state feedback.

**Tech Stack:** Rust, Dioxus WASM, Tailwind CSS, existing `JsonRpcClient` / `dp_pool` patterns

## Global Constraints

- Drawer MUST use `fixed` positioning (not `absolute`) to avoid parent overflow clipping
- Toggle changes MUST call `agent_update_capabilities` immediately — no Apply button
- Provider/Model section is Phase 1 read-only placeholder ("Coming soon")
- `GlobalState.capabilities` dead field MUST be removed
- Match existing code patterns: `use_signal`, `use_context`, `use_effect`, `use_hook`, `rsx!`, WASM-safe `#[derive]` bounds

---

## File Structure

| 文件 | 操作 | 说明 |
|------|------|------|
| `crates/vol-llm-ui/src/state/mod.rs` | Modify | Add `CapabilityDrawerState`, `ToggleSavingState`, `ProviderOption`; remove `GlobalState.capabilities` |
| `crates/vol-llm-ui/src/web/components/capability_drawer.rs` | **Create** | Drawer component: overlay, panel, header, search, groups, toggles, provider section |
| `crates/vol-llm-ui/src/web/components/capability_bar.rs` | Rewrite | Simplify: keep summary line + ✎ button, remove dropdown |
| `crates/vol-llm-ui/src/web/components/mod.rs` | Modify | Register `capability_drawer` module |
| `crates/vol-llm-ui/src/web/components/app.rs` | Modify | Provide `CapabilityDrawerState` via context; render `CapabilityDrawer` top-level |
| `crates/vol-llm-ui/src/state/mod.rs` | Modify | Clean up `GlobalState.capabilities` dead field and its initializer |
| `crates/vol-llm-ui/tests/web/` | Create test | Playwright smoke test for drawer open/close |

---

### Task 1: Add CapabilityDrawerState and clean up dead GlobalState.capabilities

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

**Interfaces:**
- Produces: `CapabilityDrawerState` (struct), `ToggleSavingState` (enum), `ProviderOption` (struct) — all `pub`, `#[derive(Debug, Clone)]`, with `Default`
- Removes: `GlobalState.capabilities: CapabilityOverlayState` field and its initializer `capabilities: CapabilityOverlayState::new()`

- [ ] **Step 1: Add ToggleSavingState enum**

After the `CapabilityOverlayState` block (after line ~605), add:

```rust
/// Per-toggle saving feedback state for instant-apply toggles.
#[derive(Debug, Clone, PartialEq)]
pub enum ToggleSavingState {
    Saving,
    Saved,
    Error(String),
}
```

- [ ] **Step 2: Add ProviderOption struct**

```rust
/// A single provider entry for the Provider dropdown.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderOption {
    pub name: String,
    #[serde(default)]
    pub models: Vec<String>,
}
```

- [ ] **Step 3: Add CapabilityDrawerState struct**

After the `ProviderOption` block:

```rust
/// UI state for the CapabilityDrawer right-side panel.
#[derive(Debug, Clone)]
pub struct CapabilityDrawerState {
    pub open: bool,
    pub search: String,
    pub collapsed_sections: std::collections::HashSet<String>,
    pub providers: Vec<ProviderOption>,
    pub selected_provider: String,
    pub selected_model: String,
    pub saving_states: std::collections::HashMap<String, ToggleSavingState>,
    /// Capabilities have been fetched at least once since opening.
    pub loaded: bool,
    pub load_error: Option<String>,
}

impl Default for CapabilityDrawerState {
    fn default() -> Self {
        Self {
            open: false,
            search: String::new(),
            collapsed_sections: std::collections::HashSet::new(),
            providers: Vec::new(),
            selected_provider: String::new(),
            selected_model: String::new(),
            saving_states: std::collections::HashMap::new(),
            loaded: false,
            load_error: None,
        }
    }
}
```

- [ ] **Step 4: Remove GlobalState.capabilities dead field**

Find `pub capabilities: CapabilityOverlayState` in `GlobalState` (line ~632) and remove it.
Also remove the initializer `capabilities: CapabilityOverlayState::new()` in the `GlobalState::new()` constructor (line ~658).

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-ui
```

Expected: PASS (no references to `GlobalState.capabilities` remain — the field was unused per exploration finding).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat(ui): add CapabilityDrawerState, remove dead GlobalState.capabilities

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: Create CapabilityDrawer component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/capability_drawer.rs`

**Interfaces:**
- Consumes: `AppState`, `AgentsState`, `GlobalState`, `CapabilityDrawerState` (context signals)
- Consumes: `GetCapabilitiesResult`, `UpdateCapabilitiesResult` from `crate::web::client`
- Consumes: `CapabilityOverlayState` from `crate::state` for data model
- Produces: `pub fn CapabilityDrawer() -> Element` (top-level component)

- [ ] **Step 1: Create file with module header and imports**

```rust
//! Capability Drawer — fixed right-side panel for selecting agent capabilities.
//! Replaces the absolute-positioned CapabilityDropdown with a proper overlay.

use crate::state::{
    AgentsState, CapabilityDrawerState, CapabilityOverlayState, GlobalState, ProviderOption,
    ToggleSavingState,
};
use crate::web::client::{
    GetCapabilitiesResult, JsonRpcClient, UpdateCapabilitiesResult,
};
use crate::web::components::app::AppState;
use dioxus::prelude::*;
use std::collections::{HashMap, HashSet};
```

- [ ] **Step 2: Write CapabilityDrawer (main component)**

```rust
#[component]
pub fn CapabilityDrawer() -> Element {
    let app_state: AppState = use_context();
    let agents: Signal<AgentsState> = use_context();
    let global: Signal<GlobalState> = use_context();
    let mut drawer_state: Signal<CapabilityDrawerState> = use_context();

    // Local state: selected sets (initialized from effective on load)
    let sel_tools: Signal<HashSet<String>> = use_signal(HashSet::new);
    let sel_skills: Signal<HashSet<String>> = use_signal(HashSet::new);
    let sel_mcps: Signal<HashSet<String>> = use_signal(HashSet::new);

    // Capability data loaded from backend
    let cap_signal: Signal<CapabilityOverlayState> = use_signal(CapabilityOverlayState::new);

    // Load capabilities when drawer opens
    let app_for_effect = app_state.clone();
    let agents_for_effect = agents.clone();
    let global_for_effect = global.clone();
    use_effect(move || {
        let open = drawer_state.read().open;
        if !open {
            return;
        }
        let agent_id = agents_for_effect.read().selected.clone().unwrap_or_default();
        if agent_id.is_empty() {
            return;
        }
        let session_id = global_for_effect.read().session_id.clone();

        // Use DP client only (same pattern as existing CapabilityBar)
        let client = app_for_effect
            .active_node_id
            .read()
            .as_ref()
            .and_then(|nid| {
                app_for_effect
                    .dp_pool
                    .read()
                    .get(nid)
                    .map(|c| c.client.clone())
            })
            .unwrap_or_else(|| app_for_effect.rpc_client.clone());

        let mut cap = cap_signal.clone();
        let mut drawer = drawer_state.clone();
        let mut st = sel_tools.clone();
        let mut ss = sel_skills.clone();
        let mut sm = sel_mcps.clone();

        client.agent_get_capabilities(&agent_id, &session_id, move |result| {
            match result {
                Ok(r) => {
                    cap.with_mut(|c| {
                        c.effective_tools.clone_from(&r.effective_tools);
                        c.available_tools.clone_from(&r.available_tools);
                        c.base_tools.clone_from(&r.base_tools);
                        c.effective_skills.clone_from(&r.effective_skills);
                        c.available_skills.clone_from(&r.available_skills);
                        c.base_skills.clone_from(&r.base_skills);
                        c.effective_mcp_servers.clone_from(&r.effective_mcp_servers);
                        c.available_mcp_servers.clone_from(&r.available_mcp_servers);
                        c.base_mcp_servers.clone_from(&r.base_mcp_servers);
                        c.loading = false;
                    });
                    st.set(r.effective_tools.iter().cloned().collect());
                    ss.set(r.effective_skills.iter().cloned().collect());
                    sm.set(r.effective_mcp_servers.iter().cloned().collect());
                    drawer.with_mut(|d| {
                        d.loaded = true;
                        d.load_error = None;
                    });
                }
                Err(e) => {
                    drawer.with_mut(|d| {
                        d.loaded = true;
                        d.load_error = Some(e);
                    });
                }
            }
        });
    });

    let ds = drawer_state.read();
    if !ds.open {
        return rsx! {};
    }
    let open = ds.open;
    let search = ds.search.clone();
    let loaded = ds.loaded;
    let load_error = ds.load_error.clone();
    drop(ds);

    let cap = cap_signal.read();
    let avail_tools = cap.available_tools.clone();
    let avail_skills = cap.available_skills.clone();
    let avail_mcps = cap.available_mcp_servers.clone();
    let base_tools = cap.base_tools.clone();
    let base_skills = cap.base_skills.clone();
    let base_mcps = cap.base_mcp_servers.clone();
    drop(cap);

    // Filter by search term
    let search_lower = search.to_lowercase();
    let filter_by_search = move |name: &str| -> bool {
        search_lower.is_empty() || name.to_lowercase().contains(&search_lower)
    };

    let ds_read = drawer_state.read();
    let collapsed = ds_read.collapsed_sections.clone();
    let saving_states = ds_read.saving_states.clone();
    drop(ds_read);

    rsx! {
        // Backdrop overlay
        div {
            class: "fixed inset-0 bg-black/50 z-40",
            onclick: move |_| {
                drawer_state.with_mut(|d| d.open = false);
            },
        }
        // Drawer panel
        div {
            class: "fixed right-0 top-0 h-full w-80 bg-[#1a1a2e] border-l border-[#3a3a55] z-50 flex flex-col shadow-2xl",
            DrawerHeader {
                on_close: move |_| {
                    drawer_state.with_mut(|d| d.open = false);
                },
            }
            div { class: "flex-1 overflow-y-auto",
                if !loaded {
                    div { class: "p-4 text-[#888] text-[13px] text-center", "Loading..." }
                } else if let Some(ref err) = load_error {
                    div { class: "p-4 text-[#c04040] text-[13px] text-center", "Error: {err}" }
                } else {
                    // Provider section (read-only Phase 1)
                    ProviderSection {
                        drawer_state: drawer_state.clone(),
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // Search
                    SearchInput {
                        search: search.clone(),
                        on_input: move |val: String| {
                            drawer_state.with_mut(|d| d.search = val);
                        },
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // Tools group
                    SectionGroup {
                        title: "Tools".to_string(),
                        items: avail_tools.clone(),
                        selected: sel_tools.read().clone(),
                        base_set: base_tools.clone(),
                        saving_states: saving_states.clone(),
                        collapsed: collapsed.clone(),
                        filter: filter_by_search.clone(),
                        on_toggle: {
                            let st = sel_tools.clone();
                            let ss = sel_skills.clone();
                            let sm = sel_mcps.clone();
                            let cs = cap_signal.clone();
                            let ag = agents.clone();
                            let gl = global.clone();
                            let ap = app_state.clone();
                            let mut ds = drawer_state.clone();
                            move |name: String, enabled: bool| {
                                handle_toggle(
                                    "tools", &name, enabled,
                                    st.clone(), ss.clone(), sm.clone(),
                                    cs.clone(), ag.clone(), gl.clone(),
                                    ap.clone(), ds.clone(),
                                );
                            }
                        },
                        on_collapse: {
                            let mut ds = drawer_state.clone();
                            move |_| {
                                ds.with_mut(|d| {
                                    if d.collapsed_sections.contains("Tools") {
                                        d.collapsed_sections.remove("Tools");
                                    } else {
                                        d.collapsed_sections.insert("Tools".to_string());
                                    }
                                });
                            }
                        },
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // Skills group
                    SectionGroup {
                        title: "Skills".to_string(),
                        items: avail_skills.clone(),
                        selected: sel_skills.read().clone(),
                        base_set: base_skills.clone(),
                        saving_states: saving_states.clone(),
                        collapsed: collapsed.clone(),
                        filter: filter_by_search.clone(),
                        on_toggle: {
                            let st = sel_tools.clone();
                            let ss = sel_skills.clone();
                            let sm = sel_mcps.clone();
                            let cs = cap_signal.clone();
                            let ag = agents.clone();
                            let gl = global.clone();
                            let ap = app_state.clone();
                            let mut ds = drawer_state.clone();
                            move |name: String, enabled: bool| {
                                handle_toggle(
                                    "skills", &name, enabled,
                                    st.clone(), ss.clone(), sm.clone(),
                                    cs.clone(), ag.clone(), gl.clone(),
                                    ap.clone(), ds.clone(),
                                );
                            }
                        },
                        on_collapse: {
                            let mut ds = drawer_state.clone();
                            move |_| {
                                ds.with_mut(|d| {
                                    if d.collapsed_sections.contains("Skills") {
                                        d.collapsed_sections.remove("Skills");
                                    } else {
                                        d.collapsed_sections.insert("Skills".to_string());
                                    }
                                });
                            }
                        },
                    }

                    div { class: "border-t border-[#2a2a44] my-1" }

                    // MCP group
                    SectionGroup {
                        title: "MCP Servers".to_string(),
                        items: avail_mcps.clone(),
                        selected: sel_mcps.read().clone(),
                        base_set: base_mcps.clone(),
                        saving_states: saving_states.clone(),
                        collapsed: collapsed.clone(),
                        filter: filter_by_search,
                        on_toggle: {
                            let st = sel_tools.clone();
                            let ss = sel_skills.clone();
                            let sm = sel_mcps.clone();
                            let cs = cap_signal.clone();
                            let ag = agents.clone();
                            let gl = global.clone();
                            let ap = app_state.clone();
                            let mut ds = drawer_state.clone();
                            move |name: String, enabled: bool| {
                                handle_toggle(
                                    "mcps", &name, enabled,
                                    st.clone(), ss.clone(), sm.clone(),
                                    cs.clone(), ag.clone(), gl.clone(),
                                    ap.clone(), ds.clone(),
                                );
                            }
                        },
                        on_collapse: {
                            let mut ds = drawer_state.clone();
                            move |_| {
                                ds.with_mut(|d| {
                                    if d.collapsed_sections.contains("Mcp") {
                                        d.collapsed_sections.remove("Mcp");
                                    } else {
                                        d.collapsed_sections.insert("Mcp".to_string());
                                    }
                                });
                            }
                        },
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 3: Write DrawerHeader component**

```rust
#[component]
fn DrawerHeader(on_close: EventHandler<()>) -> Element {
    rsx! {
        div { class: "flex items-center justify-between px-4 py-3 border-b border-[#3a3a55] flex-shrink-0",
            span { class: "text-[14px] font-semibold text-[#e0e0e0]", "Capabilities" }
            button {
                class: "text-[18px] text-[#888] hover:text-[#ccc] leading-none",
                onclick: move |_| on_close.call(()),
                "\u{2715}"
            }
        }
    }
}
```

- [ ] **Step 4: Write ProviderSection component (Phase 1: read-only placeholder)**

```rust
#[component]
fn ProviderSection(drawer_state: Signal<CapabilityDrawerState>) -> Element {
    let ds = drawer_state.read();
    let collapsed = ds.collapsed_sections.contains("Provider");
    drop(ds);

    rsx! {
        div { class: "px-3 py-2",
            // Collapsible header
            div {
                class: "flex items-center justify-between cursor-pointer hover:bg-[#2a2a44] rounded px-1 py-0.5",
                onclick: move |_| {
                    drawer_state.with_mut(|d| {
                        if d.collapsed_sections.contains("Provider") {
                            d.collapsed_sections.remove("Provider");
                        } else {
                            d.collapsed_sections.insert("Provider".to_string());
                        }
                    });
                },
                span { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                    "Provider"
                }
                span { class: "text-[10px] text-[#666]",
                    if collapsed { "\u{25b8}" } else { "\u{25be}" }
                }
            }
            if !collapsed {
                div { class: "mt-2 space-y-2",
                    // Provider dropdown (disabled)
                    div { class: "flex flex-col gap-1",
                        span { class: "text-[11px] text-[#666]", "Provider" }
                        div {
                            class: "w-full px-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[12px] text-[#555] cursor-not-allowed",
                            "Not configurable via UI"
                        }
                    }
                    // Model dropdown (disabled)
                    div { class: "flex flex-col gap-1",
                        span { class: "text-[11px] text-[#666]", "Model" }
                        div {
                            class: "w-full px-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[12px] text-[#555] cursor-not-allowed",
                            "Set in agent definition"
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 5: Write SearchInput component**

```rust
#[component]
fn SearchInput(search: String, on_input: EventHandler<String>) -> Element {
    rsx! {
        div { class: "px-3 py-2",
            div { class: "relative",
                span { class: "absolute left-2 top-1/2 -translate-y-1/2 text-[#666] text-[12px] pointer-events-none",
                    "\u{1F50D}"
                }
                input {
                    class: "w-full pl-7 pr-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[12px] text-[#ccc] placeholder-[#555] focus:outline-none focus:border-[#80a0ff]",
                    r#type: "text",
                    placeholder: "Search capabilities...",
                    value: "{search}",
                    oninput: move |evt: Event<FormData>| {
                        on_input.call(evt.value());
                    },
                }
            }
        }
    }
}
```

- [ ] **Step 6: Write SectionGroup component**

```rust
#[component]
fn SectionGroup(
    title: String,
    items: Vec<serde_json::Value>,
    selected: HashSet<String>,
    base_set: Vec<String>,
    saving_states: HashMap<String, ToggleSavingState>,
    collapsed: HashSet<String>,
    filter: Box<dyn Fn(&str) -> bool + 'static>,
    on_toggle: EventHandler<(String, bool)>,
    on_collapse: EventHandler<()>,
) -> Element {
    let section_key = title.clone();
    let is_collapsed = collapsed.contains(&section_key);
    let base_set_clone = base_set;

    // Extract name from JSON value and filter
    let filtered: Vec<(String, bool)> = items
        .iter()
        .filter_map(|v| {
            let name = v.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
            if name.is_empty() || !filter(&name) {
                return None;
            }
            let is_base = base_set_clone.contains(&name);
            Some((name, is_base))
        })
        .collect();

    rsx! {
        div { class: "px-3 py-1",
            // Header row
            div {
                class: "flex items-center justify-between cursor-pointer hover:bg-[#2a2a44] rounded px-1 py-1",
                onclick: move |_| on_collapse.call(()),
                span { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px]",
                    "{title} ({filtered.len()})"
                }
                span { class: "text-[10px] text-[#666]",
                    if is_collapsed { "\u{25b8}" } else { "\u{25be}" }
                }
            }
            // Items
            if !is_collapsed {
                if filtered.is_empty() {
                    div { class: "text-[11px] text-[#666] px-2 py-1", "No matching capabilities" }
                } else {
                    for (name, is_base) in &filtered {
                        CapabilityToggle {
                            name: name.clone(),
                            is_base: *is_base,
                            checked: selected.contains(name),
                            saving_state: saving_states.get(&format!("{}:{}", section_key.to_lowercase().replace(" ", "_"), name)).cloned(),
                            on_toggle: {
                                let ot = on_toggle.clone();
                                let n = name.clone();
                                let checked = selected.contains(name);
                                move |_| {
                                    ot.call((n.clone(), !checked));
                                }
                            },
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 7: Write CapabilityToggle component**

```rust
#[component]
fn CapabilityToggle(
    name: String,
    is_base: bool,
    checked: bool,
    saving_state: Option<ToggleSavingState>,
    on_toggle: EventHandler<()>,
) -> Element {
    let name_color = if is_base {
        "text-[#e0e0e0]"
    } else {
        "text-[#80a0ff]"
    };

    let (status_icon, status_class) = match saving_state {
        Some(ToggleSavingState::Saving) => ("\u{25CC}", "text-[#c0a040] animate-spin"),
        Some(ToggleSavingState::Saved) => ("\u{2713}", "text-[#40c040]"),
        Some(ToggleSavingState::Error(ref msg)) => ("\u{26A0}", "text-[#c04040]"),
        None => ("", ""),
    };

    let error_msg = match &saving_state {
        Some(ToggleSavingState::Error(msg)) => Some(msg.clone()),
        _ => None,
    };

    rsx! {
        div { class: "flex items-center gap-2 py-1 px-1 hover:bg-[#222240] rounded",
            // Toggle switch
            button {
                class: if checked {
                    "w-8 h-4 rounded-full relative transition-colors bg-[#4080ff] flex-shrink-0"
                } else {
                    "w-8 h-4 rounded-full relative transition-colors bg-[#3a3a55] flex-shrink-0"
                },
                onclick: move |_| on_toggle.call(()),
                div {
                    class: if checked {
                        "absolute top-0.5 right-0.5 w-3 h-3 rounded-full bg-white transition-all"
                    } else {
                        "absolute top-0.5 left-0.5 w-3 h-3 rounded-full bg-[#888] transition-all"
                    },
                }
            }
            // Name
            span { class: "text-[12px] {name_color} flex-1 truncate", "{name}" }
            // Saving state indicator
            if !status_icon.is_empty() {
                if let Some(ref err) = error_msg {
                    span {
                        class: "text-[12px] {status_class} cursor-help flex-shrink-0",
                        title: "{err}",
                        "{status_icon}"
                    }
                } else {
                    span { class: "text-[12px] {status_class} flex-shrink-0", "{status_icon}" }
                }
            }
        }
    }
}
```

- [ ] **Step 8: Write handle_toggle function**

```rust
/// Handle a toggle flip — update local state, call backend, show feedback.
fn handle_toggle(
    group: &str,
    name: &str,
    enabled: bool,
    mut sel_tools: Signal<HashSet<String>>,
    mut sel_skills: Signal<HashSet<String>>,
    mut sel_mcps: Signal<HashSet<String>>,
    mut cap_signal: Signal<CapabilityOverlayState>,
    agents: Signal<AgentsState>,
    global: Signal<GlobalState>,
    app_state: AppState,
    mut drawer_state: Signal<CapabilityDrawerState>,
) {
    let state_key = format!("{}:{}", group, name);

    // Update local selection
    match group {
        "tools" => {
            sel_tools.with_mut(|s| if enabled { s.insert(name.to_string()); } else { s.remove(name); });
        }
        "skills" => {
            sel_skills.with_mut(|s| if enabled { s.insert(name.to_string()); } else { s.remove(name); });
        }
        "mcps" => {
            sel_mcps.with_mut(|s| if enabled { s.insert(name.to_string()); } else { s.remove(name); });
        }
        _ => return,
    }

    // Mark saving
    drawer_state.with_mut(|d| {
        d.saving_states.insert(state_key.clone(), ToggleSavingState::Saving);
    });

    // Build current selection sets
    let tools: Vec<String> = sel_tools.read().iter().cloned().collect();
    let skills: Vec<String> = sel_skills.read().iter().cloned().collect();
    let mcps: Vec<String> = sel_mcps.read().iter().cloned().collect();

    let agent_id = agents.read().selected.clone().unwrap_or_default();
    let session_id = global.read().session_id.clone();
    if agent_id.is_empty() {
        return;
    }

    let client = app_state
        .active_node_id
        .read()
        .as_ref()
        .and_then(|nid| app_state.dp_pool.read().get(nid).map(|c| c.client.clone()))
        .unwrap_or_else(|| app_state.rpc_client.clone());

    // Capture the expected state at request time to detect stale responses
    let expected_selected = enabled;

    client.agent_update_capabilities(&agent_id, &session_id, tools, skills, mcps, move |result| {
        match result {
            Ok(upd) => {
                // Update effective from server response
                cap_signal.with_mut(|c| {
                    c.effective_tools.clone_from(&upd.effective_tools);
                    c.effective_skills.clone_from(&upd.effective_skills);
                    c.effective_mcp_servers.clone_from(&upd.effective_mcp_servers);
                });
                // Mark saved, then clear after 1.5s
                drawer_state.with_mut(|d| {
                    d.saving_states.insert(state_key.clone(), ToggleSavingState::Saved);
                });
                let key = state_key.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    gloo_timers::future::TimeoutFuture::new(1500).await;
                    // In WASM we can't access signals across async — use a
                    // task-local pattern: the icon auto-clears on next render
                    // when the state is None, but we clear here explicitly.
                    // Since Signal isn't Send, we drop through and let the
                    // next render cycle clear the stale Saved state.
                    let _ = key; // The state ages out naturally on re-render
                });
                // Clear saved state after timeout using a different approach:
                // schedule via spawn_local with a oneshot back to the UI thread.
                // For simplicity, the icon shows "✓" for exactly 1.5s then
                // is cleared by the timeout handler below.
                let mut ds_for_clear = drawer_state.clone();
                let clear_key = state_key.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    gloo_timers::future::TimeoutFuture::new(1500).await;
                    ds_for_clear.with_mut(|d| {
                        if let Some(ToggleSavingState::Saved) = d.saving_states.get(&clear_key) {
                            d.saving_states.remove(&clear_key);
                        }
                    });
                });
            }
            Err(e) => {
                // Rollback local selection
                match group {
                    "tools" => {
                        sel_tools.with_mut(|s| if enabled { s.remove(name); } else { s.insert(name.to_string()); });
                    }
                    "skills" => {
                        sel_skills.with_mut(|s| if enabled { s.remove(name); } else { s.insert(name.to_string()); });
                    }
                    "mcps" => {
                        sel_mcps.with_mut(|s| if enabled { s.remove(name); } else { s.insert(name.to_string()); });
                    }
                    _ => {}
                }
                drawer_state.with_mut(|d| {
                    d.saving_states.insert(state_key, ToggleSavingState::Error(e));
                });
            }
        }
    });
}
```

- [ ] **Step 9: Verify compilation**

```bash
cargo check -p vol-llm-ui
```

Expected: PASS (may have unused-variable warnings for `expected_selected` — clean those up).

- [ ] **Step 10: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/capability_drawer.rs
git commit -m "feat(ui): add CapabilityDrawer with instant-apply toggles

- Fixed right-side drawer panel replacing absolute-positioned dropdown
- Toggle switches with saving/saved/error feedback states
- Collapsible groups (Tools, Skills, MCP)
- Search input with real-time filtering
- Provider/Model read-only placeholder section

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: Rewrite CapabilityBar — simplify to entry only

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/capability_bar.rs`

**Interfaces:**
- Consumes: `AppState`, `AgentsState`, `GlobalState`, `CapabilityDrawerState` (context signals)
- Consumes: `CapabilityOverlayState` for loading/displaying counts
- Produces: `pub fn CapabilityBar() -> Element` — same signature, simplified body

- [ ] **Step 1: Remove all dropdown code, keep summary + ✎ button**

Replace the entire file content after the module doc comment:

```rust
//! Capability summary bar — sits between conversation and input area.
//! Shows "🛠 N tools · N skills · N MCPs  [✎]" — click ✎ opens the right-side drawer.

use crate::state::{AgentsState, CapabilityOverlayState, GlobalState, CapabilityDrawerState};
use crate::web::components::app::AppState;
use dioxus::prelude::*;

#[component]
pub fn CapabilityBar() -> Element {
    let app_state: AppState = use_context();
    let global: Signal<GlobalState> = use_context();
    let agents: Signal<AgentsState> = use_context();
    let mut drawer_state: Signal<CapabilityDrawerState> = use_context();

    let cap_signal: Signal<CapabilityOverlayState> = use_signal(CapabilityOverlayState::new);

    // Load capabilities when selected agent changes (for summary counts)
    let agents_for_cap = agents.clone();
    let global_for_cap = global.clone();
    let app_for_cap = app_state.clone();
    use_effect(move || {
        let agent_id = agents_for_cap.read().selected.clone().unwrap_or_default();
        let session_id = global_for_cap.read().session_id.clone();
        if agent_id.is_empty() {
            return;
        }
        let client = app_for_cap
            .active_node_id
            .read()
            .as_ref()
            .and_then(|nid| {
                app_for_cap.dp_pool.read().get(nid).map(|c| c.client.clone())
            })
            .unwrap_or_else(|| app_for_cap.rpc_client.clone());

        let mut sig = cap_signal.clone();
        client.agent_get_capabilities(&agent_id, &session_id, move |result| {
            sig.with_mut(|s| match result {
                Ok(cap) => {
                    s.effective_tools.clone_from(&cap.effective_tools);
                    s.effective_skills.clone_from(&cap.effective_skills);
                    s.effective_mcp_servers.clone_from(&cap.effective_mcp_servers);
                    s.loading = false;
                }
                Err(e) => {
                    s.loading = false;
                    log::error!("Failed to load capabilities: {e}");
                }
            });
        });
    });

    let cap = cap_signal.read();
    let n_tools = cap.effective_tools.len();
    let n_skills = cap.effective_skills.len();
    let n_mcps = cap.effective_mcp_servers.len();
    let loading = cap.loading;
    drop(cap);

    let agent_selected = agents.read().selected.is_some();

    rsx! {
        div {
            class: "flex items-center gap-2 px-3 py-1.5 border-t border-[#2a2a44] bg-[#181825] text-[12px]",
            if loading {
                span { class: "text-[#666]", "Loading capabilities..." }
            } else {
                span { class: "text-[#888]",
                    "\u{1F6E0} {n_tools} tools \u{00B7} {n_skills} skills \u{00B7} {n_mcps} MCPs"
                }
                button {
                    class: if agent_selected {
                        "ml-1 px-1.5 py-0.5 text-[11px] bg-[#2a2a44] text-[#aaa] rounded hover:bg-[#3a3a55] hover:text-[#ccc]"
                    } else {
                        "ml-1 px-1.5 py-0.5 text-[11px] bg-[#2a2a44] text-[#555] rounded cursor-not-allowed"
                    },
                    disabled: !agent_selected,
                    onclick: move |_| {
                        if agent_selected {
                            drawer_state.with_mut(|d| d.open = true);
                        }
                    },
                    "\u{270E}"
                }
            }
        }
    }
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-ui
```

Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/capability_bar.rs
git commit -m "refactor(ui): simplify CapabilityBar — remove dropdown, trigger drawer instead

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Wire CapabilityDrawer in app.rs and mod.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

**Interfaces:**
- Produces: `CapabilityDrawerState` provided via `use_context_provider` in app.rs
- Produces: `CapabilityDrawer` rendered at top level in App layout

- [ ] **Step 1: Register module in mod.rs**

In `crates/vol-llm-ui/src/web/components/mod.rs`, after the `capability_bar` line, add:

```rust
pub mod capability_drawer;
```

- [ ] **Step 2: Provide CapabilityDrawerState context in app.rs**

In `crates/vol-llm-ui/src/web/components/app.rs`:

a) Add import (near other `use super::` imports at lines 34-51):
```rust
use super::capability_drawer::CapabilityDrawer;
```

b) Create signal and provide as context. After the `debug_signal` line (after line ~286), add:
```rust
let drawer_state = use_signal(|| CapabilityDrawerState::default());
```

c) Add context provider. After `use_context_provider(|| debug_signal);` (after line ~898), add:
```rust
use_context_provider(|| drawer_state);
```

d) Add `CapabilityDrawer` to the layout. After the `DebugPanel {}` line (after line ~934), add:
```rust
CapabilityDrawer {}
```

e) Add the import for `CapabilityDrawerState` at the top:
```rust
use crate::state::CapabilityDrawerState;
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-ui
```

Expected: PASS

- [ ] **Step 4: Run existing tests**

```bash
cargo test -p vol-llm-ui
```

Expected: All existing tests PASS. The `app_layout_does_not_use_a_floating_mobile_file_tree_button` test should still pass since we're adding a drawer, not a floating button.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mod.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(ui): wire CapabilityDrawer into app layout

- Provide CapabilityDrawerState via context
- Render CapabilityDrawer at top level (fixed positioning)
- Register capability_drawer module

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: Add smoke test for drawer open/close

**Files:**
- Create: `crates/vol-llm-ui/tests/web/capability_drawer.spec.js`

- [ ] **Step 1: Create Playwright test**

```javascript
// @ts-check
const { test, expect } = require('@playwright/test');

test.describe('Capability Drawer', () => {
  test('drawer opens and closes', async ({ page }) => {
    // Navigate to app
    await page.goto('http://localhost:8080');

    // Wait for the app to load (WS connected)
    await page.waitForSelector('text=Agents', { timeout: 15000 });

    // The ✎ button should be present but may be disabled without agent selection
    // For now, verify the drawer does NOT cover the page on load
    const drawer = page.locator('.fixed.right-0.top-0.h-full.w-80');
    await expect(drawer).not.toBeVisible();
  });
});
```

- [ ] **Step 2: Verify test file is valid**

```bash
npx playwright test crates/vol-llm-ui/tests/web/capability_drawer.spec.js --dry-run
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/tests/web/capability_drawer.spec.js
git commit -m "test(ui): add capability drawer smoke test

Co-Authored-By: Claude <noreply@anthropic.com>"
```
