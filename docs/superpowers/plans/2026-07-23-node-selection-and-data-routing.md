# Node Selection and Data Routing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the UI from CP-centric to DP-centric with node selection, per-node caching, and DP-scoped data routing.

**Architecture:** Centralized `NodeDataCache` stores per-node data for all DP-scoped tabs. `active_node_id` signal drives cache lookups and tab re-renders. Nodes dropdown in status bar enables node selection. Each DP connection has its own event loop.

**Tech Stack:** Rust, Dioxus (WASM), WebSocket JSON-RPC, Tailwind CSS

---

## File Structure

### New Files
- `crates/vol-llm-ui/src/state/node_data_cache.rs` — Per-node data cache (`NodeDataCache`, `NodeData`)
- `crates/vol-llm-ui/src/web/components/nodes_dropdown.rs` — Collapsible node selection dropdown
- `crates/vol-llm-ui/src/web/components/node_detail_panel.rs` — CP-scoped node detail view

### Modified Files
- `crates/vol-llm-ui/src/state/mod.rs` — Export `NodeDataCache`, add `viewing_node_detail` signal
- `crates/vol-llm-ui/src/web/components/app.rs` — Integrate `NodeDataCache`, route to `NodeDetailPanel`, update `AppState`
- `crates/vol-llm-ui/src/web/components/status_bar.rs` — Embed `NodesDropdown`
- `crates/vol-llm-ui/src/web/dp_connection.rs` — Auto-start event loop on connection creation
- `crates/vol-llm-ui/src/web/client.rs` — Add `node_get()`, `capability_list()` methods
- `crates/vol-llm-ui/src/web/components/file_tree.rs` — Route through `NodeDataCache`
- `crates/vol-llm-ui/src/web/components/mcp_panel.rs` — Route through `NodeDataCache`
- `crates/vol-llm-ui/src/web/components/tools_tab.rs` — Route through `NodeDataCache`
- `crates/vol-llm-ui/src/web/components/tasks_panel.rs` — Route through `NodeDataCache`
- `crates/vol-llm-ui/src/web/components/skills.rs` — Route through `NodeDataCache`
- `crates/vol-llm-ui/src/web/components/log_viewer.rs` — Route through `NodeDataCache`

### Test Files
- `crates/vol-llm-ui/tests/node_data_cache.rs` — Unit tests for cache
- `crates/vol-llm-ui/tests/node_selection.rs` — Integration tests for node selection flow

---

## Phase 1: NodeDataCache Infrastructure (2-3 hours)

### Task 1.1: Implement NodeDataCache with TDD

**Files:**
- Create: `crates/vol-llm-ui/src/state/node_data_cache.rs`
- Create: `crates/vol-llm-ui/tests/node_data_cache.rs`

- [ ] **Step 1: Write failing test for empty cache**

Create `crates/vol-llm-ui/tests/node_data_cache.rs`:

```rust
use vol_llm_ui::state::node_data_cache::NodeDataCache;

#[test]
fn test_get_returns_none_for_missing_node() {
    let cache = NodeDataCache::default();
    assert!(cache.get("nonexistent").is_none());
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --test node_data_cache -- --nocapture`
Expected: FAIL — `NodeDataCache` not found

- [ ] **Step 3: Create NodeDataCache struct**

Create `crates/vol-llm-ui/src/state/node_data_cache.rs`:

```rust
use std::collections::HashMap;

/// Per-node cached data for all DP-scoped tabs.
#[derive(Debug, Clone, Default)]
pub struct NodeData {
    pub files: Option<crate::state::WorkspaceState>,
    pub mcp: Option<crate::state::McpState>,
    pub tools: Option<crate::state::ToolState>,
    pub tasks: Option<crate::state::TaskState>,
    pub skills: Option<crate::state::SkillsState>,
    pub logs: Option<crate::state::LogsState>,
}

/// Centralized cache: node_id → NodeData.
#[derive(Debug, Clone, Default)]
pub struct NodeDataCache {
    cache: HashMap<String, NodeData>,
}

impl NodeDataCache {
    pub fn get(&self, node_id: &str) -> Option<&NodeData> {
        self.cache.get(node_id)
    }
}
```

- [ ] **Step 4: Export NodeDataCache in state/mod.rs**

Edit `crates/vol-llm-ui/src/state/mod.rs`, add at line 1:

```rust
mod node_data_cache;
pub use node_data_cache::{NodeData, NodeDataCache};
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo test --test node_data_cache`
Expected: PASS — `test_get_returns_none_for_missing_node`

- [ ] **Step 6: Write failing test for get_or_insert**

Add to `crates/vol-llm-ui/tests/node_data_cache.rs`:

```rust
#[test]
fn test_get_or_insert_creates_entry() {
    let mut cache = NodeDataCache::default();
    let data = cache.get_or_insert("node-A");
    data.files = Some(vol_llm_ui::state::WorkspaceState::new("."));
    
    assert!(cache.get("node-A").is_some());
    assert!(cache.get("node-A").unwrap().files.is_some());
}
```

