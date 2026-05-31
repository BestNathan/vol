# Agent Status on Reconnect — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On agent select (especially after reconnect), query backend for agent running state; if running, load session + show banner + subscribe.

**Architecture:** New `agent.status` RPC on backend reads the existing `AgentStatus` map. Frontend calls it when agent is selected; if running, loads session entries into conversation, pushes a `RunningBanner` entry, enables live subscription.

**Tech Stack:** Rust, Dioxus 0.6, axum, JSON-RPC over WebSocket

---

### Task 1: Backend — Add Status operation to protocol

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`

- [ ] **Step 1: Add `Status` variant to `AgentOperation`**

At line 86, add `Status`:

```rust
pub enum AgentOperation {
    Submit,
    Cancel,
    Subscribe,
    Unsubscribe,
    Approve,
    List,
    Event,
    Status,
}
```

- [ ] **Step 2: Add `"agent.status"` to the operation → method string mapping**

At line 56, after the `Event` line:

```rust
Operation::Agent(AgentOperation::Status) => "agent.status",
```

- [ ] **Step 3: Add `Status` and `StatusResult` variants to `AgentPayload`**

After the existing `ListResult` variant (around line 510), add:

```rust
Status {
    agent_id: String,
},
StatusResult {
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    run_id: Option<String>,
},
```

- [ ] **Step 4: Add decode logic for `AgentOperation::Status`**

In `Payload::from_operation`, after the `AgentOperation::List` branch (around line 231):

```rust
Operation::Agent(AgentOperation::Status) => {
    #[derive(Deserialize)]
    struct P {
        agent_id: String,
    }
    let p: P = serde_json::from_value(value)
        .map_err(|_| ProtocolError::PayloadDecodeFailed("agent.status"))?;
    Ok(Payload::Agent(AgentPayload::Status { agent_id: p.agent_id }))
}
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent-channel/src/agent_server_protocol.rs
git commit -m "feat(protocol): add agent.status operation and payload"
```

---

### Task 2: Backend — Add operation codec mapping

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/operation_codec.rs`

- [ ] **Step 1: Add `"agent.status"` mapping**

At line 15, after the `agent.event` line:

```rust
"agent.status" => Ok(Operation::Agent(AgentOperation::Status)),
```

- [ ] **Step 2: Commit**

```bash
git add crates/vol-llm-agent-channel/src/operation_codec.rs
git commit -m "feat(codec): add agent.status operation codec"
```

---

### Task 3: Backend — Implement agent.status handler

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`

- [ ] **Step 1: Add handler branch for `AgentOperation::Status`**

After the `AgentOperation::List` handler (around line 189), add:

```rust
(AgentOperation::Status, Payload::Agent(AgentPayload::Status { agent_id })) => {
    let status_map = self.agent_status.read().unwrap();
    let info = status_map.get(&agent_id);
    let status = info.map_or("idle", |s| s.status.as_str()).to_string();
    let run_id = info.and_then(|s| s.run_id.clone());
    Ok(vec![AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::Status),
        Payload::Agent(AgentPayload::StatusResult {
            status,
            run_id,
        }),
    )])
}
```

- [ ] **Step 2: Register `Status` in operation list**

At line 40, add to `fn operations()`:

```rust
Operation::Agent(AgentOperation::Status),
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-agent-channel
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/agent.rs
git commit -m "feat(agent): implement agent.status handler"
```

---

### Task 4: Frontend — Add `agent_status` RPC to client

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Add `agent_status` method**

After the `agent_list` method (around line 410), add:

```rust
/// Query agent running status.
pub fn agent_status(&self, agent_id: &str, cb: impl FnOnce(Result<(String, Option<String>), String>) + 'static) {
    let id = self.alloc_id();
    let msg = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "agent.status",
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
        let status = result.get("status").and_then(|v| v.as_str()).unwrap_or("idle").to_string();
        let run_id = result.get("run_id").and_then(|v| v.as_str()).map(|s| s.to_string());
        cb(Ok((status, run_id)));
    });
    self.inner.pending.borrow_mut().insert(id, cb);
}
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat(ui): add agent_status RPC method to client"
```

---

### Task 5: Frontend — Add RunningBanner entry type

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Add variant to `ConversationEntry`**

At line 137 (after `Error`), add:

```rust
RunningBanner { run_id: String },
```

- [ ] **Step 2: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

Expected: compilation errors in conversation.rs and sessions_panel.rs for non-exhaustive match — will fix in next tasks.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat(ui): add RunningBanner conversation entry variant"
```

