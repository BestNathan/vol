# Agents Panel Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an "Agents" tab to the web UI that lists all registered agents with expandable detail views showing their definitions.

**Architecture:** Backend adds `agent.list` JSON-RPC method returning metadata from `self.holders.keys()`. Frontend adds `ActiveTab::Agents`, `AgentsState`, and `AgentsPanel` component following existing patterns (request/response on mount, no EventBus subscription).

**Tech Stack:** Rust, Dioxus 0.6 (WASM), JSON-RPC over WebSocket, serde

---

### Task 1: Add `AgentList` variant to JsonRpcRequest enum and parser

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs`

- [ ] **Step 1: Add `AgentList` variant to JsonRpcRequest enum**

Add after the `SessionResume` variant in the `JsonRpcRequest` enum (around line 73):

```rust
    AgentList {
        id: u64,
    },
```

- [ ] **Step 2: Add parser case for `"agent.list"` method**

Add in `parse_jsonrpc_request` match block (before the `_ =>` fallback, around line 410):

```rust
        "agent.list" => Ok(JsonRpcRequest::AgentList { id }),
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: All existing tests pass (no new test needed — parsing is trivial and covered by existing `test_jsonrpc_event_format` for the event side)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs
git commit -m "feat: add agent.list JSON-RPC method variant and parser"
```

---

### Task 2: Add `handle_agent_list()` to JsonRpcConnection

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

- [ ] **Step 1: Add `handle_agent_list` method**

Add after `handle_session_resume` (around line 317):

```rust
    /// Handle `agent.list`: return metadata for all registered agents.
    async fn handle_agent_list(&self, id: u64) -> String {
        let agents: Vec<serde_json::Value> = self.holders.keys().map(|k| {
            serde_json::json!({
                "id": k,
                "name": k,
                "type": k,
                "description": "Code assistant",
                "scope": "Server",
            })
        }).collect();
        to_jsonrpc_response(id, serde_json::json!({ "agents": agents }))
    }
```

- [ ] **Step 2: Wire into dispatch match**

Add a new arm in the `handle_text_frame` dispatch match (around line 184, before `JsonRpcRequest::Unknown`):

```rust
            JsonRpcRequest::AgentList { id } => {
                self.handle_agent_list(*id).await
            }
```

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: All tests pass

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/connection.rs
git commit -m "feat: add handle_agent_list to JsonRpcConnection"
```

---

### Task 3: Add `ActiveTab::Agents`, `AgentListEntry`, `AgentsState` to state module

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add `AgentListEntry` struct**

Add after `LogViewerState` (around line 472, before `SessionDialogState`):

```rust
/// A single agent entry returned by agent.list RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListEntry {
    pub id: String,
    pub name: String,
    pub r#type: String,
    pub description: String,
    pub scope: String,
}
```

- [ ] **Step 2: Add `AgentsState` struct**

Add after `AgentListEntry`:

```rust
/// Local state for AgentsPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct AgentsState {
    pub agents: Vec<AgentListEntry>,
    pub expanded: HashSet<usize>,
    pub loading: bool,
    pub error: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl AgentsState {
    pub fn new() -> Self {
        Self { agents: Vec::new(), expanded: HashSet::new(), loading: false, error: None }
    }
}
```

- [ ] **Step 3: Add `Agents` to `ActiveTab` enum**

Change line 204 from:
```rust
pub enum ActiveTab { Conversation, Tools, Workspace, Skills, Logs }
```
to:
```rust
pub enum ActiveTab { Conversation, Tools, Workspace, Skills, Logs, Agents }
```

- [ ] **Step 4: Update `ActiveTab::toggle()` method**

Change lines 207-215 to include `Agents`:

```rust
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Tools,
            ActiveTab::Tools => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Agents,
            ActiveTab::Agents => ActiveTab::Conversation,
        }
    }
```

- [ ] **Step 5: Update existing test for ActiveTab toggle**

Change the test at line 927-934 to include Agents:

```rust
    #[test]
    fn test_active_tab_toggle() {
        use ActiveTab::*;
        assert_eq!(Conversation.toggle(), Tools);
        assert_eq!(Tools.toggle(), Workspace);
        assert_eq!(Workspace.toggle(), Skills);
        assert_eq!(Skills.toggle(), Logs);
        assert_eq!(Logs.toggle(), Agents);
        assert_eq!(Agents.toggle(), Conversation);
    }
```

- [ ] **Step 6: Run tests**

