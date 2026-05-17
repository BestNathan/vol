# File Session + Agent Resume Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `InMemoryEntryStore` with `FileSessionEntryStore` in the JSON-RPC example so agent sessions persist to disk, and add a real `session.resume` RPC that restores a session into the agent's context.

**Architecture:** The example creates a shared `FileSessionEntryStore` passed to both the agent's `Session` and `JsonRpcConnection`. The `session.resume` RPC uses `Session::resume()` to load from disk, swaps the session into the agent dispatcher, and returns entries to the UI.

**Tech Stack:** Rust (axum WebSocket JSON-RPC), Dioxus 0.6 WASM frontend, `FileSessionEntryStore` (vol-session), `ReActAgent`, `AgentDispatcher`.

---

### Task 1: Replace InMemoryEntryStore with FileSessionEntryStore in example

**Files:**
- Modify: `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`

- [ ] **Step 1: Read the file and make the change**

Replace the import and session creation (lines 18 + 46):

```rust
// Change import from:
use vol_session::{InMemoryEntryStore, Session};
// To:
use vol_session::file_store::FileSessionEntryStore;
use vol_session::Session;
```

Replace the session creation (line 46):
```rust
// Before:
let session = Arc::new(Session::new(Arc::new(InMemoryEntryStore::new())));
// After:
let entry_store = Arc::new(FileSessionEntryStore::new("/tmp/vol-llm-store"));
let session = Arc::new(Session::new(entry_store));
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel --example jsonrpc_agent_service`
Expected: compiles cleanly (existing dead_code warnings are fine)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs
git commit -m "feat: use FileSessionEntryStore in jsonrpc_agent_service example for persistent sessions"
```

---

### Task 2: Add `ReActAgent::with_session` method

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Add with_session method**

Add after `with_new_session` method (after line 176):

```rust
    /// Create a new agent with the given session (clones the rest of config).
    pub fn with_session(&self, session: Arc<Session>) -> Self {
        Self {
            config: AgentConfig {
                session,
                ..self.config.clone()
            },
        }
    }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p vol-llm-agent`
Expected: compiles cleanly

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat: add ReActAgent::with_session to create agent with arbitrary session"
```

---

### Task 3: Add `AgentDispatcher::swap_session`

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/dispatcher.rs`

- [ ] **Step 1: Change agent field to RwLock<Arc<ReActAgent>>**

Change the struct (line 34-37):
```rust
pub struct AgentDispatcher {
    agent: std::sync::RwLock<Arc<ReActAgent>>,
    state: Arc<DispatcherState>,
}
```

- [ ] **Step 2: Update new() to use RwLock**

Change `new()` method (lines 43-53):
```rust
    pub fn new(agent: ReActAgent) -> Self {
        let state = Arc::new(DispatcherState::new());
        let agent = Arc::new(std::sync::RwLock::new(Arc::new(agent)));

        tokio::spawn(Self::run_loop(agent.clone(), state.clone()));

        Self { agent, state }
    }
```

- [ ] **Step 3: Update run_loop to read agent from RwLock**

Change `run_loop` (line 119) to clone from the RwLock:
```rust
            let agent = {
                let guard = agent.read().unwrap();
                guard.clone()
            };
            let result = agent.run(&pending.request.input).await;