---

### Task 6: Frontend — Render RunningBanner and handle lifecycle

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`

- [ ] **Step 1: Add render for `RunningBanner` in `TimelineEntry`**

In the `match entry` block, before `ConversationEntry::Error`:

```rust
ConversationEntry::RunningBanner { ref run_id } => {
    rsx! {
        div { class: "flex items-center gap-3 px-3 py-3 mb-2 bg-[#1a2a44] border border-[#3a5a7a] rounded-md text-sm",
            div { class: "w-2.5 h-2.5 rounded-full bg-[#40c040] animate-pulse shrink-0" }
            div { class: "flex flex-col gap-0.5",
                span { class: "text-[#c0d0e0] font-semibold", "Agent is currently running" }
                span { class: "text-[#888] text-xs font-mono", "run_id: {run_id}" }
                span { class: "text-[#666] text-xs", "Below is the live conversation." }
            }
        }
    }
}
```

- [ ] **Step 2: Clear RunningBanner on AgentComplete/Aborted/Error in `reduce_conversation`**

Add a helper function:

```rust
fn clear_running_banner(entries: &mut Vec<ConversationEntry>) {
    entries.retain(|e| !matches!(e, ConversationEntry::RunningBanner { .. }));
}
```

In the handler for `AgentComplete`:

```rust
UiEvent::AgentComplete { .. } => {
    flush_pending_content(&mut conv.entries);
    clear_running_banner(&mut conv.entries);
}
```

Same for `AgentAborted` and `AgentError`:

```rust
UiEvent::AgentAborted { reason } | UiEvent::AgentError { message: reason } => {
    flush_pending_content(&mut conv.entries);
    clear_running_banner(&mut conv.entries);
    conv.entries.push(ConversationEntry::Error { message: reason.clone() });
}
```

- [ ] **Step 3: Update `IterationComplete` to also clear banner**

```rust
UiEvent::IterationComplete { .. } => {
    clear_running_banner(&mut conv.entries);
}
```

- [ ] **Step 4: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "feat(ui): render RunningBanner and clear on agent lifecycle events"
```

---

### Task 7: Frontend — Wire up agent.status in app logic

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs`

- [ ] **Step 1: Remove `is_running = false` from WsConnected handler**

In app.rs line 158, delete:

```rust
g.is_running = false;
```

- [ ] **Step 2: Add `agent.status` call on agent select in AgentsPanel**

In the agent card `on_click` handler (agents_panel.rs around line 200), after setting selected and active agent, add a call to `agent_status`. Extend the click handler:

```rust
move |_: ()| {
    let is_deselect = is_selected;
    sig.with_mut(|s| {
        if is_deselect { s.selected = None; }
        else {
            s.selected = Some(agent_id.clone());
            s.sub_tab = AgentSubTab::Conversation;
        }
    });
    conv_sig.with_mut(|cs| {
        cs.set_active(if is_deselect { None } else { Some(agent_id.clone()) });
    });

    if !is_deselect {
        let client = app.rpc_client.clone();
        let agent = agent_id.clone();
        let conv = conv_sig;
        let global = app.global_signal.clone();
        wasm_bindgen_futures::spawn_local(async move {
            // Check agent status via RPC
            let (tx, rx) = futures_channel::oneshot::channel();
            client.agent_status(&agent, move |result| {
                let _ = tx.send(result);
            });
            let status_result = rx.await;
            match status_result {
                Ok(Ok((status, run_id))) => {
                    if status == "running" {
                        if let Some(ref rid) = run_id {
                            log::info!("Agent {} is running (run_id: {}) — loading session", agent, rid);
                            // Load session entries
                            load_running_session(&client, &agent, &conv, rid);
                            // Set running state
                            global.write_unchecked().is_running = true;
                        }
                    }
                }
                _ => log::warn!("Failed to query agent status for {}", agent),
            }
        });
    }
}
```

- [ ] **Step 3: Add `load_running_session` helper function**

Above the `App` component, add:

```rust
fn load_running_session(
    client: &crate::web::client::JsonRpcClient,
    agent_id: &str,
    conv: Signal<ConversationState>,
    run_id: &str,
) {
    let c = client.clone();
    let agent = agent_id.to_string();

    wasm_bindgen_futures::spawn_local(async move {
        // List sessions for this agent
        let (tx, rx) = futures_channel::oneshot::channel();
        c.session_list(Some(&agent), move |result| {
            let _ = tx.send(result);
        });
        let sessions = match rx.await {
            Ok(Ok(s)) => s,
            _ => { log::warn!("Failed to list sessions for {}", agent); return; }
        };
        if sessions.is_empty() { return; }
        let latest_id = sessions[0].id.clone();

        // Fetch entries
        let (tx2, rx2) = futures_channel::oneshot::channel();
        c.session_entries(&latest_id, move |result| {
            let _ = tx2.send(result);
        });
        let entries = match rx2.await {
            Ok(Ok(e)) => e,
            _ => { log::warn!("Failed to fetch session entries for {}", agent); return; }
        };

        // Build conversation
        let conv_entries = crate::web::components::sessions_panel::session_entries_to_conversation(entries);
        let mut banner_entries = vec![ConversationEntry::RunningBanner {
            run_id: run_id.to_string(),
        }];
        banner_entries.extend(conv_entries);

        let mut cs = conv.write_unchecked();
        let ac = cs.get_or_create(&agent);
        ac.entries = banner_entries;
    });
}
```

- [ ] **Step 4: Pass `AppState` into `AgentsPanel` props if needed**

Check `AgentsPanel` signature — it already gets `app: AppState` via `use_context()`. Add a `global_signal` context if not already available in the component. If the AgentsPanel doesn't have access to global, add `let global: Signal<GlobalState> = use_context();` inside `fn AgentsPanel`.

Also add `use std::collections::HashMap;` import and `use web_time::Instant;` if not already there.

- [ ] **Step 5: Verify compilation**

```bash
cargo check -p vol-llm-ui --no-default-features --features web
```

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs crates/vol-llm-ui/src/web/components/agents_panel.rs
git commit -m "feat(ui): call agent.status on select, load session when running"
```