Run: `cargo test -p vol-llm-ui --features web`
Expected: All tests pass including updated toggle test

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat: add ActiveTab::Agents, AgentListEntry, and AgentsState"
```

---

### Task 4: Add `agent_list()` method to JsonRpcClient

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add `AgentListEntry` struct to client module**

Add after `FileEntry` struct (around line 37):

```rust
/// Agent metadata entry returned by agent.list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentListEntry {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub description: String,
    pub scope: String,
}
```

- [ ] **Step 2: Add `agent_list()` method to JsonRpcClient**

Add after `file_read` method (around line 275, before `handle_message`):

```rust
    /// List all registered agents on the server. Returns entries via callback.
    pub fn agent_list(&self, cb: impl FnOnce(Result<Vec<AgentListEntry>, String>) + 'static) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.list",
            "params": {},
            "id": id,
        });
        let json = match serde_json::to_string(&msg) {
            Ok(j) => j,
            Err(e) => { cb(Err(e.to_string())); return; }
        };
        if let Err(e) = self.send_raw(&json) {
            cb(Err(format!("send failed: {e:?}")));
            return;
        }

        let cb: Box<dyn FnOnce(serde_json::Value)> = Box::new(move |result| {
            match result.get("agents").and_then(|v| v.as_array()) {
                Some(agents) => {
                    let parsed: Vec<AgentListEntry> = agents.iter()
                        .filter_map(|e| serde_json::from_value(e.clone()).ok())
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no agents in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }
```

- [ ] **Step 3: Run tests**

Run: `cargo check -p vol-llm-ui --features web`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat: add agent_list() method to JsonRpcClient"
```

---

### Task 5: Create AgentsPanel component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/agents_panel.rs`

- [ ] **Step 1: Create the AgentsPanel component file**

```rust
//! Agents panel showing all registered agents with expandable details.

use dioxus::prelude::*;

use crate::state::{AgentListEntry, AgentsState};

/// Agents panel component.
#[component]
pub fn AgentsPanel() -> Element {
    let app: crate::web::components::app::AppState = use_context();
    let agents_signal: Signal<AgentsState> = use_context();

    // Load agents on mount
    use_hook(move || {
        let rpc = app.rpc_client.clone();
        let mut sig = agents_signal;

        sig.with_mut(|s| {
            s.loading = true;
            s.error = None;
        });

        rpc.agent_list(move |result| {
            sig.with_mut(|s| {
                s.loading = false;
                match result {
                    Ok(agents) => { s.agents = agents; }
                    Err(e) => { s.error = Some(e); }
                }
            });
        });
    });

    let (agents, expanded, loading, error) = {
        let s = agents_signal.read();
        (s.agents.clone(), s.expanded.clone(), s.loading, s.error.clone())
    };

    if loading {
        return rsx! {
            div { class: "agents-panel",
                div { class: "agents-panel-loading", "Loading agents..." }
            }
        };
    }

    if let Some(ref e) = error {
        return rsx! {
            div { class: "agents-panel",
                div { class: "agents-panel-error",
                    "Error: {e}"
                }
            }
        };
    }

    if agents.is_empty() {
        return rsx! {
            div { class: "agents-panel",
                div { class: "agents-panel-empty", "No agents discovered" }
            }
        };
    }

    let items: Vec<Element> = agents.iter().enumerate().map(|(i, agent)| {
        let is_expanded = expanded.contains(&i);
        rsx! { AgentItem { agent: agent.clone(), index: i, is_expanded, agents_signal } }
    }).collect();

    rsx! {
        div { class: "agents-panel",
            {items.into_iter()}
        }
    }
}

#[component]
fn AgentItem(agent: AgentListEntry, index: usize, is_expanded: bool, agents_signal: Signal<AgentsState>) -> Element {
    let scope_color = match agent.scope.as_str() {
        "Server" => "#c0c040",
        "Repo" => "#4080ff",
        "User" => "#40c040",
        _ => "#888",
    };

    rsx! {
        div { class: "agent-item",
            div {
                class: "agent-item-header",
                onclick: move |_: Event<MouseData>| {
                    agents_signal.with_mut(|s| {
                        if s.expanded.contains(&index) {
                            s.expanded.remove(&index);
                        } else {
                            s.expanded.insert(index);
                        }
                    });
                },
                span { class: "agent-item-chevron", "\u{25be}" }
                span { class: "agent-item-name", "{agent.name}" }
                span {
                    class: "agent-item-scope",
                    style: "background: {scope_color}; color: #1a1a2e;",
                    "{agent.scope}"
                }
            }
            div { class: "agent-item-desc", "{agent.description}" }
            if is_expanded {
                div { class: "agent-item-detail",
                    div { class: "agent-detail-row",
                        span { class: "agent-detail-label", "ID: " }
                        span { class: "agent-detail-value", "{agent.id}" }
                    }
                    div { class: "agent-detail-row",
                        span { class: "agent-detail-label", "Type: " }
                        span { class: "agent-detail-value", "{agent.type}" }
                    }
                    div { class: "agent-detail-row",
                        span { class: "agent-detail-label", "Scope: " }
                        span { class: "agent-detail-value", "{agent.scope}" }
                    }
                }
            }
        }
    }
}
```

- [ ] **Step 2: Add CSS for agents panel**

Append to `GLOBAL_CSS` in `app.rs` (before the closing `"#"` of the CSS string, around line 528):

```css
/* Agents panel */
.agents-panel { flex: 1; overflow-y: auto; padding: 8px; }
.agents-panel-loading, .agents-panel-empty, .agents-panel-error { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; padding: 20px; text-align: center; }
.agents-panel-error { color: #ff6060; }
.agent-item { border-bottom: 1px solid #2a2a44; }
.agent-item-header { display: flex; align-items: center; padding: 8px 10px; cursor: pointer; gap: 8px; }
.agent-item-header:hover { background: #222240; }
.agent-item-chevron { font-size: 10px; color: #666; transition: transform 0.15s; }
.agent-item-name { font-weight: 600; font-size: 13px; color: #e0e0e0; }
.agent-item-scope { font-size: 10px; padding: 1px 6px; border-radius: 3px; font-weight: bold; margin-left: auto; }
.agent-item-desc { font-size: 12px; color: #888; padding: 0 10px 6px 28px; }
.agent-item-detail { padding: 8px 10px 8px 28px; font-size: 12px; background: #16162a; }
.agent-detail-row { padding: 2px 0; }
.agent-detail-label { color: #6090ff; font-weight: 600; }
.agent-detail-value { color: #ccc; font-family: monospace; }
```

- [ ] **Step 3: Run tests**

Run: `cargo check -p vol-llm-ui --features web`
Expected: Compiles without errors

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/agents_panel.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: create AgentsPanel component with expandable agent details"
```

---

### Task 6: Wire up Agents tab in App

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Add AgentsState signal to App**

After `tool_signal` (around line 119), add:

```rust
    let agents_signal = use_signal(|| AgentsState::new());
```

- [ ] **Step 2: Provide AgentsState via context**

After the other `use_context_provider` calls (around line 282), add:

```rust
    use_context_provider(|| agents_signal);
```

- [ ] **Step 3: Add import for AgentsState**

Add `AgentsState` to the imports at line 7:

```rust
use crate::state::{ActiveTab, AgentsState, ApprovalUiState, ConversationState, EventBus, GlobalState, SubscriptionSet, ToolState, UiEvent, UiEventKind, WorkspaceState};
```

- [ ] **Step 4: Add import for AgentsPanel component**

Add to component imports (around line 19):

```rust
use super::agents_panel::AgentsPanel;
```

- [ ] **Step 5: Add "Agents" tab button to TabBar**

Add after the Logs tab button (around line 313):

```rust
            TabButton { state: state.clone(), tab: ActiveTab::Agents, label: "Agents" }
```

- [ ] **Step 6: Add Agents tab content route**

Add to the `TabContent` match (around line 344):

```rust
        ActiveTab::Agents => rsx! { AgentsPanel {} },
```

- [ ] **Step 7: Run tests and build**

Run: `cargo check -p vol-llm-ui --features web`
Expected: Compiles without errors

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: wire Agents tab into App tab bar and content router"
```

---

## Self-Review

### 1. Spec Coverage Check

| Spec Requirement | Task |
|-----------------|------|
| Backend: `agent.list` JSON-RPC method | Task 1 (parser) + Task 2 (handler) |
| Frontend: `ActiveTab::Agents` | Task 3 |
| Frontend: `AgentListEntry`, `AgentsState` | Task 3 |
| Frontend: `agent_list()` RPC client method | Task 4 |
| Frontend: `AgentsPanel` component | Task 5 |
| Frontend: Agents tab in TabBar + TabContent | Task 6 |
| Error handling: empty list, RPC error, loading | Task 5 |
| Scope badges with colors | Task 5 (CSS inline style) |
| Expandable detail on click | Task 5 |

### 2. Placeholder Scan

No TBD, TODO, or vague instructions found. All code steps contain actual content.

### 3. Type Consistency

- `AgentListEntry` in `state/mod.rs` uses `r#type` field (Rust keyword escape)
- `AgentListEntry` in `client.rs` uses `type_` with `#[serde(rename = "type")]` — this is fine because the wire format uses `"type"` and both deserialize correctly
- `AgentsState` follows same pattern as `ToolState`, `ConversationState` etc.
- `ActiveTab::Agents` added consistently to enum and toggle

### 4. Architecture Notes

- The backend returns placeholder data (`"Code assistant"` description, `"Server"` scope) since full `AgentLoader` integration is not yet wired. This matches the spec's "pragmatic choice."
- The frontend `AgentsPanel` uses the same pattern as `FileTree`: `use_hook` for load-on-mount, reads from shared `Signal<AgentsState>` via `use_context`.
- Unlike Conversation/Tools, Agents does NOT need EventBus subscriptions — it's a simple request/response. The state is kept in a local signal for expand/collapse tracking and refresh.
