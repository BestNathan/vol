# Capability Overlay — Move to Conversation Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move capability overlay from per-panel sections to a compact summary row between conversation messages and the input box, with a dropdown picker for detailed adjustment.

**Architecture:** New `CapabilityBar` component renders a summary row (`🛠 N tools · N skills · N MCPs [✎]`) wired into the Conversation sub-tab layout between `ConversationView` and `InputArea`. Clicking `[✎]` opens a `CapabilityDropdown` floating panel with grouped checkboxes. The existing per-panel overlay sections are removed.

**Tech Stack:** Dioxus 0.6 RSX, existing `CapabilityOverlayState` + `JsonRpcClient` RPC methods.

---

### Task 1: Create CapabilityBar component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/capability_bar.rs`

- [ ] **Step 1: Create the component file**

```rust
//! Capability summary bar — sits between conversation and input area.
//! Shows "🛠 N tools · N skills · N MCPs  [✎]" with a dropdown picker.

use crate::state::{AgentsState, CapabilityOverlayState, GlobalState};
use crate::web::client::JsonRpcClient;
use crate::web::components::app::AppState;
use dioxus::prelude::*;
use std::collections::HashSet;

/// Dropdown visibility signal type.
type DropdownSignal = Signal<bool>;

#[component]
pub fn CapabilityBar() -> Element {
    let app_state: AppState = use_context();
    let global: Signal<GlobalState> = use_context();
    let agents: Signal<AgentsState> = use_context();

    let cap_signal: Signal<CapabilityOverlayState> = use_signal(CapabilityOverlayState::new);
    let dropdown_open: Signal<bool> = use_signal(|| false);

    // Load capabilities when selected agent changes
    let agents_for_cap = agents.clone();
    let global_for_cap = global.clone();
    let app_for_cap = app_state.clone();
    use_effect(move || {
        let agent_id = agents_for_cap.read().selected.clone().unwrap_or_default();
        let session_id = global_for_cap.read().session_id.clone();
        if agent_id.is_empty() {
            cap_signal.with_mut(|s| s.loading = false);
            return;
        }
        let node_id = app_for_cap.active_node_id.read().clone();
        let client = node_id
            .as_ref()
            .and_then(|nid| app_for_cap.dp_pool.read().get(nid).map(|c| c.client.clone()))
            .unwrap_or_else(|| app_for_cap.rpc_client.clone());
        let sig = cap_signal.clone();
        client.agent_get_capabilities(&agent_id, &session_id, move |result| {
            let mut sig = sig;
            sig.with_mut(|s| match result {
                Ok(cap) => {
                    s.effective_tools.clone_from(&cap.effective_tools);
                    s.available_tools = cap.available_tools;
                    s.base_tools = cap.base_tools;
                    s.effective_skills.clone_from(&cap.effective_skills);
                    s.available_skills = cap.available_skills;
                    s.base_skills = cap.base_skills;
                    s.effective_mcp_servers.clone_from(&cap.effective_mcp_servers);
                    s.available_mcp_servers = cap.available_mcp_servers;
                    s.base_mcp_servers = cap.base_mcp_servers;
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

    rsx! {
        div {
            class: "flex items-center gap-2 px-3 py-1.5 border-t border-[#2a2a44] bg-[#181825] text-[12px]",
            if loading {
                span { class: "text-[#666]", "Loading capabilities..." }
            } else {
                span { class: "text-[#888]",
                    "🛠 {n_tools} tools · {n_skills} skills · {n_mcps} MCPs"
                }
                button {
                    class: "ml-1 px-1.5 py-0.5 text-[11px] bg-[#2a2a44] text-[#aaa] rounded hover:bg-[#3a3a55] hover:text-[#ccc]",
                    onclick: move |_| dropdown_open.set(!dropdown_open()),
                    "✎"
                }
            }
        }
        // Dropdown
        if dropdown_open() {
            CapabilityDropdown {
                cap_signal,
                agents,
                global,
                app_state,
                dropdown_open,
            }
        }
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: errors about missing `CapabilityDropdown` — expected, builds in next task.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/capability_bar.rs
git commit -m "feat(ui): add CapabilityBar component skeleton"
```