---

### Task 8: Frontend — Update TUI and sessions_panel match arms

**Files:**
- Modify: `crates/vol-llm-ui/src/tui/render.rs`
- Modify: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

- [ ] **Step 1: Add `RunningBanner` match arm in TUI render**

In `tui/render.rs`, in the `match entry` block, add before `Error`:

```rust
ConversationEntry::RunningBanner { run_id } => {
    lines.push(Line::from(vec![
        Span::styled(
            format!("⬤ Agent running  [{}]", run_id),
            Style::default().fg(Color::LightBlue).add_modifier(Modifier::BOLD),
        ),
    ]));
}
```

- [ ] **Step 2: Add `RunningBanner` match arm in sessions_panel render**

In `sessions_panel.rs`, in the `match entry` block (around line 168, in the `render_entry_preview` function or equivalent), add:

```rust
ConversationEntry::RunningBanner { run_id } => rsx! {
    div { class: "mb-2 px-3 py-2 rounded-md bg-[#1a2a44] border border-[#3a5a7a] text-sm",
        span { class: "text-[#c0d0e0]", "⬤ Running  [{run_id}]" }
    }
},
```

- [ ] **Step 3: Verify compilation**

```bash
cargo check -p vol-llm-ui
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/tui/render.rs crates/vol-llm-ui/src/web/components/sessions_panel.rs
git commit -m "fix(ui): add RunningBanner match arms in TUI and sessions panel"
```

---

### Task 9: Full integration verification

- [ ] **Step 1: Check all crates compile**

```bash
cargo check -p vol-llm-agent-channel -p vol-llm-ui --no-default-features --features web
```

Expected: clean compilation.

- [ ] **Step 2: Run tests**

```bash
cargo test -p vol-llm-agent-channel -p vol-llm-ui --lib
```

- [ ] **Step 3: Manual test plan**

1. Start backend + frontend (`make web-backend` + `make web-dev`)
2. Open browser, select an agent → should show normal idle state
3. Submit a task that takes time → conversation should show running banner at top
4. Kill and restart backend → frontend should reconnect
5. Select the same agent → should detect running state, load session, show banner
6. Wait for agent to complete → banner should disappear
7. Select agent that is idle → conversation should not change

- [ ] **Step 4: Commit any final fixes**

```bash
git add -A
git commit -m "chore: final integration fixes for agent.status"
```
