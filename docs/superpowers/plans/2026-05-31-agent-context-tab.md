# Agent Context Tab Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a "Context" sub-tab in the agent panel showing contributor list with metadata, click to open modal with full content snapshot.

**Architecture:** Two new `ContextBuilder` methods (`contributor_infos`, `snapshot_by_name`) provide data. Backend exposes them via two new JSON-RPC operations handled by `AgentHandler`, accessed through a new `AgentDispatcher::with_agent` method. Frontend adds `ContextPanel` with inline `ContextDialog` modal, wired as third sub-tab in `AgentsPanel`.

**Tech Stack:** Rust/tokio, Dioxus 0.6 WASM, ContextContributor trait

---

## File Map

| Action | File | Purpose |
|--------|------|---------|
| Modify | `crates/vol-llm-context/src/builder.rs` | Add `ContributorInfo`, `ContextMessage`, `contributor_infos()`, `snapshot_by_name()` |
| Modify | `crates/vol-llm-agent-channel/src/dispatcher.rs` | Add `get_agent` accessor |
| Modify | `crates/vol-llm-agent-channel/src/router.rs` | Add `get_agent` lookup |
| Modify | `crates/vol-llm-agent-channel/src/agent_server_protocol.rs` | Add ops and payloads |
| Modify | `crates/vol-llm-agent-channel/src/domain/agent.rs` | Handle new operations |
| Modify | `crates/vol-llm-ui/src/state/mod.rs` | Add `Context` variant, `ContextState` |
| Modify | `crates/vol-llm-ui/src/web/client.rs` | Add two JSON-RPC client methods |
| Create | `crates/vol-llm-ui/src/web/components/context_panel.rs` | ContextPanel + ContextDialog |
| Modify | `crates/vol-llm-ui/src/web/components/mod.rs` | Re-export |
| Modify | `crates/vol-llm-ui/src/web/components/agents_panel.rs` | Sub-tab + routing |
| Modify | `crates/vol-llm-ui/src/web/components/app.rs` | Provide ContextState signal |

---

### Task 1: Add `ContributorInfo` + methods to ContextBuilder

**Files:**
- Modify: `crates/vol-llm-context/src/builder.rs`

`ContextBuilder` needs two new public methods consumed by the backend handler.

- [ ] **Step 1: Add types and method signatures**

In `builder.rs`, after `ContextOutput` (line 7), add:

```rust
/// Metadata about a context contributor for UI display.
#[derive(Debug, Clone)]
pub struct ContributorInfo {
    pub name: String,
    pub anchor_zone: String,
    pub estimated_tokens: usize,
    pub message_count: usize,
}

/// A message from a contributor snapshot, suitable for frontend display.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ContextMessage {
    pub role: String,
    pub content: String,
}
```

Add `serde` to `vol-llm-context` dependencies if not already present:
```toml
# crates/vol-llm-context/Cargo.toml
serde = { workspace = true }
```

In `impl ContextBuilder`, after `contributor_names()` (line 37), add:

```rust
/// Get info for all contributors (calls contribute() for message_count + anchor_zone).
pub async fn contributor_infos(&self) -> Result<Vec<ContributorInfo>, ContextError> {
    let mut infos = Vec::new();
    for contributor in &self.contributors {
        let blocks = contributor.contribute().await?;
        let anchor_zone = blocks
            .first()
            .map(|b| match b.anchor {
                AttentionAnchor::Head(_) => "head",
                AttentionAnchor::Middle(_) => "middle",
                AttentionAnchor::Tail(_) => "tail",
            })
            .unwrap_or("unknown")
            .to_string();
        let message_count: usize = blocks.iter().map(|b| b.messages.len()).sum();
        infos.push(ContributorInfo {
            name: contributor.name().to_string(),
            anchor_zone,
            estimated_tokens: contributor.estimate_size(),
            message_count,
        });
    }
    Ok(infos)
}

/// Get full message snapshot from a named contributor.
pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
    for contributor in &self.contributors {
        if contributor.name() == name {
            let blocks = contributor.contribute().await?;
            let messages: Vec<ContextMessage> = blocks
                .into_iter()
                .flat_map(|b| b.messages)
                .map(|msg| {
                    let role = msg.role.to_string();
                    let content = msg
                        .content
                        .as_ref()
                        .map(|c| c.as_str().to_string())
                        .unwrap_or_default();
                    ContextMessage { role, content }
                })
                .collect();
            return Ok(messages);
        }
    }
    Err(ContextError::ContributorError(
        name.to_string(),
        "contributor not found".to_string(),
    ))
}
```