- [ ] **Step 7: Run test to verify it fails**

Run: `cargo test --test node_data_cache`
Expected: FAIL — `get_or_insert` method not found

- [ ] **Step 8: Implement get_or_insert**

Edit `crates/vol-llm-ui/src/state/node_data_cache.rs`, add method:

```rust
impl NodeDataCache {
    pub fn get_or_insert(&mut self, node_id: &str) -> &mut NodeData {
        self.cache.entry(node_id.to_string()).or_default()
    }
}
```

- [ ] **Step 9: Run test to verify it passes**

Run: `cargo test --test node_data_cache`
Expected: PASS — both tests pass

- [ ] **Step 10: Write failing test for invalidate**

Add to `crates/vol-llm-ui/tests/node_data_cache.rs`:

```rust
#[test]
fn test_invalidate_removes_entry() {
    let mut cache = NodeDataCache::default();
    cache.get_or_insert("node-A");
    cache.invalidate("node-A");
    
    assert!(cache.get("node-A").is_none());
}
```

- [ ] **Step 11: Run test to verify it fails**

Run: `cargo test --test node_data_cache`
Expected: FAIL — `invalidate` method not found

- [ ] **Step 12: Implement invalidate**

Edit `crates/vol-llm-ui/src/state/node_data_cache.rs`, add method:

```rust
impl NodeDataCache {
    pub fn invalidate(&mut self, node_id: &str) {
        self.cache.remove(node_id);
    }
}
```

- [ ] **Step 13: Run test to verify it passes**

Run: `cargo test --test node_data_cache`
Expected: PASS — all 3 tests pass

- [ ] **Step 14: Commit**

```bash
git add crates/vol-llm-ui/src/state/node_data_cache.rs crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/tests/node_data_cache.rs
git commit -m "feat(state): add NodeDataCache for per-node data caching

Implements centralized cache with get/get_or_insert/invalidate methods.
Enables per-node data persistence and instant switching between nodes.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 1.2: Integrate NodeDataCache into AppState

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs:80-89` (AppState struct)

- [ ] **Step 1: Add node_data_cache field to AppState**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, update AppState struct (around line 82):

```rust
pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,
    pub active_tab: Signal<ActiveTab>,
    pub cp_client: JsonRpcClient,
    pub dp_pool: Signal<DpConnectionPool>,
    pub active_node_id: Signal<Option<String>>,
    pub node_data_cache: Signal<NodeDataCache>,  // ← NEW
}
```

- [ ] **Step 2: Initialize node_data_cache in App component**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, add signal initialization (around line 256):

```rust
let node_data_cache = use_signal(|| NodeDataCache::new());
```

- [ ] **Step 3: Pass node_data_cache to AppState provider**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, update use_context_provider (around line 697):

```rust
use_context_provider(|| AppState {
    event_bus: event_bus.with(|eb| eb.clone()),
    rpc_client: client.clone(),
    active_tab,
    cp_client: client.clone(),
    dp_pool,
    active_node_id,
    node_data_cache,  // ← NEW
});
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(app): integrate NodeDataCache into AppState

Adds node_data_cache signal to enable per-node data caching across all tabs.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 2: Nodes Dropdown UI (3-4 hours)

### Task 2.1: Add node_get and capability_list to JsonRpcClient

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add NodeRecord type**

Edit `crates/vol-llm-ui/src/web/client.rs`, add after NodeListEntry (around line 50):

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct NodeRecord {
    pub node_id: String,
    pub name: String,
    pub version: String,
    pub status: String,
    #[serde(default)]
    pub last_seen_at_ms: Option<u64>,
    #[serde(default)]
    pub capability_revision: u64,
    #[serde(default)]
    pub load: Option<NodeLoadInfo>,
}
```

- [ ] **Step 2: Add node_get method**

Edit `crates/vol-llm-ui/src/web/client.rs`, add after `node_list` method (around line 200):

```rust
pub fn node_get(&self, node_id: &str, cb: impl FnOnce(Result<Option<NodeRecord>, String>) + 'static) {
    let params = serde_json::json!({ "node_id": node_id });
    self.send_request("control.node_get", params, move |result| {
        cb(result.and_then(|v| {
            serde_json::from_value(v)
                .map(|r: NodeRecord| Some(r))
                .map_err(|e| e.to_string())
        }));
    });
}
```

- [ ] **Step 3: Add CapabilitySnapshot type**