```

The `run_loop` signature takes `agent: std::sync::RwLock<Arc<ReActAgent>>`. Update the spawn call and function signature accordingly:
```rust
    async fn run_loop(agent: std::sync::RwLock<Arc<ReActAgent>>, state: Arc<DispatcherState>) {
```

- [ ] **Step 4: Add swap_session method**

Add after `queue_len` method:
```rust
    /// Atomically replace the agent's session.
    pub fn swap_session(&self, new_session: Arc<Session>) {
        let old_agent = self.agent.read().unwrap();
        let new_agent = Arc::new(old_agent.with_session(new_session));
        *self.agent.write().unwrap() = new_agent;
    }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: compiles cleanly

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-channel/src/dispatcher.rs
git commit -m "feat: add AgentDispatcher::swap_session for runtime session replacement"
```

---

### Task 4: Real handle_session_resume in JsonRpcConnection

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

- [ ] **Step 1: Add Session import**

Add `Session` to imports at the top:
```rust
use vol_session::Session;
```

- [ ] **Step 2: Replace handle_session_resume stub**

Replace the existing stub method (around line 314-320):
```rust
    /// Handle `session.resume`: resume a session from disk and swap agent session.
    async fn handle_session_resume(&self, id: u64, session_id: String) -> String {
        let entry_store = &self.entry_store;

        // Resume session from disk
        let session = match Session::resume(session_id.clone(), entry_store.clone()).await {
            Ok(s) => Arc::new(s),
            Err(e) => {
                return to_jsonrpc_error(Some(id), -32000, format!("Failed to resume session: {e}"));
            }
        };

        // Swap session in all dispatchers
        for dispatcher in self.dispatchers.values() {
            dispatcher.swap_session(session.clone());
        }

        // Return session info + entries for UI display
        match entry_store.get_entries(&session_id).await {
            Ok(entries) => {
                let json_entries: Vec<serde_json::Value> = entries
                    .into_iter()
                    .filter_map(|e| serde_json::to_value(e).ok())
                    .collect();
                to_jsonrpc_response(id, serde_json::json!({
                    "session_id": session_id,
                    "entry_count": entries.len(),
                    "entries": json_entries,
                }))
            }
            Err(e) => to_jsonrpc_error(Some(id), -32000, format!("Failed to read entries: {e}")),
        }
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-agent-channel`
Expected: compiles cleanly

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/connection.rs
git commit -m "feat: implement real handle_session_resume RPC that swaps agent session from disk"
```

---

### Task 5: Add session_resume RPC client method to frontend

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add SessionResumeResponse struct**

Add after `SessionEntry` struct:
```rust
/// Response from session.resume RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionResumeResponse {
    pub session_id: String,
    pub entry_count: usize,
    pub entries: Vec<SessionEntry>,
}
```

- [ ] **Step 2: Add session_resume method**

Add after `session_entries` method:
```rust
    /// Resume a session on the server (swaps agent session). Returns response via callback.
    pub fn session_resume(&self, session_id: &str, cb: impl FnOnce(Result<SessionResumeResponse, String>) + 'static) {
        let id = self.alloc_id();

        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "session.resume",
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
            let session_id = result.get("session_id").and_then(|v| v.as_str())?.to_string();
            let entry_count = result.get("entry_count").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let entries = result.get("entries").and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|e| serde_json::from_value(e.clone()).ok()).collect())
                .unwrap_or_default();
            cb(Ok(SessionResumeResponse { session_id, entry_count, entries }));
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p vol-llm-ui --features web`
Expected: compiles cleanly (zero new errors beyond pre-existing ones)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat: add session_resume RPC client method with SessionResumeResponse"
```

---

### Task 6: Add Resume button to SessionsPanel

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

- [ ] **Step 1: Read the file to confirm current state**

The file should already have `session_entries_to_conversation`, `format_age`, `truncate_id`, and `SessionsPanel`.

- [ ] **Step 2: Add a separate SessionItem component**

Instead of inline rsx with onclick, create a `SessionItem` component that has a Resume button. Add after `truncate_id`:

```rust
#[component]
fn SessionItem(
    session: crate::state::SessionListEntry,
    rpc: crate::web::client::JsonRpcClient,
    sessions_signal: Signal<SessionsState>,
    conversation_signal: Signal<ConversationState>,
    active_tab: Signal<ActiveTab>,
) -> Element {
    let mut is_resuming = use_signal(|| false);

    rsx! {
        div {
            class: "session-item",
            onclick: move |_: Event<MouseData>| {
                // View-only: load history without swapping agent session
                let sid = session.id.clone();
                let mut conv = conversation_signal.clone();
                let rpc = rpc.clone();
                let mut tab = active_tab;

                rpc.session_entries(&sid, move |result| {
                    match result {
                        Ok(entries) => {
                            let conv_entries = session_entries_to_conversation(entries);
                            conv.with_mut(|s| { s.entries = conv_entries; });
                            tab.set(ActiveTab::Conversation);
                        }
                        Err(e) => log::error!("Failed to load session: {}", e),
                    }
                });
            },
            span { class: "session-item-id", "{truncate_id(&session.id)}" }
            span { class: "session-item-count", "{} entries", session.entry_count }
            span { class: "session-item-age", "{format_age(session.created_at)}" }
            button {
                class: "session-resume-btn",
                onclick: move |evt: Event<MouseData>| {
                    evt.stop_propagation();
                    let mut resuming = is_resuming;
                    let sid = session.id.clone();
                    let mut conv = conversation_signal.clone();
                    let rpc = rpc.clone();
                    let mut tab = active_tab;

                    resuming.set(true);
                    rpc.session_resume(&sid, move |result| {
                        resuming.set(false);
                        match result {
                            Ok(resp) => {
                                let conv_entries = session_entries_to_conversation(resp.entries);
                                conv.with_mut(|s| { s.entries = conv_entries; });
                                tab.set(ActiveTab::Conversation);
                            }
                            Err(e) => log::error!("Failed to resume session: {}", e),
                        }
                    });
                },
                if is_resuming.read().clone() { "Resuming..." } else { "Resume" }
            }
        }
    }
}
```

- [ ] **Step 3: Update SessionsPanel to use SessionItem**

Replace the inline items generation in `SessionsPanel`:

```rust
    let items: Vec<Element> = sessions.iter().map(|session| {
        let app = app.clone();
        rsx! {
            SessionItem {
                session: session.clone(),
                rpc: app.rpc_client.clone(),
                sessions_signal,
                conversation_signal,
                active_tab: app_state.active_tab,
            }
        }
    }).collect();
```

- [ ] **Step 4: Add CSS for Resume button**

Add to `GLOBAL_CSS` in `app.rs`:
```css
.session-resume-btn { padding: 3px 10px; background: #408040; color: #e0e0e0; border: none; border-radius: 3px; cursor: pointer; font-size: 12px; margin-left: auto; }
.session-resume-btn:hover { background: #50a050; }
.session-resume-btn:disabled { background: #333355; cursor: not-allowed; }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check -p vol-llm-ui --features web`
Expected: compiles cleanly

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/sessions_panel.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: add Resume button to SessionsPanel that swaps agent session and loads history"
```

---

### Task 7: Integration test and full build

- [ ] **Step 1: Full workspace build**

Run: `cargo build -p vol-llm-agent-channel -p vol-llm-agent`
Expected: builds cleanly

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-agent-channel -p vol-llm-agent`
Expected: all tests pass

- [ ] **Step 3: Manual verification**

Start the service:
```bash
cargo run --example jsonrpc_agent_service
```

Verify it starts on `ws://localhost:3001` with all methods listed. Submit a conversation via the web UI, then check that a `.jsonl` file was created in `/tmp/vol-llm-store/`. Kill the service, restart, and verify the session appears in the Sessions tab.
