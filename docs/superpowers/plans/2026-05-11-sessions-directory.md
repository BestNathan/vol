# Sessions Directory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Sessions tab that lists all persisted sessions from disk, lets users select and load a session's message history into the Conversation view, displaying the complete entry timeline including messages, checkpoints, and summaries.

**Architecture:** Wire `FileSessionEntryStore` into `JsonRpcConnection` to serve `session.list` and new `session.entries` RPC methods. Frontend adds a Sessions tab with `SessionsPanel` component that fetches session list, and on click fetches entries and converts them to `ConversationEntry` for display in the existing Conversation view.

**Tech Stack:** Rust (axum WebSocket JSON-RPC), Dioxus 0.6 WASM frontend, `FileSessionEntryStore` (vol-session), split signal state architecture.

---

### Task 1: Add `SessionEntries` variant to JsonRpcRequest + parser

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs`

- [ ] **Step 1: Add SessionEntries variant to JsonRpcRequest enum**

Add after `AgentList` variant (line ~76) in `serde_helpers.rs`:

```rust
    SessionEntries {
        id: u64,
        session_id: String,
    },
```

- [ ] **Step 2: Add parser case for "session.entries"**

Add after the `"agent.list"` parser arm (line ~414) in `parse_jsonrpc_request`:

```rust
        "session.entries" => {
            let session_id = params
                .get("session_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "session.entries: missing 'session_id'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::SessionEntries { id, session_id })
        }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: compiles cleanly (existing warnings about dead_code are fine)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs
git commit -m "feat: add SessionEntries JSON-RPC request variant and parser"
```

---

### Task 2: Wire FileSessionEntryStore into JsonRpcConnection with real handlers

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`
- Modify: `crates/vol-llm-agent-channel/Cargo.toml` (if vol-session not already a dep)

- [ ] **Step 1: Check vol-session dependency**

Read `crates/vol-llm-agent-channel/Cargo.toml` to verify `vol-session` is a dependency. If not, add it.

- [ ] **Step 2: Add entry_store field and imports**

At the top of `connection.rs`, add import after existing imports:

```rust
use vol_session::file_store::FileSessionEntryStore;
```

Add `entry_store: FileSessionEntryStore` field to `JsonRpcConnection` struct (after `store_dir`):

```rust
pub struct JsonRpcConnection {
    // ... existing fields ...
    /// Store directory for log/session operations.
    store_dir: String,
    /// File-based session entry store.
    entry_store: FileSessionEntryStore,
}
```

- [ ] **Step 3: Create entry_store in new()**

In `JsonRpcConnection::new()`, add after `store_dir`:

```rust
        Self {
            // ... existing fields ...
            working_dir,
            store_dir,
            entry_store: FileSessionEntryStore::new(&store_dir),
        }
```

- [ ] **Step 4: Replace handle_session_list stub with real implementation**

Replace the existing `handle_session_list` method (lines 309-312):

```rust
    /// Handle `session.list`: return session summaries from FileSessionEntryStore.
    async fn handle_session_list(&self, id: u64) -> String {
        match self.entry_store.list_sessions() {
            Ok(summaries) => {
                let sessions: Vec<serde_json::Value> = summaries
                    .into_iter()
                    .map(|s| {
                        serde_json::json!({
                            "id": s.session_id,
                            "entry_count": s.entry_count,
                            "created_at": s.created_at,
                        })
                    })
                    .collect();
                to_jsonrpc_response(id, serde_json::json!({ "sessions": sessions }))
            }
            Err(e) => to_jsonrpc_error(Some(id), -32000, format!("Failed to list sessions: {e}")),
        }
    }
```

- [ ] **Step 5: Add handle_session_entries method**

Add after `handle_agent_list` method:

```rust
    /// Handle `session.entries`: return all entries for a session.
    async fn handle_session_entries(&self, id: u64, session_id: String) -> String {
        match self.entry_store.get_entries(&session_id).await {
            Ok(entries) => {
                let json_entries: Vec<serde_json::Value> = entries
                    .into_iter()
                    .filter_map(|e| serde_json::to_value(e).ok())
                    .collect();
                to_jsonrpc_response(id, serde_json::json!({ "entries": json_entries }))
            }
            Err(e) => to_jsonrpc_error(Some(id), -32000, format!("Failed to get entries: {e}")),
        }
    }
```

