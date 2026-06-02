# Skills Panel Content Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Populate the Skills panel with live skill data from the backend via JSON-RPC, with a detail dialog for viewing individual skills.

**Architecture:** Two new `skill.list` and `skill.get` RPC methods on the existing JSON-RPC server, backed by the existing `SkillLoader`. Frontend calls RPC on mount, shows a modal dialog on row click.

**Tech Stack:** Rust (vol-llm-agent-channel, vol-llm-skill), Dioxus (vol-llm-ui web)

---

### Task 1: Add vol-llm-skill dependency to vol-llm-agent-channel

**Files:**
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`

- [ ] **Step 1.1: Add dependency**

Add `vol-llm-skill` to `[dependencies]` in `crates/vol-llm-agent-channel/Cargo.toml`:

```toml
vol-llm-skill = { path = "../vol-llm-skill" }
```

Also add to `[dev-dependencies]` for tests:

```toml
vol-llm-skill = { path = "../vol-llm-skill" }
```

- [ ] **Step 1.2: Verify it compiles**

Run: `cargo check -p vol-llm-agent-channel`
Expected: success (no code changes yet, just dependency resolution)

- [ ] **Step 1.3: Commit**

```bash
git add crates/vol-llm-agent-channel/Cargo.toml
git commit -m "chore: add vol-llm-skill dependency to vol-llm-agent-channel"
```

---

### Task 2: Add skill RPC methods to JSON-RPC request parsing

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs:29-127` (add enum variants)
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs:384-553` (add parse arms)
- Test: `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs` (existing tests section)

- [ ] **Step 2.1: Add enum variants**

Add these two variants to the `JsonRpcRequest` enum in `serde_helpers.rs`, before the `Unknown` variant:

```rust
    SkillList {
        id: u64,
    },
    SkillGet {
        id: u64,
        name: String,
    },
```

- [ ] **Step 2.2: Add parse arms**

In `parse_jsonrpc_request()`, add these arms before the `_ =>` catch-all (before line 549):

```rust
        "skill.list" => Ok(JsonRpcRequest::SkillList { id }),
        "skill.get" => {
            let name = params
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| "skill.get: missing 'name'".to_string())?
                .to_string();
            Ok(JsonRpcRequest::SkillGet { id, name })
        }
```

- [ ] **Step 2.3: Add unit tests for parsing**

Add to the test section at the bottom of `serde_helpers.rs`:

```rust
    #[test]
    fn test_parse_skill_list() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"skill.list","params":{}}"#;
        let req = parse_jsonrpc_request(json).unwrap();
        match req {
            JsonRpcRequest::SkillList { id } => assert_eq!(id, 1),
            _ => panic!("expected SkillList"),
        }
    }

    #[test]
    fn test_parse_skill_get() {
        let json = r#"{"jsonrpc":"2.0","id":2,"method":"skill.get","params":{"name":"brainstorming"}}"#;
        let req = parse_jsonrpc_request(json).unwrap();
        match req {
            JsonRpcRequest::SkillGet { id, name } => {
                assert_eq!(id, 2);
                assert_eq!(name, "brainstorming");
            }
            _ => panic!("expected SkillGet"),
        }
    }

    #[test]
    fn test_parse_skill_get_missing_name() {
        let json = r#"{"jsonrpc":"2.0","id":3,"method":"skill.get","params":{}}"#;
        let result = parse_jsonrpc_request(json);
        assert!(result.is_err(), "should fail without name param");
    }
```

- [ ] **Step 2.4: Run the tests**

Run: `cargo test -p vol-llm-agent-channel jsonrpc::serde_helpers::test -- --nocapture`
Expected: all 3 new tests pass + existing tests pass

- [ ] **Step 2.5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs
git commit -m "feat: add skill.list and skill.get JSON-RPC request parsing"
```

---

