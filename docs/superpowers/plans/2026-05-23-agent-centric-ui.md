# Agent-Centric UI + Protocol Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Restructure UI around agents — move Agents tab to first position, embed Conversation + Sessions as sub-tabs inside Agents panel scoped to selected agent. Add protocol support for agent_id-filtered sessions and agent status.

**Architecture:** Backend: add `agent_id` to `SessionPayload::List`, add `agent_status` tracking to `AgentServerCore`, enrich `agent.list` with status/current_input. Frontend: reorder tab bar, rewrite `AgentsPanel` as card grid with embedded sub-tabs (Conversation/Sessions) and input area, remove old Conversation/Sessions tabs.

**Tech Stack:** Rust, Dioxus 0.6 WASM, serde

---

### Task 1: Add agent_id to SessionPayload::List

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/agent_server_protocol.rs`

- [ ] **Step 1: Update SessionPayload::List variant**

Change:
```rust
    List,
```
To:
```rust
    List {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        agent_id: Option<String>,
    },
```

- [ ] **Step 2: Update from_operation decode for session.list**

Add `agent_id` decoding. In `Payload::from_operation()` for `SessionOperation::List`:
```rust
            Operation::Session(SessionOperation::List) => {
                #[derive(Deserialize)]
                struct P {
                    #[serde(default)]
                    agent_id: Option<String>,
                }
                let p: P = serde_json::from_value(value)
                    .map_err(|_| ProtocolError::PayloadDecodeFailed("session.list"))?;
                Ok(Payload::Session(SessionPayload::List { agent_id: p.agent_id }))
            }
```

- [ ] **Step 3: Check compilation**

Run: `cargo check -p vol-llm-agent-channel 2>&1 | tail -5`
Expected: errors in SessionHandler (match pattern changed — Task 2 fixes)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/agent_server_protocol.rs
git commit -m "feat: add agent_id filter to session.list protocol"
```

---

### Task 2: Update SessionHandler to filter by agent_id

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/session.rs`

- [ ] **Step 1: Update SessionOperation::List match arm**

Change the match pattern and body to filter by `agent_id`:
```rust
            (SessionOperation::List, Payload::Session(SessionPayload::List { agent_id })) => {
                let mut all_sessions: Vec<serde_json::Value> = Vec::new();

                // Determine which agent dirs to scan
                let agent_ids: Vec<String> = if let Some(ref aid) = agent_id {
                    vec![aid.clone()]
                } else if self.agents_root.is_dir() {
                    std::fs::read_dir(&self.agents_root)
                        .into_iter().flatten().flatten()
                        .filter(|e| e.path().is_dir())
                        .filter_map(|e| e.file_name().to_str().map(String::from))
                        .collect()
                } else {
                    vec![]
                };

                for aid in &agent_ids {
                    let store = self.agent_store(aid);
                    if let Ok(summaries) = store.list_sessions() {
                        for s in summaries {
                            all_sessions.push(serde_json::json!({
                                "id": s.session_id,
                                "agent_id": aid,
                                "session_id": s.session_id,
                                "entry_count": s.entry_count,
                                "created_at": s.created_at,
                            }));
                        }
                    }
                }

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Session(SessionOperation::List),
                    Payload::Session(SessionPayload::ListResult { sessions: all_sessions }),
                )])
            }
```

And update the catch-all for List at the bottom:
```rust
            (SessionOperation::List, _) => Err(ProtocolError::PayloadDecodeFailed("session.list")),
```

- [ ] **Step 2: Check compilation and run tests**

Run: `cargo check -p vol-llm-agent-channel && cargo test -p vol-llm-agent-channel 2>&1 | grep "test result"`
Expected: clean, all pass

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/session.rs
git commit -m "feat: filter sessions by agent_id in session.list"
```

---

### Task 3: Add agent status tracking

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`
- Modify: `crates/vol-llm-agent-channel/src/connection.rs`

- [ ] **Step 1: Add AgentStatus type and agent_status field to AgentServerCore**

In `server_core.rs`, add:
```rust
#[derive(Debug, Clone, Default)]
pub struct AgentStatus {
    pub status: String,  // "idle" | "running"
    pub current_input: Option<String>,
    pub run_id: Option<String>,
}