---

### Task 2: Create CapabilityDropdown component

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/capability_bar.rs` (add CapabilityDropdown)

- [ ] **Step 1: Add CapabilityDropdown component**

Add to the same file:

```rust
#[component]
fn CapabilityDropdown(
    cap_signal: Signal<CapabilityOverlayState>,
    agents: Signal<AgentsState>,
    global: Signal<GlobalState>,
    app_state: AppState,
    dropdown_open: Signal<bool>,
) -> Element {
    let sel_tools: Signal<HashSet<String>> = {
        let cap = cap_signal.read();
        use_signal(|| cap.effective_tools.iter().cloned().collect())
    };
    let sel_skills: Signal<HashSet<String>> = {
        let cap = cap_signal.read();
        use_signal(|| cap.effective_skills.iter().cloned().collect())
    };
    let sel_mcps: Signal<HashSet<String>> = {
        let cap = cap_signal.read();
        use_signal(|| cap.effective_mcp_servers.iter().cloned().collect())
    };
    let dirty: Signal<bool> = use_signal(|| false);

    let cap = cap_signal.read();

    rsx! {
        div {
            class: "absolute bottom-full left-0 right-0 mx-3 mb-1 bg-[#1e1e2e] border border-[#3a3a55] rounded-lg shadow-xl max-h-[60vh] overflow-y-auto z-50",
            // Click outside to close
            div {
                class: "fixed inset-0 z-[-1]",
                onclick: move |_| dropdown_open.set(false),
            }
            div { class: "p-3",
                // Header
                div { class: "flex items-center justify-between mb-3",
                    div { class: "text-[13px] font-semibold text-[#ccc]", "Capabilities" }
                    button {
                        class: "text-[18px] text-[#888] hover:text-[#ccc] leading-none",
                        onclick: move |_| dropdown_open.set(false),
                        "×"
                    }
                }

                // Tools group
                div { class: "mb-3",
                    div { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px] mb-1", "Tools" }
                    for tool in &cap.available_tools {
                        {
                            let name = tool.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let is_base = cap.base_tools.contains(&name);
                            let label = if is_base { name.clone() } else { format!("+{}", name) };
                            let chk = sel_tools.read().contains(&name);
                            rsx! {
                                div { class: "flex items-center gap-2 py-0.5",
                                    input {
                                        r#type: "checkbox",
                                        checked: chk,
                                        oninput: {
                                            let st = sel_tools.clone();
                                            let d = dirty.clone();
                                            let n = name.clone();
                                            move |_| {
                                                let mut st = st;
                                                let mut d = d;
                                                let mut hs = st.write();
                                                if hs.contains(&n) { hs.remove(&n); } else { hs.insert(n.clone()); }
                                                d.set(true);
                                            }
                                        },
                                    }
                                    span { class: "text-[12px] text-[#ccc]", "{label}" }
                                }
                            }
                        }
                    }
                }

                // Divider
                div { class: "border-t border-[#2a2a44] my-2" }

                // Skills group
                div { class: "mb-3",
                    div { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px] mb-1", "Skills" }
                    for skill in &cap.available_skills {
                        {
                            let name = skill.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let is_base = cap.base_skills.contains(&name);
                            let label = if is_base { name.clone() } else { format!("+{}", name) };
                            let chk = sel_skills.read().contains(&name);
                            rsx! {
                                div { class: "flex items-center gap-2 py-0.5",
                                    input {
                                        r#type: "checkbox",
                                        checked: chk,
                                        oninput: {
                                            let ss = sel_skills.clone();
                                            let d = dirty.clone();
                                            let n = name.clone();
                                            move |_| {
                                                let mut ss = ss;
                                                let mut d = d;
                                                let mut hs = ss.write();
                                                if hs.contains(&n) { hs.remove(&n); } else { hs.insert(n.clone()); }
                                                d.set(true);
                                            }
                                        },
                                    }
                                    span { class: "text-[12px] text-[#ccc]", "{label}" }
                                }
                            }
                        }
                    }
                }

                // Divider
                div { class: "border-t border-[#2a2a44] my-2" }

                // MCP group
                div { class: "mb-3",
                    div { class: "text-[11px] font-semibold text-[#888] uppercase tracking-[0.5px] mb-1", "MCP Servers" }
                    for server in &cap.available_mcp_servers {
                        {
                            let name = server.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            let is_base = cap.base_mcp_servers.contains(&name);
                            let label = if is_base { name.clone() } else { format!("+{}", name) };
                            let chk = sel_mcps.read().contains(&name);
                            rsx! {
                                div { class: "flex items-center gap-2 py-0.5",
                                    input {
                                        r#type: "checkbox",
                                        checked: chk,
                                        oninput: {
                                            let sm = sel_mcps.clone();
                                            let d = dirty.clone();
                                            let n = name.clone();
                                            move |_| {
                                                let mut sm = sm;
                                                let mut d = d;
                                                let mut hs = sm.write();
                                                if hs.contains(&n) { hs.remove(&n); } else { hs.insert(n.clone()); }
                                                d.set(true);
                                            }
                                        },
                                    }
                                    span { class: "text-[12px] text-[#ccc]", "{label}" }
                                }
                            }
                        }
                    }
                }

                // Action buttons
                let d = *dirty.read();
                div { class: "flex gap-2 mt-2 pt-2 border-t border-[#2a2a44]",
                    button {
                        class: "px-3 py-1 text-[12px] bg-[#4a4aff] text-white rounded hover:bg-[#5a5aff] disabled:opacity-40 disabled:cursor-not-allowed",
                        disabled: !d,
                        onclick: {
                            let st = sel_tools.clone();
                            let ss = sel_skills.clone();
                            let sm = sel_mcps.clone();
                            let cs = cap_signal.clone();
                            let ag = agents.clone();
                            let gl = global.clone();
                            let app = app_state.clone();
                            let dd = dropdown_open.clone();
                            let dirty_sig = dirty.clone();
                            move |_| {
                                let agent_id = ag.read().selected.clone().unwrap_or_default();
                                let session_id = gl.read().session_id.clone();
                                if agent_id.is_empty() { return; }
                                let node_id = app.active_node_id.read().clone();
                                let client = node_id
                                    .as_ref()
                                    .and_then(|nid| app.dp_pool.read().get(nid).map(|c| c.client.clone()))
                                    .unwrap_or_else(|| app.rpc_client.clone());
                                let tools: Vec<String> = st.read().iter().cloned().collect();
                                let skills: Vec<String> = ss.read().iter().cloned().collect();
                                let mcps: Vec<String> = sm.read().iter().cloned().collect();
                                let mut cs = cs;
                                let mut dirty_sig = dirty_sig;
                                let dd = dd;
                                client.agent_update_capabilities(&agent_id, &session_id, tools, skills, mcps, move |result| {
                                    cs.with_mut(|s| match result {
                                        Ok(upd) => {
                                            s.effective_tools.clone_from(&upd.effective_tools);
                                            s.effective_skills.clone_from(&upd.effective_skills);
                                            s.effective_mcp_servers.clone_from(&upd.effective_mcp_servers);
                                            s.dirty = false;
                                            dirty_sig.set(false);
                                        }
                                        Err(e) => { log::error!("Failed to update capabilities: {e}"); }
                                    });
                                });
                                dd.set(false);
                            }
                        },
                        "Apply"
                    }
                    button {
                        class: "px-3 py-1 text-[12px] bg-[#3a3a55] text-[#ccc] rounded hover:bg-[#4a4a65]",
                        onclick: {
                            let st = sel_tools.clone();
                            let ss = sel_skills.clone();
                            let sm = sel_mcps.clone();
                            let cs = cap_signal.clone();
                            let d = dirty.clone();
                            move |_| {
                                let mut st = st;
                                let mut ss = ss;
                                let mut sm = sm;
                                let mut d = d;
                                let c = cs.read();
                                st.set(c.base_tools.iter().cloned().collect());
                                ss.set(c.base_skills.iter().cloned().collect());
                                sm.set(c.base_mcp_servers.iter().cloned().collect());
                                d.set(true);
                            }
                        },
                        "Reset to default"
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/capability_bar.rs
git commit -m "feat(ui): add CapabilityDropdown with grouped checkboxes and Apply/Reset"
```

---

### Task 3: Wire CapabilityBar into Conversation sub-tab

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs:426-428`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs` (add module)

- [ ] **Step 1: Add module declaration**

In `crates/vol-llm-ui/src/web/components/mod.rs`, add after other pub mod declarations:

```rust
pub mod capability_bar;
```

- [ ] **Step 2: Wire into Conversation sub-tab**

In `crates/vol-llm-ui/src/web/components/agents_panel.rs`, import:

```rust
use super::capability_bar::CapabilityBar;
```

Change the Conversation sub-tab layout from:

```rust
AgentSubTab::Conversation => rsx! {
    ConversationView {}
    InputArea {}
},
```

To:

```rust
AgentSubTab::Conversation => rsx! {
    ConversationView {}
    CapabilityBar {}
    InputArea {}
},
```

- [ ] **Step 3: Build check**

Run: `cargo check -p vol-llm-ui --no-default-features --features web`
Expected: compiles cleanly.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/
git commit -m "feat(ui): wire CapabilityBar into Conversation sub-tab"
```

---

### Task 4: Remove capability overlay sections from panel files

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/skills.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

- [ ] **Step 1: Remove from tools_panel.rs**

Delete the entire Capability Overlay section — from the divider line through the closing `}` of the capability `div`. Also remove the capability-specific imports (`AgentsState`, `CapabilityOverlayState`, `GlobalState`, `HashSet` if not used elsewhere), and the signals/hooks (`cap_signal`, `selected_tools_signal`, `cap_dirty`, `global`, `agents`, `client_for_cap`, `global_for_cap`, `agents_for_cap`, the entire `use_effect` block for capabilities), and the pre-rsx capability reads (`cap_state`, `cap_loading`, `cap_eff`, etc.).

Run: `cargo check -p vol-llm-ui --no-default-features --features web` to verify.

- [ ] **Step 2: Remove from skills.rs**

Same cleanup as tools_panel.rs — remove capability overlay section, imports, signals, hooks, and pre-rsx reads.

Run: `cargo check -p vol-llm-ui --no-default-features --features web` to verify.

- [ ] **Step 3: Remove from mcp_panel.rs**

Same cleanup.

Run: `cargo check -p vol-llm-ui --no-default-features --features web` to verify.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_panel.rs crates/vol-llm-ui/src/web/components/skills.rs crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "refactor(ui): remove capability overlay sections from panel files"
```

---

### Task 5: Final verification

**Files:** all changed

- [ ] **Step 1: Full build**

Run: `cargo build -p vol-agent-server`
Expected: compiles.

- [ ] **Step 2: Run all tests**

Run: `cargo test -p vol-agent-server -p vol-llm-ui`
Expected: all pass.

- [ ] **Step 3: Pre-commit check**

Run: `./.githooks/pre-commit` (stage a file first to trigger checks)
Expected: all checks pass.

- [ ] **Step 4: Commit**

```bash
git commit -m "chore: final verification for capability overlay move"
```

---

### Task 6 (optional): Remove dirty-tracking pre-rsx reads

If Task 4 left any dead code in the panel files (unused imports, leftover variables), clean up with `cargo fix --lib -p vol-llm-ui` and commit.