- [ ] **Step 6: Wire SessionEntries into handle_text_frame dispatch**

Add after the `AgentList` dispatch arm (line ~187) in `handle_text_frame`:

```rust
            JsonRpcRequest::SessionEntries { id, session_id } => {
                self.handle_session_entries(*id, session_id.clone()).await
            }
```

- [ ] **Step 7: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: compiles cleanly

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/connection.rs
git commit -m "feat: wire FileSessionEntryStore into JsonRpcConnection with real session.list and session.entries handlers"
```

---

### Task 3: Add Sessions-related state types to state/mod.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add Sessions variant to ActiveTab enum**

Update `ActiveTab` enum (line ~204):

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActiveTab { Conversation, Sessions, Tools, Workspace, Skills, Logs, Agents }
```

- [ ] **Step 2: Update ActiveTab::toggle()**

Update `toggle()` method (lines 207-216):

```rust
impl ActiveTab {
    pub fn toggle(self) -> Self {
        match self {
            ActiveTab::Conversation => ActiveTab::Sessions,
            ActiveTab::Sessions => ActiveTab::Tools,
            ActiveTab::Tools => ActiveTab::Workspace,
            ActiveTab::Workspace => ActiveTab::Skills,
            ActiveTab::Skills => ActiveTab::Logs,
            ActiveTab::Logs => ActiveTab::Agents,
            ActiveTab::Agents => ActiveTab::Conversation,
        }
    }
}
```

- [ ] **Step 3: Add EntryCheckpoint variant to ConversationEntry**

Add to `ConversationEntry` enum (after `RunSummary`, line ~130):

```rust
    EntryCheckpoint { reason: String, note: Option<String>, created_at: i64 },
```

- [ ] **Step 4: Add SessionListEntry and SessionsState structs**

Add after `AgentsState` impl (after line ~501):

```rust
/// Session list entry from session.list RPC.
#[derive(Debug, Clone)]
pub struct SessionListEntry {
    pub id: String,
    pub entry_count: usize,
    pub created_at: i64,
}

/// Local state for SessionsPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct SessionsState {
    pub sessions: Vec<SessionListEntry>,
    pub loading: bool,
    pub error: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SessionsState {
    pub fn new() -> Self {
        Self { sessions: Vec::new(), loading: false, error: None }
    }
}
```

- [ ] **Step 5: Update test_active_tab_toggle test**

Update the test (lines 956-964):

```rust
    #[test]
    fn test_active_tab_toggle() {
        use ActiveTab::*;
        assert_eq!(Conversation.toggle(), Sessions);
        assert_eq!(Sessions.toggle(), Tools);
        assert_eq!(Tools.toggle(), Workspace);
        assert_eq!(Workspace.toggle(), Skills);
        assert_eq!(Skills.toggle(), Logs);
        assert_eq!(Logs.toggle(), Agents);
        assert_eq!(Agents.toggle(), Conversation);
    }
```

- [ ] **Step 6: Verify compilation and tests**