impl AgentStatus {
    pub fn idle() -> Self {
        Self { status: "idle".into(), current_input: None, run_id: None }
    }
    pub fn running(input: String, run_id: String) -> Self {
        Self { status: "running".into(), current_input: Some(input), run_id: Some(run_id) }
    }
}
```

Add field to `AgentServerCore` struct:
```rust
    agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>>,
```

Add accessor:
```rust
    pub fn agent_status(&self) -> &Arc<std::sync::RwLock<HashMap<String, AgentStatus>>> {
        &self.agent_status
    }
```

Initialize in `build()`:
```rust
    agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
```

Update `for_test()` similarly.

Update `discover_agents()` to init status for each discovered agent:
```rust
    self.agent_status.write().unwrap().insert(meta.name.clone(), AgentStatus::idle());
```

- [ ] **Step 2: Update ConnectionHolder to update agent status on events**

In `connection.rs`, update `ConnectionHolder::listen()` to catch AgentStart/AgentComplete and update status. Read `agent_status` from somewhere accessible — pass as field on `ConnectionHolder`:

```rust
pub struct ConnectionHolder {
    connection: Arc<RwLock<Option<Arc<dyn Connection>>>>,
    sender: String,
    receiver: String,
    agent_status: Option<Arc<std::sync::RwLock<HashMap<String, crate::server_core::AgentStatus>>>>,
}
```

Update `new()` accordingly. In `listen()`, check for AgentStart/AgentComplete:
```rust
    async fn listen(&self, event: &AgentStreamEvent, ctx: &RunContext) {
        // Update agent status
        if let Some(ref status_map) = self.agent_status {
            match event {
                AgentStreamEvent::AgentStart { input, .. } => {
                    status_map.write().unwrap().insert(
                        self.sender.clone(),
                        crate::server_core::AgentStatus::running(input.clone(), ctx.run_id.clone()),
                    );
                }
                AgentStreamEvent::AgentComplete { .. } | AgentStreamEvent::AgentAborted { .. } => {
                    status_map.write().unwrap().insert(
                        self.sender.clone(),
                        crate::server_core::AgentStatus::idle(),
                    );
                }
                _ => {}
            }
        }

        // ... existing event forwarding code ...
    }
```

- [ ] **Step 3: Update ConnectionHolder construction in register_agent()**

In `server_core.rs` `register_agent()`, pass `agent_status` to `ConnectionHolder::new()`:
```rust
    let holder = ConnectionHolder::new(agent_id.clone(), "client".to_string(), Some(self.agent_status.clone()));
```

Update `ConnectionHolder::new()` to accept the new param.

- [ ] **Step 4: Update agent.list to return status**

In `domain/agent.rs`, add status lookup to the List handler:
```rust
    let status_map = ... // need access to agent_status

    // In the map closure:
    let status = status_map.get(k).map_or("idle", |s| s.status.as_str());
    let current_input = status_map.get(k).and_then(|s| s.current_input.clone());
```

AgentHandler needs access to `agent_status`. Add field and pass in constructor from server_core.

- [ ] **Step 5: Check compilation and run tests**

Run: `cargo check -p vol-llm-agent-channel && cargo test -p vol-llm-agent-channel 2>&1 | grep "test result"`
Expected: clean, all pass

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-channel/src/
git commit -m "feat: add agent status tracking and expose in agent.list"
```

---

### Task 4: Frontend — reorder tabs, remove Conversation/Sessions

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Update ActiveTab enum in state/mod.rs**

Change:
```rust
pub enum ActiveTab { Conversation, Sessions, Tools, Workspace, Skills, Mcp, Logs, Agents }
```
To:
```rust
pub enum ActiveTab { Agents, Tools, Workspace, Skills, Mcp, Logs }
```

Update `next()` method:
```rust
    ActiveTab::Agents => ActiveTab::Tools,
    ActiveTab::Tools => ActiveTab::Workspace,
    ActiveTab::Workspace => ActiveTab::Skills,
    ActiveTab::Skills => ActiveTab::Mcp,
    ActiveTab::Mcp => ActiveTab::Logs,
    ActiveTab::Logs => ActiveTab::Agents,
```