### Task 3: Wire SkillLoader into JsonRpcServer and JsonRpcConnection

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/server.rs`
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

- [ ] **Step 3.1: Add SkillLoader to JsonRpcServer**

In `server.rs`, add import and field:

```rust
use vol_llm_skill::loader::SkillLoader;
```

Add field to `JsonRpcServer` struct:

```rust
pub struct JsonRpcServer {
    router: AgentRouter,
    dispatchers: HashMap<String, Arc<AgentDispatcher>>,
    holders: HashMap<String, Arc<ConnectionHolder>>,
    working_dir: String,
    store_dir: String,
    mcp_manager: Option<Arc<McpManager>>,
    skill_loader: Option<Arc<SkillLoader>>,  // NEW
}
```

Add parameter to `JsonRpcServer::new()`:

```rust
    pub async fn new(
        agents: Vec<AgentRegistration>,
        working_dir: String,
        store_dir: String,
        mcp_manager: Option<Arc<McpManager>>,
        skill_loader: Option<Arc<SkillLoader>>,  // NEW
    ) -> Self {
```

Store it in `Self { ... }`:

```rust
        Self { router, dispatchers, holders, working_dir, store_dir, mcp_manager, skill_loader }
```

Update `into_axum_router()` to pass skill_loader to `handle_ws`:

```rust
    pub fn into_axum_router(self) -> Router {
        let server = Arc::new(self);

        Router::new()
            .route(
                "/ws",
                get(move |ws: WebSocketUpgrade| {
                    let server = server.clone();
                    async move { ws.on_upgrade(move |socket| handle_ws(socket, server)) }
                }),
            )
    }
```

Update `handle_ws` to pass skill_loader to `JsonRpcConnection::new()`:

```rust
async fn handle_ws(socket: WebSocket, server: Arc<JsonRpcServer>) {
    let session_store = Arc::new(vol_session::FileSessionEntryStore::new(&server.store_dir));
    let conn = JsonRpcConnection::new(
        socket,
        server.router.clone(),
        server.dispatchers.clone(),
        server.holders.clone(),
        server.working_dir.clone(),
        server.store_dir.clone(),
        session_store,
        server.mcp_manager.clone(),
        server.skill_loader.clone(),  // NEW
    );
    let conn_arc = Arc::new(conn);
    conn_arc.run().await;
}
```

- [ ] **Step 3.2: Add SkillLoader to JsonRpcConnection**

In `connection.rs`, add import:

```rust
use vol_llm_skill::loader::SkillLoader;
```

Add field to `JsonRpcConnection`:

```rust
    skill_loader: Option<Arc<SkillLoader>>,
```

Add parameter to `JsonRpcConnection::new()`:

```rust
    pub fn new(
        ws: WebSocket,
        router: AgentRouter,
        dispatchers: HashMap<String, Arc<AgentDispatcher>>,
        holders: HashMap<String, Arc<ConnectionHolder>>,
        working_dir: String,
        store_dir: String,
        session_store: Arc<vol_session::FileSessionEntryStore>,
        mcp_manager: Option<Arc<McpManager>>,
        skill_loader: Option<Arc<SkillLoader>>,  // NEW
    ) -> Self {
```

Store it in `Self { ... }`:

```rust
        Self {
            // ... existing fields ...
            skill_loader,
        }
```

- [ ] **Step 3.3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/server.rs \
  crates/vol-llm-agent-channel/src/jsonrpc/connection.rs
git commit -m "feat: wire SkillLoader into JsonRpcServer and JsonRpcConnection"
```

---

### Task 4: Implement skill.list and skill.get handlers

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs`

- [ ] **Step 4.1: Add dispatch arms in handle_text_frame**

In the `handle_text_frame` match, add before the `Unknown` arm:

```rust
            JsonRpcRequest::SkillList { id } => {
                self.handle_skill_list(*id).await
            }
            JsonRpcRequest::SkillGet { id, name } => {
                self.handle_skill_get(*id, name.clone()).await
            }
```

- [ ] **Step 4.2: Implement handle_skill_list**

Add to the `impl JsonRpcConnection` block (after existing handlers):

```rust
    async fn handle_skill_list(&self, id: u64) -> String {
        let Some(loader) = &self.skill_loader else {
            return to_jsonrpc_response(id, serde_json::json!({ "skills": [] }));
        };
        let metadata = loader.list_metadata().await;
        let skills: Vec<serde_json::Value> = metadata.iter().map(|m| {
            serde_json::json!({
                "id": m.id,
                "name": m.name,
                "version": m.version,
                "scope": m.scope.to_string(),
                "description": m.description,
                "triggers": m.triggers,
            })
        }).collect();
        to_jsonrpc_response(id, serde_json::json!({ "skills": skills }))
    }
```

- [ ] **Step 4.3: Implement handle_skill_get**

```rust
    async fn handle_skill_get(&self, id: u64, name: String) -> String {
        let Some(loader) = &self.skill_loader else {
            return to_jsonrpc_error(Some(id), -32000, "Skills not configured".to_string());
        };
        match loader.get(&name).await {
            Some(skill) => to_jsonrpc_response(id, serde_json::json!({
                "name": skill.name,
                "version": skill.version,
                "scope": skill.scope.to_string(),
                "description": skill.description,
                "triggers": skill.triggers,
                "content": skill.content,
                "file_listing": skill.file_listing,
            })),
            None => to_jsonrpc_error(Some(id), -32000, format!("Skill '{name}' not found")),
        }
    }
```

- [ ] **Step 4.4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/jsonrpc/connection.rs
git commit -m "feat: implement skill.list and skill.get RPC handlers"
```

---

### Task 5: Update the example service to use SkillLoader

**Files:**
- Modify: `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`

- [ ] **Step 5.1: Add imports and create SkillLoader**

Add these imports near the top:

```rust
use vol_llm_skill::loader::SkillLoader;
use std::sync::Arc;
```

After the MCP manager block (around line 87), add:

```rust
    // Create skill loader and discover skills
    let skill_loader = {
        let loader = Arc::new(SkillLoader::new(Some(std::path::PathBuf::from("."))));
        let loader_for_discover = loader.clone();
        tokio::spawn(async move {
            if let Err(e) = loader_for_discover.discover_all().await {
                tracing::warn!("Failed to discover skills: {}", e);
            }
        });
        Some(loader)
    };
```

- [ ] **Step 5.2: Pass skill_loader to JsonRpcServer::new()**

Update the `JsonRpcServer::new()` call to include the new parameter:

```rust
    let server = JsonRpcServer::new(
        vec![AgentRegistration {
            agent_id: "general-assistant".to_string(),
            dispatcher,
            holder,
        }],
        ".".to_string(),
        "/tmp/vol-llm-store".to_string(),
        Some(mcp_manager),
        skill_loader,  // NEW
    ).await;
```

- [ ] **Step 5.3: Update the log message**

Add to the tracing info block:

```rust
    tracing::info!("           skill.list, skill.get");
```

- [ ] **Step 5.4: Commit**

```bash
git add crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs
git commit -m "feat: add SkillLoader to jsonrpc_agent_service example"
```

---

### Task 6: Add skill types and RPC methods to the web frontend

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 6.1: Add SkillDetail and update SkillsState**

In `state/mod.rs`, add this struct near `SkillDisplayEntry` (around line 190):

```rust
/// Full skill detail returned by skill.get RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillDetail {
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
    pub triggers: Vec<String>,
    pub content: String,
    pub file_listing: Vec<String>,
}
```

Update `SkillsState` (around line 462):

```rust
/// Local state for SkillsPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct SkillsState {
    pub skills: Vec<SkillDisplayEntry>,
    pub error: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SkillsState {
    pub fn new() -> Self {
        Self { skills: Vec::new(), error: None }
    }
}
```

Also add `SkillDialogState` after `SkillsState`:

```rust
/// Dialog state for viewing a skill's full details.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug, Clone)]
pub struct SkillDialogState {
    pub open: bool,
    pub skill: Option<SkillDetail>,
    pub loading: bool,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl SkillDialogState {
    pub fn new() -> Self {
        Self { open: false, skill: None, loading: false }
    }
}
```

- [ ] **Step 6.2: Add skill_list and skill_get RPC methods to JsonRpcClient**

In `client.rs`, add the `SkillDisplayEntry` type for the list response:

```rust
/// Skill metadata returned by skill.list RPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillListEntry {
    pub id: String,
    pub name: String,
    pub version: String,
    pub scope: String,
    pub description: String,
    pub triggers: Vec<String>,
}
```

Add `skill_list` method to `impl JsonRpcClient` (before `handle_message`):

```rust
    /// List all discovered skills.
    pub fn skill_list(&self, cb: impl FnOnce(Result<Vec<SkillListEntry>, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "skill.list",
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
            match result.get("skills").and_then(|v| v.as_array()) {
                Some(skills) => {
                    let parsed: Vec<SkillListEntry> = skills.iter()
                        .filter_map(|s| serde_json::from_value(s.clone()).ok())
                        .collect();
                    cb(Ok(parsed));
                }
                None => cb(Err("no skills in response".to_string())),
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }
```

Add `skill_get` method:

```rust
    /// Get full skill details by name.
    pub fn skill_get(&self, name: &str, cb: impl FnOnce(Result<crate::state::SkillDetail, String>) + 'static) {
        let id = self.alloc_id();
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "skill.get",
            "params": { "name": name },
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
            if let Some(error) = result.get("error") {
                let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("unknown error");
                cb(Err(msg.to_string()));
            } else {
                match serde_json::from_value::<crate::state::SkillDetail>(result) {
                    Ok(detail) => cb(Ok(detail)),
                    Err(e) => cb(Err(format!("failed to parse skill: {e}"))),
                }
            }
        });
        self.inner.pending.borrow_mut().insert(id, cb);
    }
```

- [ ] **Step 6.3: Compile check**

Run: `cargo check -p vol-llm-ui --features web`
Expected: success

- [ ] **Step 6.4: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs \
  crates/vol-llm-ui/src/web/client.rs
git commit -m "feat: add skill RPC types and client methods to web frontend"
```

---

### Task 7: Create SkillDetailDialog component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mod.rs`

- [ ] **Step 7.1: Create the component**

Create `crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs`:

```rust
//! Dialog showing full details of a skill.

use dioxus::prelude::*;
use crate::state::SkillDialogState;

#[component]
pub fn SkillDetailDialog(mut signal: Signal<SkillDialogState>) -> Element {
    let (open, skill, loading) = {
        let s = signal.read();
        (s.open, s.skill.clone(), s.loading)
    };

    if !open {
        return rsx! {};
    }

    rsx! {
        div { class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            div { class: "bg-[#1a1a2e] border border-[#3a3a55] rounded-lg p-4 w-[650px] max-w-[90vw] max-h-[85vh] flex flex-col",
                // Header
                div { class: "flex items-center justify-between mb-3",
                    div { class: "flex items-center gap-2",
                        if let Some(ref s) = skill {
                            span { class: "text-[16px] font-semibold text-[#e0e0e0]", "{s.name}" }
                            span { class: "text-[11px] text-[#888] bg-[#2a2a44] px-1.5 py-0.5 rounded", "v{s.version}" }
                            span { class: "text-[11px] px-1.5 py-0.5 rounded", style: "color: {if s.scope == \"User\" { \"#40c040\" } else { \"#4080ff\" }}; background: #2a2a44;", "{s.scope}" }
                        }
                    }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px]",
                        onclick: move |_| {
                            let mut s = signal.write_unchecked();
                            s.open = false;
                            s.skill = None;
                        },
                        "x"
                    }
                }
                if loading {
                    div { class: "text-[#888] text-[13px] py-8 text-center", "Loading skill details..." }
                } else if let Some(ref detail) = skill {
                    // Description
                    div { class: "text-[#ccc] text-[13px] mb-3", "{detail.description}" }

                    // Triggers
                    if !detail.triggers.is_empty() {
                        div { class: "flex gap-1.5 flex-wrap mb-3",
                            {detail.triggers.iter().map(|t| {
                                rsx! {
                                    span { class: "text-[11px] text-[#c0c040] bg-[#2a2a20] px-2 py-0.5 rounded", "{t}" }
                                }
                            }).collect::<Vec<Element>>()}
                        }
                    }

                    // SKILL.md body
                    div { class: "bg-[#12121e] border border-[#2a2a44] rounded p-2 max-h-48 overflow-y-auto mb-3",
                        pre { class: "text-[12px] text-[#aaa] font-mono whitespace-pre-wrap", "{detail.content}" }
                    }

                    // File listing
                    if !detail.file_listing.is_empty() {
                        div { class: "text-[11px] text-[#888] mb-1 font-semibold", "Files" }
                        div { class: "bg-[#12121e] border border-[#2a2a44] rounded max-h-32 overflow-y-auto mb-3",
                            {detail.file_listing.iter().map(|f| {
                                rsx! {
                                    div { class: "text-[12px] text-[#aaa] font-mono px-2 py-0.5 border-b border-[#2a2a44] last:border-b-0", "{f}" }
                                }
                            }).collect::<Vec<Element>>()}
                        }
                    }
                } else {
                    div { class: "text-[#c04040] text-[13px] py-4 text-center", "Failed to load skill details" }
                }
            }
        }
    }
}
```

- [ ] **Step 7.2: Export from mod.rs**

In `crates/vol-llm-ui/src/web/components/mod.rs`, add:

```rust
pub mod skill_detail_dialog;
```

And add the re-export:

```rust
pub use skill_detail_dialog::SkillDetailDialog;
```

- [ ] **Step 7.3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/skill_detail_dialog.rs \
  crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat: add SkillDetailDialog component"
```

---

### Task 8: Wire SkillsPanel to RPC and integrate dialog into App

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/skills.rs`
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 8.1: Update SkillsPanel to fetch skills via RPC**

Replace the entire `SkillsPanel` component in `skills.rs`:

```rust
//! Skills panel showing available skills.

use dioxus::prelude::*;
use crate::state::{SkillDialogState, SkillsState};
use crate::web::components::app::AppState;

#[component]
pub fn SkillsPanel(mut dialog_signal: Signal<SkillDialogState>) -> Element {
    let app_state: AppState = use_context();
    let rpc_client = app_state.rpc_client.clone();

    let signal = use_signal(|| SkillsState::new());

    // Fetch skills on mount
    use_effect(move || {
        let client = rpc_client.clone();
        let sig = signal.clone();
        client.skill_list(move |result| {
            match result {
                Ok(entries) => {
                    sig.write_unchecked().skills = entries.iter().map(|e| {
                        crate::state::SkillDisplayEntry {
                            name: e.name.clone(),
                            version: e.version.clone(),
                            scope: e.scope.clone(),
                            description: e.description.clone(),
                        }
                    }).collect();
                    sig.write_unchecked().error = None;
                }
                Err(e) => {
                    sig.write_unchecked().error = Some(e);
                }
            }
        });
    });

    let count = signal.read().skills.len();
    let error = signal.read().error.clone();

    if let Some(err) = error {
        let retry_client = rpc_client.clone();
        let retry_sig = signal.clone();
        return rsx! {
            div { class: "flex-1 overflow-y-auto p-2.5",
                div { class: "flex flex-col items-center justify-center h-full text-[#c04040]",
                    "Failed to load skills"
                    button {
                        class: "mt-2 px-3 py-1 bg-[#4080ff] text-white rounded text-[12px] cursor-pointer hover:bg-[#5090ff]",
                        onclick: move |_| {
                            let client = retry_client.clone();
                            let sig = retry_sig.clone();
                            client.skill_list(move |result| {
                                match result {
                                    Ok(entries) => {
                                        sig.write_unchecked().skills = entries.iter().map(|e| {
                                            crate::state::SkillDisplayEntry {
                                                name: e.name.clone(),
                                                version: e.version.clone(),
                                                scope: e.scope.clone(),
                                                description: e.description.clone(),
                                            }
                                        }).collect();
                                        sig.write_unchecked().error = None;
                                    }
                                    Err(e) => { sig.write_unchecked().error = Some(e); }
                                }
                            });
                        },
                        "Retry"
                    }
                }
            }
        };
    }

    if count == 0 {
        return rsx! { div { class: "flex-1 overflow-y-auto p-2.5", div { class: "flex items-center justify-center h-full text-[#666]", "No skills discovered" } } };
    }
    rsx! {
        div { class: "flex-1 overflow-y-auto p-2.5",
            table { class: "skills-table",
                thead { tr {
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Name" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Version" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Scope" }
                    th { class: "text-left px-2 py-1 border-b border-[#333355] text-[12px] text-[#888]", "Description" }
                } }
                tbody {
                    {(0..count).map(|i| { let s = signal.clone(); let d = dialog_signal.clone(); let c = rpc_client.clone(); rsx! { SkillRow { signal: s, dialog_signal: d, rpc_client: c, index: i } } }).collect::<Vec<Element>>().into_iter()}
                }
            }
        }
    }
}

#[component]
fn SkillRow(
    signal: Signal<SkillsState>,
    mut dialog_signal: Signal<SkillDialogState>,
    rpc_client: crate::web::client::JsonRpcClient,
    index: usize,
) -> Element {
    let skill = signal.read().skills.get(index).cloned();
    let Some(skill) = skill else { return rsx! {}; };

    let color = match skill.scope.as_str() { "User" => "#40c040", "Repo" => "#4080ff", _ => "#c0c040" };

    rsx! {
        tr {
            class: "cursor-pointer hover:bg-[#2a2a44]",
            onclick: move |_| {
                let client = rpc_client.clone();
                let name = skill.name.clone();
                let mut d = dialog_signal.write_unchecked();
                d.open = true;
                d.skill = None;
                d.loading = true;
                client.skill_get(&name, move |result| {
                    match result {
                        Ok(detail) => {
                            d.skill = Some(detail);
                        }
                        Err(_) => {
                            d.skill = None;
                        }
                    }
                    d.loading = false;
                });
            },
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#e0e0e0] font-bold", "{skill.name}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.version}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44]", style: "color: {color};", "{skill.scope}" }
            td { class: "px-2 py-1 text-[13px] border-b border-[#2a2a44] text-[#888]", "{skill.description}" }
        }
    }
}
```

- [ ] **Step 8.2: Add SkillDialogState to App and render dialog**

In `app.rs`, add import:

```rust
use crate::state::SkillDialogState;
```

Add the signal in `App()` component (near other signal declarations):

```rust
    let skill_dialog_signal = use_signal(|| SkillDialogState::new());
```

Add to context providers:

```rust
    use_context_provider(|| skill_dialog_signal);
```

Update the `SkillsPanel` render in `TabContent()`:

```rust
        ActiveTab::Skills => rsx! { SkillsPanel { dialog_signal: skill_dialog_signal } },
```

Add the dialog rendering in the main `rsx!` block in `App()` (near other dialogs):

```rust
            SkillDetailDialog { signal: skill_dialog_signal }
```

- [ ] **Step 8.3: Compile check**

Run: `cargo check -p vol-llm-ui --features web`
Expected: success

- [ ] **Step 8.4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/skills.rs \
  crates/vol-llm-ui/src/web/components/app.rs
git commit -m "feat: wire SkillsPanel to RPC and integrate SkillDetailDialog into App"
```

---

### Task 9: Add integration test for skill RPC

**Files:**
- Modify: `crates/vol-llm-agent-channel/tests/jsonrpc_integration.rs` (if exists) or `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs` (in tests section)

- [ ] **Step 9.1: Find or check existing integration test file**

Read `crates/vol-llm-agent-channel/tests/jsonrpc_integration.rs` to see if there's an existing test framework for RPC round-trips.

- [ ] **Step 9.2: If integration test exists, add skill tests**

If the file exists and has a test pattern similar to other RPC methods, add:

```rust
#[tokio::test]
async fn test_skill_list_empty() {
    // Server without skills configured should return empty list
    let (client, _) = setup_server().await;
    let msg = r#"{"jsonrpc":"2.0","id":1,"method":"skill.list","params":{}}"#;
    client.send(msg).await.unwrap();
    let resp = client.recv().await.unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(parsed["id"], 1);
    assert_eq!(parsed["result"]["skills"].as_array().unwrap().len(), 0);
}
```

- [ ] **Step 9.3: If no integration test infrastructure exists, skip**

Skip — the unit tests in `serde_helpers.rs` (Task 2) cover the parsing layer, and manual testing covers the rest.

- [ ] **Step 9.4: Commit (if changes made)**

```bash
git add crates/vol-llm-agent-channel/tests/jsonrpc_integration.rs
git commit -m "test: add skill.list integration test"
```

---

### Task 10: Final compile and verification

- [ ] **Step 10.1: Full workspace compile check**

Run: `cargo check -p vol-llm-agent-channel -p vol-llm-ui --features web`
Expected: success

- [ ] **Step 10.2: Run all tests**

Run: `cargo test -p vol-llm-agent-channel`
Expected: all tests pass

- [ ] **Step 10.3: Commit any fixes**

```bash
git add -A
git commit -m "fix: resolve compile errors and test failures"
```