- [ ] **Step 2: Run tests**

```bash
cd crates/vol-llm-context && cargo test 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-context/src/builder.rs crates/vol-llm-context/Cargo.toml
git commit -m "feat(context): add ContributorInfo, ContextMessage, contributor_infos(), snapshot_by_name()"
```

---

### Task 2: Add agent accessor methods

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/dispatcher.rs`
- Modify: `crates/vol-llm-agent-channel/src/router.rs`

- [ ] **Step 1: Add `get_agent` to `AgentDispatcher`**

In `impl AgentDispatcher`, after `is_busy()` (line 105), before `run_loop()`:

```rust
/// Clone the wrapped agent (read-only access via Arc clone).
pub fn get_agent(&self) -> Arc<ReActAgent> {
    self.agent.read().unwrap().clone()
}
```

- [ ] **Step 2: Add `get_agent` to `AgentRouter`**

In `crates/vol-llm-agent-channel/src/router.rs`, in `impl AgentRouter`, after `list_agents()` (line 83):

```rust
/// Clone the agent for the given agent_id.
pub async fn get_agent(&self, agent_id: &str) -> Option<Arc<ReActAgent>> {
    self.dispatchers
        .read()
        .await
        .get(agent_id)
        .map(|d| d.get_agent())
}
```

Add the import at top of router.rs:
```rust
use vol_llm_agent::ReActAgent;
```

- [ ] **Step 3: Run check**

```bash
cd crates/vol-llm-agent-channel && cargo check 2>&1 | tail -5
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/dispatcher.rs crates/vol-llm-agent-channel/src/router.rs
git commit -m "feat(agent-channel): add get_agent accessor to AgentDispatcher and AgentRouter"
```

---

### Task 3: Add protocol operations and payloads

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`

- [ ] **Step 1: Add enum variants to `AgentOperation`**

After `Status,`:

```rust
    ContextConfig,
    ContextSnapshot,
```

- [ ] **Step 2: Add method_name mappings in `Operation::method_name()`**

After the `AgentOperation::Status` line:

```rust
Operation::Agent(AgentOperation::ContextConfig) => "agent.context_config",
Operation::Agent(AgentOperation::ContextSnapshot) => "agent.context_snapshot",
```

- [ ] **Step 3: Add payload variants to `AgentPayload`**

After `StatusResult { ... }`:

```rust
    ContextConfig {
        agent_id: String,
    },
    ContextConfigResult {
        contributors: Vec<serde_json::Value>,
    },
    ContextSnapshot {
        agent_id: String,
        contributor_name: String,
    },
    ContextSnapshotResult {
        messages: Vec<serde_json::Value>,
    },
```

- [ ] **Step 4: Add decode arms in `Payload::from_operation()`**

After the `AgentOperation::Status` decode arm:

```rust
Operation::Agent(AgentOperation::ContextConfig) => {
    #[derive(Deserialize)]
    struct P { agent_id: String }
    let p: P = serde_json::from_value(value)
        .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.context_config"))?;
    Ok(Payload::Agent(AgentPayload::ContextConfig { agent_id: p.agent_id }))
}
Operation::Agent(AgentOperation::ContextSnapshot) => {
    #[derive(Deserialize)]
    struct P { agent_id: String, contributor_name: String }
    let p: P = serde_json::from_value(value)
        .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.context_snapshot"))?;
    Ok(Payload::Agent(AgentPayload::ContextSnapshot {
        agent_id: p.agent_id,
        contributor_name: p.contributor_name,
    }))
}
```

- [ ] **Step 5: Check compilation (expect wildcard match warnings on new variants — resolved in Task 4)**

```bash
cd crates/vol-llm-agent-channel && cargo check 2>&1 | tail -10
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-channel/src/agent_server_protocol.rs
git commit -m "feat(protocol): add agent.context_config and agent.context_snapshot operations"
```