Update default values in `GlobalState::new()` and `UiState::new()` to use `ActiveTab::Agents`.

- [ ] **Step 2: Update tab bar in app.rs**

Change the TabBar to:
```rust
TabButton { state: state.clone(), tab: ActiveTab::Agents, label: "Agents" }
TabButton { state: state.clone(), tab: ActiveTab::Tools, label: "Tools" }
TabButton { state: state.clone(), tab: ActiveTab::Workspace, label: "Workspace" }
TabButton { state: state.clone(), tab: ActiveTab::Skills, label: "Skills" }
TabButton { state: state.clone(), tab: ActiveTab::Mcp, label: "MCP" }
TabButton { state: state.clone(), tab: ActiveTab::Logs, label: "Logs" }
```

- [ ] **Step 3: Update TabContent in app.rs**

Remove `ActiveTab::Conversation` and `ActiveTab::Sessions` match arms:
```rust
match active {
    ActiveTab::Agents => rsx! { AgentsPanel {} },
    ActiveTab::Tools => rsx! { ToolsTabContent {} },
    ActiveTab::Workspace => rsx! { FileContentView {} },
    ActiveTab::Skills => rsx! { SkillsPanel { dialog_signal: skill_dialog_signal } },
    ActiveTab::Logs => rsx! { LogViewer {} },
    ActiveTab::Mcp => rsx! { McpPanel {} },
}
```

- [ ] **Step 4: Remove Conversation/Sessions from UiEventKind**

In `state/mod.rs`, remove `Conversation` and `Sessions` UiEventKind variants and their usages in `kind()` and event list builders.

- [ ] **Step 5: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: errors in components referencing old ActiveTab variants (Task 5 fixes)

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/src/web/components/app.rs
git commit -m "refactor: reorder tabs, remove Conversation/Sessions from tab bar"
```

---

### Task 5: Rewrite AgentsPanel with cards, sub-tabs, embedded content

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/input_area.rs`

- [ ] **Step 1: Add agent sub-tab state and AgentCard type**

In `state/mod.rs`, add:
```rust
#[derive(Debug, Clone, PartialEq)]
pub enum AgentSubTab { Conversation, Sessions }

pub struct AgentCardInfo {
    pub id: String,
    pub name: String,
    pub type_: String,
    pub description: String,
    pub scope: String,
    pub status: String,
    pub current_input: Option<String>,
}
```

In `AgentsState`, add:
```rust
    pub selected: Option<String>,
    pub sub_tab: AgentSubTab,
    pub agent_cards: Vec<AgentCardInfo>,
```

- [ ] **Step 2: Rewrite agents_panel.rs**

Replace existing content with agent-centric panel:

```rust
// Top: Agent card grid
// Each card: status dot, name, type badge, current task preview
// Click to select

// Selected agent info bar: name, type, description, status

// Sub-tab bar: [Conversation] [Sessions]

// Sub-tab content:
//   Conversation → embedded ConversationView (scoped to selected agent)
//   Sessions → embedded SessionsPanel (scoped to selected agent)

// InputArea at bottom (only in Conversation sub-tab)
```

The `agent.list` RPC call populates `agent_cards`. Status updates come from event stream agent start/complete events which update the card data.

- [ ] **Step 3: Update SessionsPanel to accept agent_id**

Add `agent_id: Option<String>` as a component prop or read from `AgentsState.selected`. Pass to `session.list` RPC.

- [ ] **Step 4: Update InputArea to work inside agents panel**

Accept `selected_agent: Signal<Option<String>>` from context/AgentsState. Pass as target to `client.submit()`.

- [ ] **Step 5: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: clean

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-ui/src/
git commit -m "feat: rewrite agents panel with cards, sub-tabs, embedded conversation and sessions"
```

---

### Task 6: Verify and wiki ingest

- [ ] **Step 1: Full channel tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1 | grep "test result"`
Expected: all pass

- [ ] **Step 2: WASM check**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -3`
Expected: clean

- [ ] **Step 3: Wiki ingest**

Update wiki with agent-centric UI changes.