Edit `crates/vol-llm-ui/src/web/client.rs`, add after NodeRecord:

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilitySnapshot {
    pub node_id: String,
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub skills: Vec<String>,
    #[serde(default)]
    pub mcp_servers: Vec<String>,
}
```

- [ ] **Step 4: Add capability_list method**

Edit `crates/vol-llm-ui/src/web/client.rs`, add after `node_get`:

```rust
pub fn capability_list(&self, node_id: Option<&str>, cb: impl FnOnce(Result<Vec<CapabilitySnapshot>, String>) + 'static) {
    let params = serde_json::json!({ "node_id": node_id });
    self.send_request("control.capability_list", params, move |result| {
        cb(result.and_then(|v| {
            serde_json::from_value(v).map_err(|e| e.to_string())
        }));
    });
}
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat(client): add node_get and capability_list methods

Enables fetching detailed node info and capabilities from control-plane.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2.2: Implement NodesDropdown component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/nodes_dropdown.rs`

- [ ] **Step 1: Create NodesDropdown struct**

Create `crates/vol-llm-ui/src/web/components/nodes_dropdown.rs`:

```rust
use dioxus::prelude::*;
use crate::web::client::NodeListEntry;

#[component]
pub fn NodesDropdown(
    nodes: Vec<NodeListEntry>,
    selected_node_id: Signal<Option<String>>,
    on_select: EventHandler<String>,
) -> Element {
    let mut is_open = use_signal(|| false);
    
    let selected_name = selected_node_id.read().as_ref().and_then(|id| {
        nodes.iter().find(|n| &n.node_id == id).map(|n| n.name.clone())
    }).unwrap_or_else(|| "None".to_string());
    
    rsx! {
        div { class: "relative",
            button {
                class: "flex items-center gap-1 px-2 py-0.5 text-[11px] bg-[#2a2a44] hover:bg-[#3a3a55] rounded cursor-pointer",
                onclick: move |_| is_open.toggle(),
                span { class: "text-[#80a0ff]", "▾" }
                span { class: "text-[#e0e0e0]", "Nodes({nodes.len()})" }
            }
            
            if *is_open.read() {
                div { class: "absolute right-0 top-full mt-1 w-80 bg-[#1e1e36] border border-[#333355] rounded shadow-lg z-50",
                    div { class: "px-3 py-2 border-b border-[#333355] text-[12px] font-bold text-[#80a0ff]", "Nodes" }
                    div { class: "max-h-60 overflow-y-auto",
                        for node in nodes.iter() {
                            NodeRow {
                                node: node.clone(),
                                is_selected: selected_node_id.read().as_ref() == Some(&node.node_id),
                                on_click: {
                                    let node_id = node.node_id.clone();
                                    let mut is_open_sig = is_open;
                                    move |_| {
                                        on_select.call(node_id.clone());
                                        is_open_sig.set(false);
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn NodeRow(
    node: NodeListEntry,
    is_selected: bool,
    on_click: EventHandler<()>,
) -> Element {
    let status_color = if node.status == "online" { "bg-green-500" } else { "bg-red-500" };
    let row_class = if is_selected {
        "flex items-center gap-2 px-3 py-2 hover:bg-[#2a2a44] cursor-pointer bg-[#1a2a44]"
    } else {
        "flex items-center gap-2 px-3 py-2 hover:bg-[#2a2a44] cursor-pointer"
    };
    
    rsx! {
        div { class: "{row_class}", onclick: move |_| on_click.call(()),
            div { class: "w-2 h-2 rounded-full {status_color} flex-shrink-0" }
            div { class: "flex-1 min-w-0",
                div { class: "text-[#e0e0e0] text-[12px] font-medium truncate", "{node.name}" }
                div { class: "text-[#888] text-[10px]", "{node.node_id} · v{node.version}" }
            }
            if let Some(ref load) = node.load {
                div { class: "text-[#888] text-[10px] flex-shrink-0",
                    if let Some(cpu) = load.cpu {
                        span { "cpu:{cpu:.0}%" }
                    }
                }
            }
            if is_selected {
                span { class: "text-[#80a0ff] text-[10px] flex-shrink-0", "✓" }
            }
        }
    }
}
```

- [ ] **Step 2: Export NodesDropdown in components/mod.rs**

Edit `crates/vol-llm-ui/src/web/components/mod.rs`, add:

```rust
pub mod nodes_dropdown;
pub use nodes_dropdown::NodesDropdown;
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/nodes_dropdown.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat(ui): add NodesDropdown component

Collapsible dropdown showing all nodes with status, version, and load.
Supports node selection with visual indicator for selected node.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2.3: Integrate NodesDropdown into StatusBar

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/status_bar.rs`

- [ ] **Step 1: Import NodesDropdown**

Edit `crates/vol-llm-ui/src/web/components/status_bar.rs`, add at top:

```rust
use super::nodes_dropdown::NodesDropdown;
use crate::web::client::NodeListEntry;
```

- [ ] **Step 2: Add nodes state to StatusBar**

Edit `crates/vol-llm-ui/src/web/components/status_bar.rs`, add after `let debug = ...` (around line 18):

```rust
let mut nodes = use_signal(Vec::<NodeListEntry>::new);
let app = app_state.clone();

// Fetch nodes on mount
use_effect(move || {
    let cp = app.cp_client.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let (tx, rx) = futures_channel::oneshot::channel();
        cp.node_list(move |result| {
            let _ = tx.send(result);
        });
        if let Ok(Ok(n)) = rx.await {
            nodes.set(n);
        }
    });
});
```

- [ ] **Step 3: Add NodesDropdown to StatusBar layout**

Edit `crates/vol-llm-ui/src/web/components/status_bar.rs`, replace the existing "DP: node-A" section (around line 64-74) with:

```rust
// DP indicator
if let Some(ref node_id) = *app_state.active_node_id.read() {
    span { class: "flex items-center gap-1 mr-1",
        span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #40c040; box-shadow: 0 0 4px #40c040;" }
        span { class: "text-[10px] text-[#80a0ff]", "DP: {node_id}" }
    }
} else {
    span { class: "flex items-center gap-1 mr-1",
        span { class: "w-2 h-2 rounded-full inline-block flex-shrink-0", style: "background-color: #666;" }
        span { class: "text-[10px] text-[#888]", "DP: —" }
    }
}

// Nodes dropdown
NodesDropdown {
    nodes: nodes.read().clone(),
    selected_node_id: app_state.active_node_id,
    on_select: move |node_id: String| {
        app_state.active_node_id.set(Some(node_id));
    },
}
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/status_bar.rs
git commit -m "feat(ui): integrate NodesDropdown into StatusBar

Enables node selection directly from status bar with collapsible dropdown.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 3: Tab Data Routing (4-5 hours)

### Task 3.1: Auto-select first node on CP connect

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Add auto-select logic after CP connects**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, add after the WS event loop (around line 500):

```rust
// Auto-select first online node after CP connects
let auto_select_client = client.clone();
let auto_select_active_node = active_node_id;
let auto_select_dp_pool = dp_pool;
wasm_bindgen_futures::spawn_local(async move {
    loop {
        // Wait for CP to connect
        loop {
            if global_signal.read().ws_connected {
                break;
            }
            TimeoutFuture::new(200).await;
        }
        
        // Fetch node list
        let (tx, rx) = futures_channel::oneshot::channel();
        auto_select_client.node_list(move |result| {
            let _ = tx.send(result);
        });
        
        if let Ok(Ok(nodes)) = rx.await {
            // Find first online node
            if let Some(node) = nodes.iter().find(|n| n.status == "online") {
                let node_id = node.node_id.clone();
                let ws_url = node.ws_url.clone().unwrap_or_default();
                
                // Set as active
                auto_select_active_node.set(Some(node_id.clone()));
                
                // Create DP connection
                let mut pool = auto_select_dp_pool.write();
                pool.get_or_create(&node_id, &ws_url, vec![]);
            }
        }
        
        // Wait for disconnect before retrying
        loop {
            if !global_signal.read().ws_connected {
                break;
            }
            TimeoutFuture::new(200).await;
        }
    }
});
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(app): auto-select first online node on CP connect

Automatically sets active_node_id to first online node when CP connects.
Creates initial DP connection for the selected node.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3.2: Route FileTree through NodeDataCache

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/file_tree.rs`

- [ ] **Step 1: Replace direct rpc_client with cache lookup**

Edit `crates/vol-llm-ui/src/web/components/file_tree.rs`, replace the `use_hook` block (around line 403-460) with:

```rust
let app = use_context::<crate::web::components::app::AppState>();
let active_node = app.active_node_id;
let cache = app.node_data_cache;

// Load from cache or trigger load
use_effect(move || {
    let node_id = active_node.read().clone();
    if let Some(ref nid) = node_id {
        let needs_load = {
            let cache_read = cache.read();
            cache_read.get(nid).and_then(|d| d.files.as_ref()).is_none()
        };
        
        if needs_load {
            let dp_client = app.dp_pool.read().get(nid).map(|c| c.client.clone());
            if let Some(client) = dp_client {
                let cache_mut = cache;
                let nid_clone = nid.clone();
                client.file_list(".", move |result| {
                    let mut c = cache_mut.write();
                    let node_data = c.get_or_insert(&nid_clone);
                    match result {
                        Ok(entries) => {
                            let flat_entries: Vec<(String, bool)> = entries.into_iter().map(|e| (e.name, e.is_dir)).collect();
                            let mut ws = crate::state::WorkspaceState::new(".");
                            ws.workspace.replace_dir_children(".", flat_entries);
                            node_data.files = Some(ws);
                        }
                        Err(_) => {
                            // Leave as None, will show error
                        }
                    }
                });
            }
        }
    }
});

// Read from cache
let workspace = {
    let node_id = active_node.read().clone();
    node_id.and_then(|nid| {
        cache.read().get(&nid).and_then(|d| d.files.clone())
    })
};
```

- [ ] **Step 2: Update rendering to use cached workspace**

Edit `crates/vol-llm-ui/src/web/components/file_tree.rs`, replace `let workspace = ws.read().workspace.clone();` (around line 462) with:

```rust
let workspace = match workspace {
    Some(ws) => ws.workspace,
    None => {
        return rsx! {
            div { class: "{file_tree_outer_class(drawer_open)}",
                div { class: "{file_tree_panel_content_class(drawer_open)}",
                    div { class: "flex items-center justify-center h-full text-[#666] text-[12px]",
                        if active_node.read().is_none() {
                            "No node selected"
                        } else {
                            "Loading files..."
                        }
                    }
                }
            }
        };
    }
};
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/file_tree.rs
git commit -m "feat(ui): route FileTree through NodeDataCache

FileTree now reads from per-node cache instead of direct CP client.
Enables instant switching between nodes with cached file trees.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3.3: Route McpPanel through NodeDataCache

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

- [ ] **Step 1: Replace direct rpc_client with cache lookup**

Edit `crates/vol-llm-ui/src/web/components/mcp_panel.rs`, replace the `use_hook` block (around line 14-50) with:

```rust
let app = use_context::<crate::web::components::app::AppState>();
let active_node = app.active_node_id;
let cache = app.node_data_cache;

// Load from cache or trigger load
use_effect(move || {
    let node_id = active_node.read().clone();
    if let Some(ref nid) = node_id {
        let needs_load = {
            let cache_read = cache.read();
            cache_read.get(nid).and_then(|d| d.mcp.as_ref()).is_none()
        };
        
        if needs_load {
            let dp_client = app.dp_pool.read().get(nid).map(|c| c.client.clone());
            if let Some(client) = dp_client {
                let cache_mut = cache;
                let nid_clone = nid.clone();
                
                // Fetch MCP servers
                let sig = signal;
                client.mcp_list_servers(move |result| {
                    sig.write_unchecked().loading = false;
                    match result {
                        Ok(servers) => {
                            let mut c = cache_mut.write();
                            let node_data = c.get_or_insert(&nid_clone);
                            let mut mcp_state = node_data.mcp.clone().unwrap_or_default();
                            mcp_state.servers = servers;
                            node_data.mcp = Some(mcp_state);
                        }
                        Err(e) => {
                            sig.write_unchecked().error = Some(e);
                        }
                    }
                });
                
                // Fetch tools, resources, etc. (similar pattern)
                // ... (repeat for mcp_list_tools, mcp_list_resources, etc.)
            }
        }
    }
});

// Read from cache
let mcp_state = {
    let node_id = active_node.read().clone();
    node_id.and_then(|nid| {
        cache.read().get(&nid).and_then(|d| d.mcp.clone())
    })
};
```

- [ ] **Step 2: Update rendering to handle None case**

Edit `crates/vol-llm-ui/src/web/components/mcp_panel.rs`, add early return before main rendering:

```rust
let mcp_data = match mcp_state {
    Some(data) => data,
    None => {
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-3",
                div { class: "flex items-center justify-center h-full text-[#666] text-[12px]",
                    if active_node.read().is_none() {
                        "No node selected"
                    } else {
                        "Loading MCP data..."
                    }
                }
            }
        };
    }
};
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/mcp_panel.rs
git commit -m "feat(ui): route McpPanel through NodeDataCache

McpPanel now reads from per-node cache instead of direct CP client.
Enables instant switching between nodes with cached MCP data.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3.4: Route remaining tabs through NodeDataCache

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_tab.rs`
- Modify: `crates/vol-llm-ui/src/web/components/tasks_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/skills.rs`
- Modify: `crates/vol-llm-ui/src/web/components/log_viewer.rs`

- [ ] **Step 1: Apply same pattern to ToolsTab**

Edit `crates/vol-llm-ui/src/web/components/tools_tab.rs`, replace `use_hook` with cache lookup pattern (similar to Task 3.3).

- [ ] **Step 2: Apply same pattern to TasksPanel**

Edit `crates/vol-llm-ui/src/web/components/tasks_panel.rs`, replace `use_hook` with cache lookup pattern.

- [ ] **Step 3: Apply same pattern to SkillsPanel**

Edit `crates/vol-llm-ui/src/web/components/skills.rs`, replace `use_hook` with cache lookup pattern.

- [ ] **Step 4: Apply same pattern to LogViewer**

Edit `crates/vol-llm-ui/src/web/components/log_viewer.rs`, replace `use_hook` with cache lookup pattern.

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/tools_tab.rs crates/vol-llm-ui/src/web/components/tasks_panel.rs crates/vol-llm-ui/src/web/components/skills.rs crates/vol-llm-ui/src/web/components/log_viewer.rs
git commit -m "feat(ui): route all DP-scoped tabs through NodeDataCache

ToolsTab, TasksPanel, SkillsPanel, LogViewer now read from per-node cache.
All tabs (except Agents/Nodes) are now DP-scoped with instant node switching.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 4: Node Detail UI (3-4 hours)

### Task 4.1: Add viewing_node_detail to AppState

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs:80-89`

- [ ] **Step 1: Add viewing_node_detail signal to AppState**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, update AppState struct:

```rust
pub struct AppState {
    pub event_bus: EventBus,
    pub rpc_client: JsonRpcClient,
    pub active_tab: Signal<ActiveTab>,
    pub cp_client: JsonRpcClient,
    pub dp_pool: Signal<DpConnectionPool>,
    pub active_node_id: Signal<Option<String>>,
    pub node_data_cache: Signal<NodeDataCache>,
    pub viewing_node_detail: Signal<Option<String>>,  // ← NEW
}
```

- [ ] **Step 2: Initialize viewing_node_detail**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, add signal initialization:

```rust
let viewing_node_detail = use_signal(|| Option::<String>::None);
```

- [ ] **Step 3: Pass to AppState provider**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, update use_context_provider:

```rust
use_context_provider(|| AppState {
    event_bus: event_bus.with(|eb| eb.clone()),
    rpc_client: client.clone(),
    active_tab,
    cp_client: client.clone(),
    dp_pool,
    active_node_id,
    node_data_cache,
    viewing_node_detail,  // ← NEW
});
```

- [ ] **Step 4: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(app): add viewing_node_detail signal to AppState

Enables routing to Node Detail UI when user clicks a node card.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4.2: Implement NodeDetailPanel component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/node_detail_panel.rs`

- [ ] **Step 1: Create NodeDetailPanel struct**

Create `crates/vol-llm-ui/src/web/components/node_detail_panel.rs`:

```rust
use dioxus::prelude::*;
use crate::web::client::{NodeRecord, CapabilitySnapshot, AgentListEntry};

#[derive(Debug, Clone, Default)]
pub struct NodeDetailState {
    pub node: Option<NodeRecord>,
    pub agents: Vec<AgentListEntry>,
    pub capabilities: Option<CapabilitySnapshot>,
    pub loading: bool,
    pub error: Option<String>,
}

#[component]
pub fn NodeDetailPanel(node_id: String) -> Element {
    let app = use_context::<crate::web::components::app::AppState>();
    let mut state = use_signal(|| NodeDetailState::default());
    
    // Fetch node detail
    use_effect(move || {
        let mut s = state;
        s.with_mut(|s| { s.loading = true; s.error = None; });
        
        let cp = app.cp_client.clone();
        let nid = node_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            let (tx, rx) = futures_channel::oneshot::channel();
            cp.node_get(&nid, move |result| {
                let _ = tx.send(result);
            });
            
            match rx.await {
                Ok(Ok(Some(node))) => {
                    s.with_mut(|s| { s.node = Some(node); s.loading = false; });
                }
                Ok(Ok(None)) => {
                    s.with_mut(|s| { s.error = Some("Node not found".into()); s.loading = false; });
                }
                Ok(Err(e)) => {
                    s.with_mut(|s| { s.error = Some(e); s.loading = false; });
                }
                Err(_) => {
                    s.with_mut(|s| { s.error = Some("Channel closed".into()); s.loading = false; });
                }
            }
        });
    });
    
    let node_data = state.read().node.clone();
    
    rsx! {
        div { class: "flex-1 overflow-y-auto p-4",
            // Back button
            button {
                class: "mb-4 px-3 py-1.5 bg-[#2a2a44] hover:bg-[#3a3a55] rounded text-[12px] text-[#e0e0e0]",
                onclick: move |_| {
                    app.viewing_node_detail.set(None);
                },
                "← Back to DP View"
            }
            
            if state.read().loading {
                div { class: "text-[#888] text-[14px]", "Loading node details..." }
            } else if let Some(ref err) = state.read().error {
                div { class: "text-[#ff6060] text-[14px]", "Error: {err}" }
            } else if let Some(node) = node_data {
                // Overview section
                div { class: "mb-6",
                    h2 { class: "text-[16px] font-bold text-[#e0e0e0] mb-3", "Overview" }
                    div { class: "bg-[#1e1e36] border border-[#333355] rounded p-3 space-y-2",
                        div { class: "flex gap-2", span { class: "text-[#888] text-[12px]", "Node ID:" } span { class: "text-[#e0e0e0] text-[12px]", "{node.node_id}" } }
                        div { class: "flex gap-2", span { class: "text-[#888] text-[12px]", "Name:" } span { class: "text-[#e0e0e0] text-[12px]", "{node.name}" } }
                        div { class: "flex gap-2", span { class: "text-[#888] text-[12px]", "Version:" } span { class: "text-[#e0e0e0] text-[12px]", "{node.version}" } }
                        div { class: "flex gap-2", span { class: "text-[#888] text-[12px]", "Status:" } span { class: "text-[#e0e0e0] text-[12px]", "{node.status}" } }
                    }
                }
                
                // Resource Usage section
                if let Some(ref load) = node.load {
                    div { class: "mb-6",
                        h2 { class: "text-[16px] font-bold text-[#e0e0e0] mb-3", "Resource Usage" }
                        div { class: "bg-[#1e1e36] border border-[#333355] rounded p-3 space-y-2",
                            if let Some(cpu) = load.cpu {
                                div { class: "flex gap-2", span { class: "text-[#888] text-[12px]", "CPU:" } span { class: "text-[#e0e0e0] text-[12px]", "{cpu:.1}%" } }
                            }
                            if let Some(mem) = load.memory_mb {
                                div { class: "flex gap-2", span { class: "text-[#888] text-[12px]", "Memory:" } span { class: "text-[#e0e0e0] text-[12px]", "{mem}MB" } }
                            }
                        }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Export NodeDetailPanel**

Edit `crates/vol-llm-ui/src/web/components/mod.rs`, add:

```rust
pub mod node_detail_panel;
pub use node_detail_panel::NodeDetailPanel;
```

- [ ] **Step 3: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/node_detail_panel.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat(ui): add NodeDetailPanel component

CP-scoped view showing node overview, resource usage, and metadata.
Accessible by clicking node name in NodesDropdown.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4.3: Route to NodeDetailPanel from TabContent

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Update TabContent to check viewing_node_detail**

Edit `crates/vol-llm-ui/src/web/components/app.rs`, update TabContent function (around line 795):

```rust
#[component]
fn TabContent(skill_dialog_signal: Signal<SkillDialogState>) -> Element {
    let state: AppState = use_context();
    let active = *state.active_tab.read();
    
    // Check if viewing node detail
    if let Some(ref node_id) = *state.viewing_node_detail.read() {
        return rsx! { NodeDetailPanel { node_id: node_id.clone() } };
    }
    
    // Normal tab routing
    match active {
        ActiveTab::Nodes => rsx! { NodesPanel {} },
        ActiveTab::Tasks => rsx! { TasksPanel { assignee_filter: None } },
        ActiveTab::Agents => rsx! { AgentsPanel {} },
        ActiveTab::Tools => rsx! { ToolsTabContent {} },
        ActiveTab::Workspace => rsx! { FileContentView {} },
        ActiveTab::Skills => rsx! { SkillsPanel { dialog_signal: skill_dialog_signal } },
        ActiveTab::Logs => rsx! { LogViewer {} },
        ActiveTab::Mcp => rsx! { McpPanel {} },
    }
}
```

- [ ] **Step 2: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat(ui): route to NodeDetailPanel when viewing node detail

TabContent now checks viewing_node_detail and renders NodeDetailPanel if set.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4.4: Add click handler to NodesDropdown to enter Node Detail

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/nodes_dropdown.rs`

- [ ] **Step 1: Add click handler for node name**

Edit `crates/vol-llm-ui/src/web/components/nodes_dropdown.rs`, update NodeRow to add a separate click handler for the node name (around line 60-80):

```rust
#[component]
fn NodeRow(
    node: NodeListEntry,
    is_selected: bool,
    on_select: EventHandler<()>,
    on_view_detail: EventHandler<()>,  // ← NEW
) -> Element {
    let status_color = if node.status == "online" { "bg-green-500" } else { "bg-red-500" };
    let row_class = if is_selected {
        "flex items-center gap-2 px-3 py-2 hover:bg-[#2a2a44] cursor-pointer bg-[#1a2a44]"
    } else {
        "flex items-center gap-2 px-3 py-2 hover:bg-[#2a2a44] cursor-pointer"
    };
    
    rsx! {
        div { class: "{row_class}", onclick: move |_| on_select.call(()),
            div { class: "w-2 h-2 rounded-full {status_color} flex-shrink-0" }
            div { class: "flex-1 min-w-0 cursor-pointer hover:text-[#80a0ff]", onclick: move |e| {
                e.stop_propagation();
                on_view_detail.call(());
            },
                div { class: "text-[#e0e0e0] text-[12px] font-medium truncate", "{node.name}" }
                div { class: "text-[#888] text-[10px]", "{node.node_id} · v{node.version}" }
            }
            if let Some(ref load) = node.load {
                div { class: "text-[#888] text-[10px] flex-shrink-0",
                    if let Some(cpu) = load.cpu {
                        span { "cpu:{cpu:.0}%" }
                    }
                }
            }
            if is_selected {
                span { class: "text-[#80a0ff] text-[10px] flex-shrink-0", "✓" }
            }
        }
    }
}
```

- [ ] **Step 2: Update NodesDropdown to pass on_view_detail**

Edit `crates/vol-llm-ui/src/web/components/nodes_dropdown.rs`, update the NodeRow invocation (around line 40-50):

```rust
for node in nodes.iter() {
    NodeRow {
        node: node.clone(),
        is_selected: selected_node_id.read().as_ref() == Some(&node.node_id),
        on_select: {
            let node_id = node.node_id.clone();
            let mut is_open_sig = is_open;
            move |_| {
                on_select.call(node_id.clone());
                is_open_sig.set(false);
            }
        },
        on_view_detail: {
            let node_id = node.node_id.clone();
            let app = app_state.clone();  // ← Need to pass AppState
            move |_| {
                app.viewing_node_detail.set(Some(node_id.clone()));
                is_open.set(false);
            }
        },
    }
}
```

- [ ] **Step 3: Pass AppState to NodesDropdown**

Edit `crates/vol-llm-ui/src/web/components/nodes_dropdown.rs`, update NodesDropdown signature:

```rust
#[component]
pub fn NodesDropdown(
    nodes: Vec<NodeListEntry>,
    selected_node_id: Signal<Option<String>>,
    on_select: EventHandler<String>,
    app_state: AppState,  // ← NEW
) -> Element {
    // ...
}
```

- [ ] **Step 4: Update StatusBar to pass app_state**

Edit `crates/vol-llm-ui/src/web/components/status_bar.rs`, update NodesDropdown invocation:

```rust
NodesDropdown {
    nodes: nodes.read().clone(),
    selected_node_id: app_state.active_node_id,
    on_select: move |node_id: String| {
        app_state.active_node_id.set(Some(node_id));
    },
    app_state: app_state.clone(),  // ← NEW
}
```

- [ ] **Step 5: Run cargo check**

Run: `cargo check -p vol-llm-ui`
Expected: PASS — no errors

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/nodes_dropdown.rs crates/vol-llm-ui/src/web/components/status_bar.rs
git commit -m "feat(ui): enable entering Node Detail from NodesDropdown

Clicking node name in dropdown enters Node Detail UI (CP-scoped view).

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Phase 5: Testing and Polish (2-3 hours)

### Task 5.1: Add integration tests for node selection

**Files:**
- Create: `crates/vol-llm-ui/tests/node_selection.rs`

- [ ] **Step 1: Write integration test for cache persistence**

Create `crates/vol-llm-ui/tests/node_selection.rs`:

```rust
use vol_llm_ui::state::NodeDataCache;

#[test]
fn test_cached_data_persists_across_node_switches() {
    let mut cache = NodeDataCache::default();
    
    // Load data for node-A
    let data_a = cache.get_or_insert("node-A");
    data_a.files = Some(vol_llm_ui::state::WorkspaceState::new("."));
    
    // Load data for node-B
    let data_b = cache.get_or_insert("node-B");
    data_b.files = Some(vol_llm_ui::state::WorkspaceState::new("."));
    
    // Both should be cached
    assert!(cache.get("node-A").unwrap().files.is_some());
    assert!(cache.get("node-B").unwrap().files.is_some());
    
    // Switch back to node-A, data should still be there
    assert!(cache.get("node-A").unwrap().files.is_some());
}
```

- [ ] **Step 2: Run test**

Run: `cargo test --test node_selection`
Expected: PASS

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/tests/node_selection.rs
git commit -m "test: add integration test for node selection cache

Verifies per-node data persists across node switches.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5.2: Run full test suite and verify

- [ ] **Step 1: Run all tests**

Run: `cargo test -p vol-llm-ui`
Expected: All tests pass

- [ ] **Step 2: Run cargo clippy**

Run: `cargo clippy -p vol-llm-ui -- -D warnings`
Expected: No warnings

- [ ] **Step 3: Fix any issues found**

If clippy finds issues, fix them and commit:

```bash
git add -A
git commit -m "fix: address clippy warnings in node selection code

Co-Authored-By: Claude <noreply@anthropic.com>"
```

- [ ] **Step 4: Build in release mode**

Run: `cargo build -p vol-llm-ui --release`
Expected: Build succeeds

- [ ] **Step 5: Final commit**

```bash
git add -A
git commit -m "chore: finalize node selection and data routing implementation

All phases complete:
- NodeDataCache infrastructure
- NodesDropdown UI in status bar
- Tab data routing through per-node cache
- Node Detail UI for CP-scoped view
- Integration tests

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

## Summary

**Total estimated time:** 14-19 hours

**Key deliverables:**
1. ✅ NodeDataCache for per-node data caching
2. ✅ NodesDropdown in status bar for node selection
3. ✅ All DP-scoped tabs route through NodeDataCache
4. ✅ Node Detail UI for CP-scoped node info
5. ✅ Integration tests

**Testing checklist:**
- [ ] `cargo test -p vol-llm-ui` — all tests pass
- [ ] `cargo clippy -p vol-llm-ui` — no warnings
- [ ] `cargo build -p vol-llm-ui --release` — builds successfully
- [ ] Manual test: open UI, verify Nodes dropdown appears in status bar
- [ ] Manual test: click node to select, verify tabs load DP data
- [ ] Manual test: switch nodes, verify instant cache switching
- [ ] Manual test: click node name, verify Node Detail UI appears