---

### Task 4: Handle new operations in AgentHandler

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`

- [ ] **Step 1: Register operations in `AgentHandler::operations()`**

Add after the `Status` line:

```rust
Operation::Agent(AgentOperation::ContextConfig),
Operation::Agent(AgentOperation::ContextSnapshot),
```

- [ ] **Step 2: Add handler match arms**

After the `Status` handler block (before `(AgentOperation::Status, _) =>`), add:

```rust
(AgentOperation::ContextConfig, Payload::Agent(AgentPayload::ContextConfig { agent_id })) => {
    let agent = match self.router.get_agent(&agent_id).await {
        Some(a) => a,
        None => return Ok(vec![AgentServerMessage::new_error(
            message.message_id,
            Operation::Agent(AgentOperation::ContextConfig),
            crate::agent_server_protocol::ErrorPayload {
                code: "agent_not_found".to_string(),
                message: format!("agent '{}' not found", agent_id),
                detail: None,
                terminal: true,
            },
        )]),
    };
    let contributors = {
        let cb = &agent.config().context_builder;
        let infos = cb.contributor_infos().await.unwrap_or_default();
        infos.into_iter().map(|info| {
            serde_json::json!({
                "name": info.name,
                "anchor_zone": info.anchor_zone,
                "estimated_tokens": info.estimated_tokens,
                "message_count": info.message_count,
            })
        }).collect::<Vec<_>>()
    };

    Ok(vec![AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::ContextConfig),
        Payload::Agent(AgentPayload::ContextConfigResult { contributors }),
    )])
}
(AgentOperation::ContextSnapshot, Payload::Agent(AgentPayload::ContextSnapshot { agent_id, contributor_name })) => {
    let agent = match self.router.get_agent(&agent_id).await {
        Some(a) => a,
        None => return Ok(vec![AgentServerMessage::new_error(
            message.message_id,
            Operation::Agent(AgentOperation::ContextSnapshot),
            crate::agent_server_protocol::ErrorPayload {
                code: "agent_not_found".to_string(),
                message: format!("agent '{}' not found", agent_id),
                detail: None,
                terminal: true,
            },
        )]),
    };

    let messages = {
        let cb = &agent.config().context_builder;
        cb.snapshot_by_name(&contributor_name).await.unwrap_or_default()
            .into_iter()
            .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
            .collect::<Vec<_>>()
    };

    Ok(vec![AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::ContextSnapshot),
        Payload::Agent(AgentPayload::ContextSnapshotResult { messages }),
    )])
}
```

Also add missing match arms for the new payload variants on error paths. After `(AgentOperation::Status, _) =>`:

```rust
(AgentOperation::ContextConfig, _) => Err(ProtocolError::PayloadDecodeFailed("agent.context_config")),
(AgentOperation::ContextSnapshot, _) => Err(ProtocolError::PayloadDecodeFailed("agent.context_snapshot")),
```

- [ ] **Step 3: Run check**

```bash
cd crates/vol-llm-agent-channel && cargo check 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/agent.rs
git commit -m "feat(agent-handler): handle context_config and context_snapshot operations"
```

---

### Task 5: Frontend — state types and client methods

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add `Context` to `AgentSubTab`**

In `state/mod.rs` line 253:

```rust
pub enum AgentSubTab { Conversation, Sessions, Context }
```

- [ ] **Step 2: Add ContextState types + signal**

In `state/mod.rs`, after `AgentListEntry` (line 594):

```rust
/// A contributor info entry from agent.context_config RPC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContributorInfoEntry {
    pub name: String,
    pub anchor_zone: String,
    pub estimated_tokens: usize,
    pub message_count: usize,
}

/// A context message from agent.context_snapshot RPC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextMessageEntry {
    pub role: String,
    pub content: String,
}