Run: `cargo check -p vol-llm-ui --features web`
Run: `cargo test -p vol-llm-ui test_active_tab_toggle`
Expected: compiles and test passes

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat: add ActiveTab::Sessions, SessionsState, SessionListEntry, EntryCheckpoint to state"
```

---

### Task 4: Add session_entries() RPC client method to client.rs

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add SessionEntry struct**

Add after `AgentListEntry` struct (after line ~48):

```rust
/// Session entry matching the vol-session wire format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEntry {
    pub id: String,
    pub session_id: String,
    pub created_at: i64,
    pub parent_id: Option<String>,
    #[serde(rename = "type")]
    pub entry_type: String,
    pub data: serde_json::Value,
}
```

- [ ] **Step 2: Add session_list method**

Add after `agent_list` method (after line ~319):

```rust
    /// List all persisted sessions on the server. Returns entries via callback.
    pub fn session_list(&self, cb: impl FnOnce(Result<Vec<crate::state::SessionListEntry>, String>) + 'static) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session.list",
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
            match result.get("sessions").and_then(|v| v.as_array()) {
                Some(sessions) => {
                    let parsed: Vec<crate::state::SessionListEntry> = sessions.iter()
                        .filter_map(|s| {
                            let id = s.get("id").and_then(|v| v.as_str())?.to_string();
                            let entry_count = s.get("entry_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
                            let created_at = s.get("created_at").and_then(|v| v.as_i64())?;
                            Some(crate::state::SessionListEntry { id, entry_count, created_at })
                        })
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no sessions in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }
```

- [ ] **Step 3: Add session_entries method**

Add after `session_list`:

```rust
    /// Fetch all entries for a specific session. Returns entries via callback.
    pub fn session_entries(&self, session_id: &str, cb: impl FnOnce(Result<Vec<SessionEntry>, String>) + 'static) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session.entries",
            "params": { "session_id": session_id },
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
            match result.get("entries").and_then(|v| v.as_array()) {
                Some(entries) => {
                    let parsed: Vec<SessionEntry> = entries.iter()
                        .filter_map(|e| serde_json::from_value(e.clone()).ok())
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no entries in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }
```

- [ ] **Step 4: Verify compilation**

Run: `cargo check -p vol-llm-ui --features web`
Expected: compiles cleanly

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat: add session_list and session_entries RPC client methods with SessionEntry type"
```

---

### Task 5: Create SessionsPanel component with converter function

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

- [ ] **Step 1: Register sessions_panel module**

In `mod.rs`, add `pub mod sessions_panel;` and `pub use sessions_panel::SessionsPanel;`:

```rust
pub mod sessions_panel;
// ...
pub use sessions_panel::SessionsPanel;
```

- [ ] **Step 2: Create sessions_panel.rs with converter and component**

Create `crates/vol-llm-ui/src/web/components/sessions_panel.rs`:

```rust
//! Sessions panel listing all persisted sessions with load-on-click into Conversation view.

use dioxus::prelude::*;

use crate::state::{ConversationEntry, ConversationState, SessionsState};
use crate::web::client::SessionEntry;

/// Convert raw session entries to ConversationEntry for display.
fn session_entries_to_conversation(entries: Vec<SessionEntry>) -> Vec<ConversationEntry> {
    entries.into_iter().filter_map(|e| {
        match e.entry_type.as_str() {
            "message" => {
                // Try to extract message role and content from data
                let data = &e.data;
                if let Some(msg) = data.get("message") {
                    if let Some(role) = msg.get("role").and_then(|v| v.as_str()) {
                        let text = msg.get("content").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        match role {
                            "user" => Some(ConversationEntry::UserInput { text }),
                            "assistant" => Some(ConversationEntry::AgentAnswer { text }),
                            "tool" => {
                                let tool_name = msg.get("name").and_then(|v| v.as_str()).unwrap_or("tool").to_string();
                                Some(ConversationEntry::ToolResult {
                                    tool_name,
                                    preview: text,
                                    success: true,
                                })
                            }
                            _ => None,
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            "checkpoint" => {
                let reason = e.data.get("reason")
                    .and_then(|v| v.as_str())
                    .unwrap_or("Checkpoint")
                    .to_string();
                let note = e.data.get("note").and_then(|v| v.as_str()).map(|s| s.to_string());
                Some(ConversationEntry::EntryCheckpoint { reason, note, created_at: e.created_at })
            }
            "summary" => {
                let summary = e.data.get("summary")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                Some(ConversationEntry::RunSummary {
                    iterations: 0,
                    tool_calls: 0,
                    elapsed_ms: 0,
                })
            }
            _ => None,
        }
    }).collect()
}

/// Format a Unix timestamp as a human-readable age label.
fn format_age(ts: i64) -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let diff = (now - ts).max(0);
    if diff < 60 {
        format!("{diff}s ago")
    } else if diff < 3600 {
        format!("{}m ago", diff / 60)
    } else if diff < 86400 {
        format!("{}h ago", diff / 3600)
    } else {
        format!("{}d ago", diff / 86400)
    }
}

/// Truncate a session ID for display.
fn truncate_id(id: &str) -> String {
    if id.len() > 12 {
        format!("{}...", &id[..12])
    } else {
        id.to_string()
    }
}

/// Sessions panel component.
#[component]
pub fn SessionsPanel() -> Element {
    let app: super::app::AppState = use_context();
    let sessions_signal: Signal<SessionsState> = use_context();
    let conversation_signal: Signal<ConversationState> = use_context();

    // Load sessions on mount
    use_hook(move || {
        let rpc = app.rpc_client.clone();
        let mut sig = sessions_signal;

        sig.with_mut(|s| {
            s.loading = true;
            s.error = None;
        });

        rpc.session_list(move |result| {
            sig.with_mut(|s| {
                s.loading = false;
                match result {
                    Ok(sessions) => { s.sessions = sessions; }
                    Err(e) => { s.error = Some(e); }
                }
            });
        });
    });

    let (sessions, loading, error) = {
        let s = sessions_signal.read();
        (s.sessions.clone(), s.loading, s.error.clone())
    };

    if loading {
        return rsx! {
            div { class: "sessions-panel",
                div { class: "sessions-panel-loading", "Loading sessions..." }
            }
        };
    }

    if let Some(ref e) = error {
        return rsx! {
            div { class: "sessions-panel",
                div { class: "sessions-panel-error", "Error: {e}" }
            }
        };
    }

    if sessions.is_empty() {
        return rsx! {
            div { class: "sessions-panel",
                div { class: "sessions-panel-empty", "No sessions found" }
            }
        };
    }

    let items: Vec<Element> = sessions.iter().map(|session| {
        let session = session.clone();
        let rpc = app.rpc_client.clone();
        let mut conv = conversation_signal;
        let app_state = app.clone();

        rsx! {
            div {
                class: "session-item",
                onclick: move |_: Event<MouseData>| {
                    let sid = session.id.clone();
                    let mut conv_local = conv.clone();
                    let rpc_local = rpc.clone();
                    let mut tab = app_state.active_tab;

                    rpc_local.session_entries(&sid, move |result| {
                        match result {
                            Ok(entries) => {
                                let conv_entries = session_entries_to_conversation(entries);
                                conv_local.with_mut(|s| {
                                    s.entries = conv_entries;
                                });
                                tab.set(crate::state::ActiveTab::Conversation);
                            }
                            Err(e) => {
                                log::error!("Failed to load session entries: {}", e);
                            }
                        }
                    });
                },
                span { class: "session-item-id", "{truncate_id(&session.id)}" }
                span { class: "session-item-count", "{} entries", session.entry_count }
                span { class: "session-item-age", "{format_age(session.created_at)}" }
            }
        }
    }).collect();

    rsx! {
        div { class: "sessions-panel",
            div { class: "sessions-panel-header", "Sessions" }
            {items.into_iter()}
        }
    }
}
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-ui --features web`
Expected: compiles cleanly

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/sessions_panel.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat: add SessionsPanel component with session_entries_to_conversation converter"
```

---

### Task 6: Wire Sessions tab in App (signal, TabBar, TabContent, CSS)

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Import SessionsState and SessionsPanel**

Update imports at the top of `app.rs`:

```rust
use crate::state::{ActiveTab, ApprovalUiState, AgentsState, ConversationState, EventBus, GlobalState, SessionsState, SubscriptionSet, ToolState, UiEvent, UiEventKind, WorkspaceState};
```

Add import:

```rust
use super::sessions_panel::SessionsPanel;
```

- [ ] **Step 2: Add sessions_signal**

In `App()` component, add after `agents_signal` (line ~121):

```rust
    let sessions_signal = use_signal(|| SessionsState::new());
```

- [ ] **Step 3: Provide sessions_signal via context**

Add after `use_context_provider(|| agents_signal);` (line ~285):

```rust
    use_context_provider(|| sessions_signal);
```

- [ ] **Step 4: Add Sessions tab button to TabBar**

Add after Conversation tab button in `TabBar()` (line ~312):

```rust
            TabButton { state: state.clone(), tab: ActiveTab::Sessions, label: "Sessions" }
```

- [ ] **Step 5: Add Sessions tab content to TabContent**

Add to `TabContent()` match (after Conversation, line ~344):

```rust
        ActiveTab::Sessions => rsx! { SessionsPanel {} },
```

- [ ] **Step 6: Add CSS styles for sessions panel**

Add to `GLOBAL_CSS` (before the closing `"#;`):

```css
/* Sessions panel */
.sessions-panel { flex: 1; overflow-y: auto; padding: 8px; }
.sessions-panel-header { padding: 4px 10px 8px; font-size: 12px; font-weight: 600; color: #888; text-transform: uppercase; letter-spacing: 0.5px; }
.sessions-panel-loading, .sessions-panel-empty, .sessions-panel-error { display: flex; align-items: center; justify-content: center; height: 100%; color: #666; padding: 20px; text-align: center; }
.sessions-panel-error { color: #ff6060; }
.session-item { display: flex; align-items: center; padding: 8px 10px; border-bottom: 1px solid #2a2a44; cursor: pointer; gap: 8px; }
.session-item:hover { background: #222240; }
.session-item-id { font-family: monospace; font-size: 13px; color: #e0e0e0; font-weight: 600; min-width: 80px; }
.session-item-count { font-size: 11px; color: #888; }
.session-item-age { font-size: 11px; color: #666; margin-left: auto; }
```

- [ ] **Step 7: Add EntryCheckpoint rendering to Conversation view**

In `crates/vol-llm-ui/src/web/components/conversation.rs`, add a match arm to `MessageEntry` (before the closing `}` of the match, after line 139):

```rust
        ConversationEntry::EntryCheckpoint { reason, note, created_at: _ } => {
            let note_text = note.as_deref().unwrap_or("Previous messages archived.");
            rsx! { div { class: "msg msg-checkpoint", "\u{23f8} Checkpoint: {reason} \u{2014} {note_text}" } }
        }
```

Add CSS for the checkpoint style to `GLOBAL_CSS` in `app.rs` (before the closing `"#;`):

```css
.msg-checkpoint { background: #2a2a20; border-left: 3px solid #c0a040; color: #aaa; font-size: 12px; font-style: italic; padding: 6px 10px; }
```

- [ ] **Step 7b: Remove SessionDialog from web UI render**

In `app.rs`, remove `SessionDialog {}` from the rsx! block (line 299). The component remains in code for TUI use but should not render in web.

Also remove `use super::session_dialog::SessionDialog;` from imports (line 17).

- [ ] **Step 8: Verify compilation**

Run: `cargo check -p vol-llm-ui --features web`
Expected: compiles cleanly

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs crates/vol-llm-ui/src/web/components/conversation.rs crates/vol-llm-ui/src/web/components/session_dialog.rs
git commit -m "feat: wire Sessions tab into App with signal, TabBar, TabContent, CSS, checkpoint rendering, and remove SessionDialog from web UI"
```

---

### Task 7: Update TUI render for Sessions tab

**Files:**
- Modify: `crates/vol-llm-ui/src/tui/render.rs`

- [ ] **Step 1: Add Sessions to TUI tab bar**

Find the tab bar rendering in `render.rs` and add "Sessions" after "Conversation".

- [ ] **Step 2: Add ActiveTab::Sessions match arm**

Find the `ActiveTab` match in the render function and add:

```rust
ActiveTab::Sessions => render_sessions_panel(f, state),
```

- [ ] **Step 3: Add render_sessions_panel placeholder function**

```rust
fn render_sessions_panel(f: &mut Frame, state: &UiState) {
    let area = f.area();
    let block = Block::default()
        .title("Sessions")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));
    f.render_widget(block, area);
    let inner = block.inner(area);
    let text = Text::raw("No sessions (TUI)");
    let paragraph = Paragraph::new(text)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::DarkGray));
    f.render_widget(paragraph, inner);
}
```

- [ ] **Step 4: Update test if needed**

If there's a TUI-specific test for ActiveTab exhaustiveness, update it.

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-ui --features tui`
Expected: compiles cleanly

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/tui/render.rs
git commit -m "feat: add Sessions tab placeholder to TUI render"
```

---

### Task 8: Integration test and full build

- [ ] **Step 1: Full workspace build**

Run: `cargo build --all-features`
Expected: builds cleanly

- [ ] **Step 2: Run all tests**

Run: `cargo test -p vol-llm-agent-channel -p vol-llm-ui`
Expected: all tests pass

- [ ] **Step 3: Start JSON-RPC service and verify session.list**

Run: `cargo run --example jsonrpc_agent_service`
Expected: starts on ws://localhost:3001, lists `session.list, session.entries` in methods

- [ ] **Step 4: Commit**

```bash
git commit --allow-empty -m "chore: verify full build and tests for sessions directory feature"
```