/// Local state for ContextPanel.
#[cfg(all(feature = "web", not(feature = "tui")))]
#[derive(Debug)]
pub struct ContextState {
    pub contributors: Vec<ContributorInfoEntry>,
    pub loading: bool,
    pub error: Option<String>,
    pub dialog_open: bool,
    pub dialog_contributor_name: String,
    pub dialog_messages: Vec<ContextMessageEntry>,
    pub dialog_loading: bool,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl ContextState {
    pub fn new() -> Self {
        Self {
            contributors: Vec::new(),
            loading: false,
            error: None,
            dialog_open: false,
            dialog_contributor_name: String::new(),
            dialog_messages: Vec::new(),
            dialog_loading: false,
        }
    }
}
```

- [ ] **Step 3: Add client methods in `client.rs`**

In `client.rs`, after `agent_status` (around line 496), add:

```rust
/// Fetch contributor metadata for an agent.
pub fn agent_context_config(&self, agent_id: &str, cb: impl FnOnce(Result<Vec<crate::state::ContributorInfoEntry>, String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "agent.context_config",
        "params": { "agent_id": agent_id },
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
        match result.get("contributors").and_then(|v| v.as_array()) {
            Some(arr) => {
                let parsed: Vec<crate::state::ContributorInfoEntry> = arr.iter()
                    .filter_map(|e| serde_json::from_value(e.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no contributors in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}

/// Fetch full message snapshot for a named contributor.
pub fn agent_context_snapshot(&self, agent_id: &str, contributor_name: &str, cb: impl FnOnce(Result<Vec<crate::state::ContextMessageEntry>, String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "agent.context_snapshot",
        "params": { "agent_id": agent_id, "contributor_name": contributor_name },
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
        match result.get("messages").and_then(|v| v.as_array()) {
            Some(arr) => {
                let parsed: Vec<crate::state::ContextMessageEntry> = arr.iter()
                    .filter_map(|e| serde_json::from_value(e.clone()).ok())
                    .collect();
                cb(Ok(parsed));
            }
            None => cb(Err("no messages in response".to_string())),
        }
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 4: Check compilation**

```bash
cd crates/vol-llm-ui && cargo check --features web 2>&1 | tail -10
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/src/web/client.rs
git commit -m "feat(ui): add ContextState, context client methods"
```

---

### Task 6: Frontend — ContextPanel + ContextDialog component

**Files:**
- Create: `crates/vol-llm-ui/src/web/components/context_panel.rs`

- [ ] **Step 1: Write the component**

```rust
//! Context panel — contributor list with metadata, click to open snapshot dialog.

use dioxus::prelude::*;

use crate::state::{ContextMessageEntry, ContextState, ContributorInfoEntry};

/// Anchor zone color tag.
fn anchor_badge(zone: &str) -> &'static str {
    match zone {
        "head" => "#4080ff",
        "middle" => "#c0a040",
        "tail" => "#40c040",
        _ => "#888",
    }
}

/// Role color tag.
fn role_color(role: &str) -> &'static str {
    match role {
        "system" => "#c080ff",
        "user" => "#4080ff",
        "assistant" => "#40c040",
        "tool" => "#c0a040",
        _ => "#888",
    }
}

/// Modal dialog showing contributor message snapshot.
#[component]
fn ContextDialog(contributor_name: String, messages: Vec<ContextMessageEntry>, loading: bool, on_close: EventHandler<()>) -> Element {
    rsx! {
        div {
            class: "fixed inset-0 bg-black/50 flex items-center justify-center z-50",
            onclick: move |_| on_close.call(()),
            div {
                class: "w-[95vw] sm:w-[700px] max-h-[80vh] flex flex-col overflow-hidden bg-[#1a1a2e] border border-[#3a3a55] rounded-lg",
                onclick: move |evt: Event<MouseData>| { evt.stop_propagation(); },
                // Header
                div { class: "flex items-center justify-between flex-shrink-0 px-4 pt-3 pb-2 border-b border-[#3a3a55]",
                    span { class: "text-[15px] font-semibold text-[#e0e0e0] truncate", "{contributor_name}" }
                    button {
                        class: "text-[#888] hover:text-[#e0e0e0] text-[18px] flex-shrink-0 ml-2",
                        onclick: move |_| on_close.call(()),
                        "x"
                    }
                }
                // Content
                div { class: "flex-1 min-h-0 overflow-y-auto px-4 pb-4",
                    if loading {
                        div { class: "text-[#888] text-[13px] py-4 text-center", "Loading..." }
                    } else if messages.is_empty() {
                        div { class: "text-[#666] text-[13px] py-4 text-center", "No messages" }
                    } else {
                        for msg in &messages {
                            div { class: "mb-3",
                                div { class: "flex items-center gap-2 mb-1",
                                    span {
                                        class: "text-[10px] font-bold uppercase px-1.5 py-0.5 rounded",
                                        style: "color: {role_color(&msg.role)}; background: #2a2a44;",
                                        "{msg.role}"
                                    }
                                }
                                div { class: "bg-[#12121e] border border-[#2a2a44] rounded p-2 max-h-[300px] overflow-y-auto",
                                    pre { class: "text-[12px] text-[#ccc] font-mono whitespace-pre-wrap break-words",
                                        "{msg.content}"
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Context tab content.
#[component]
pub fn ContextPanel() -> Element {
    let app: crate::web::components::app::AppState = use_context();
    let agents_signal: Signal<crate::state::AgentsState> = use_context();

    let mut ctx_state = use_signal(ContextState::new);

    let selected = agents_signal.read().selected.clone();

    // Load contributors when selected agent changes
    use_effect(move || {
        if let Some(ref agent_id) = selected {
            let client = app.rpc_client.clone();
            let aid = agent_id.clone();
            let mut sig = ctx_state;
            sig.with_mut(|s| { s.loading = true; s.error = None; });
            wasm_bindgen_futures::spawn_local(async move {
                let (tx, rx) = futures_channel::oneshot::channel();
                client.agent_context_config(&aid, move |result| {
                    let _ = tx.send(result);
                });
                match rx.await {
                    Ok(Ok(contributors)) => {
                        sig.with_mut(|s| {
                            s.contributors = contributors;
                            s.loading = false;
                        });
                    }
                    Ok(Err(e)) => {
                        sig.with_mut(|s| {
                            s.error = Some(e);
                            s.loading = false;
                        });
                    }
                    Err(_) => {
                        sig.with_mut(|s| {
                            s.error = Some("request dropped".to_string());
                            s.loading = false;
                        });
                    }
                }
            });
        }
    });

    if selected.is_none() {
        return rsx! {
            div { class: "flex-1 flex items-center justify-center text-[#666] text-[14px]",
                "Select an agent to view context"
            }
        };
    }

    let loading = ctx_state.read().loading;
    let error = ctx_state.read().error.clone();
    let contributors = ctx_state.read().contributors.clone();

    let dialog_open = ctx_state.read().dialog_open;
    let dialog_name = ctx_state.read().dialog_contributor_name.clone();
    let dialog_messages = ctx_state.read().dialog_messages.clone();
    let dialog_loading = ctx_state.read().dialog_loading;

    rsx! {
        div { class: "flex-1 min-h-0 flex flex-col overflow-hidden",
            if loading {
                div { class: "flex items-center justify-center h-full text-[#666] text-[14px]",
                    "Loading contributors..."
                }
            } else if let Some(ref err) = error {
                div { class: "flex items-center justify-center h-full text-[#ff6060] text-[14px] text-center px-4",
                    "{err}"
                }
            } else if contributors.is_empty() {
                div { class: "flex items-center justify-center h-full text-[#888] text-[14px]",
                    "No contributors configured"
                }
            } else {
                div { class: "flex-1 overflow-y-auto",
                    for contributor in &contributors {
                        {
                            let name = contributor.name.clone();
                            let agent_id = selected.clone().unwrap_or_default();
                            let client = app.rpc_client.clone();
                            let mut sig = ctx_state;
                            rsx! {
                                div {
                                    key: "{contributor.name}",
                                    class: "flex items-center gap-3 px-3 py-2 border-b border-[#2a2a44] cursor-pointer hover:bg-[#2a2a44]",
                                    onclick: move |_| {
                                        let name = name.clone();
                                        let aid = agent_id.clone();
                                        let client = client.clone();
                                        let mut sig = sig;
                                        sig.with_mut(|s| {
                                            s.dialog_open = true;
                                            s.dialog_contributor_name = name.clone();
                                            s.dialog_messages = Vec::new();
                                            s.dialog_loading = true;
                                        });
                                        wasm_bindgen_futures::spawn_local(async move {
                                            let (tx, rx) = futures_channel::oneshot::channel();
                                            client.agent_context_snapshot(&aid, &name, move |result| {
                                                let _ = tx.send(result);
                                            });
                                            match rx.await {
                                                Ok(Ok(msgs)) => {
                                                    sig.with_mut(|s| {
                                                        s.dialog_messages = msgs;
                                                        s.dialog_loading = false;
                                                    });
                                                }
                                                Ok(Err(e)) => {
                                                    sig.with_mut(|s| {
                                                        s.dialog_loading = false;
                                                    });
                                                    log::warn!("snapshot failed for {}: {}", name, e);
                                                }
                                                Err(_) => {
                                                    sig.with_mut(|s| s.dialog_loading = false);
                                                }
                                            }
                                        });
                                    },
                                    // Anchor zone badge
                                    span {
                                        class: "text-[9px] font-bold px-1.5 py-0.5 rounded flex-shrink-0",
                                        style: "color: {anchor_badge(&contributor.anchor_zone)}; background: #2a2a44;",
                                        "{contributor.anchor_zone}"
                                    }
                                    // Name
                                    span { class: "font-semibold text-[13px] text-[#e0e0e0] flex-1 min-w-0 truncate",
                                        "{contributor.name}"
                                    }
                                    // Tokens
                                    span { class: "text-[11px] text-[#888] flex-shrink-0",
                                        "{contributor.estimated_tokens} tokens"
                                    }
                                    // Message count
                                    span { class: "text-[11px] text-[#666] flex-shrink-0",
                                        "{contributor.message_count} msg"
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Dialog
            if dialog_open {
                ContextDialog {
                    contributor_name: dialog_name,
                    messages: dialog_messages,
                    loading: dialog_loading,
                    on_close: move |_| {
                        ctx_state.with_mut(|s| {
                            s.dialog_open = false;
                            s.dialog_contributor_name = String::new();
                            s.dialog_messages = Vec::new();
                        });
                    },
                }
            }
        }
    }
}
```

- [ ] **Step 2: Re-export in `mod.rs`**

In `crates/vol-llm-ui/src/web/components/mod.rs`, add:

```rust
pub use context_panel::ContextPanel;
```

And ensure it's listed in the module declarations at the top (add if missing):
```rust
pub mod context_panel;
```

- [ ] **Step 3: Check compilation**

```bash
cd crates/vol-llm-ui && cargo check --features web 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/context_panel.rs crates/vol-llm-ui/src/web/components/mod.rs
git commit -m "feat(ui): add ContextPanel with ContextDialog modal"
```

---

### Task 7: Wire ContextPanel into AgentsPanel

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs`

- [ ] **Step 1: Add "Context" sub-tab button**

In `AgentsPanel::render()`, in the sub-tab bar div (around line 293), add after the "Sessions" button:

```rust
SubTabButton {
    label: "Context".to_string(),
    active: sub_tab == AgentSubTab::Context,
    onclick: {
        let mut sig = agents_signal;
        move |_: ()| { sig.with_mut(|s| s.sub_tab = AgentSubTab::Context); }
    },
}
```

- [ ] **Step 2: Add route for Context tab**

In the sub-tab content match (line 312), add after the `AgentSubTab::Sessions` arm:

```rust
AgentSubTab::Context => rsx! {
    ContextPanel {}
},
```

Add the import at the top:
```rust
use super::context_panel::ContextPanel;
```

- [ ] **Step 3: Check full web crate compilation**

```bash
cd crates/vol-llm-ui && cargo check --features web 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/agents_panel.rs
git commit -m "feat(ui): wire Context sub-tab into AgentsPanel"
```

---

### Task 8: Verify end-to-end

- [ ] **Step 1: Full workspace check**

```bash
cargo check 2>&1 | tail -10
```

Expected: no errors.

- [ ] **Step 2: Run all tests**

```bash
cargo test 2>&1 | tail -15
```

Expected: all tests pass.

- [ ] **Step 3: Commit any remaining changes**

```bash
git status
# Commit if needed
```
