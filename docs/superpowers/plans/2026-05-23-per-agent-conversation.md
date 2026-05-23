# Per-Agent Conversation State Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace global `ConversationState` with per-agent map so each agent has independent conversation state. Switching agents restores that agent's conversation. Events update the correct agent.

**Architecture:** `ConversationState.entries` becomes `HashMap<String, AgentConversation>` keyed by agent_id. `reduce_conversation` and `ConversationView` route by `active_agent`. Resume stores under the resumed agent's key. Agent switch activates a different key.

**Tech Stack:** Rust, Dioxus 0.6 WASM

---

### Task 1: Rewrite ConversationState as per-agent map

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`

- [ ] **Step 1: Replace ConversationState and AgentConversation**

Replace the current `ConversationState` struct and impl:

```rust
/// Per-agent conversation entries.
#[derive(Debug, Clone)]
pub struct AgentConversation {
    pub entries: Vec<ConversationEntry>,
    pub auto_scroll: bool,
}

impl AgentConversation {
    pub fn new() -> Self {
        Self { entries: Vec::new(), auto_scroll: true }
    }
}

/// Conversation state keyed by agent_id.
#[derive(Debug, Clone)]
pub struct ConversationState {
    pub agents: HashMap<String, AgentConversation>,
    pub active_agent: Option<String>,
}

#[cfg(all(feature = "web", not(feature = "tui")))]
impl ConversationState {
    pub fn new() -> Self {
        Self { agents: HashMap::new(), active_agent: None }
    }

    /// Get or create conversation for the given agent_id.
    pub fn get_or_create(&mut self, agent_id: &str) -> &mut AgentConversation {
        self.agents.entry(agent_id.to_string()).or_insert_with(AgentConversation::new)
    }

    /// Get the active agent's conversation (panics if no active agent).
    pub fn active_mut(&mut self) -> &mut AgentConversation {
        let id = self.active_agent.clone().unwrap_or_default();
        self.get_or_create(&id)
    }

    /// Get entries for the active agent.
    pub fn active_entries(&self) -> &[ConversationEntry] {
        self.active_agent.as_ref()
            .and_then(|id| self.agents.get(id))
            .map(|a| a.entries.as_slice())
            .unwrap_or(&[])
    }

    /// Set the active agent. Returns true if agent changed.
    pub fn set_active(&mut self, agent_id: Option<String>) -> bool {
        if self.active_agent != agent_id {
            self.active_agent = agent_id;
            true
        } else {
            false
        }
    }
}
```

Add `use std::collections::HashMap;` at the top of the file if not already present.

- [ ] **Step 2: Check compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: errors in other files referencing old `ConversationState` fields (fixed in subsequent tasks)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "refactor: replace ConversationState with per-agent HashMap"
```

---

### Task 2: Update reduce_conversation and ConversationView

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/conversation.rs`

- [ ] **Step 1: Update reduce_conversation to use active agent**

The `reduce_conversation` function currently takes `&mut ConversationState` and mutates `s.entries`. Update all entry accesses to use `s.active_mut()`:

```rust
pub fn reduce_conversation(s: &mut ConversationState, event: &UiEvent) {
    let conv = s.active_mut();
    match event {
        UiEvent::AgentStart { input } => {
            conv.entries.clear();
            conv.entries.push(ConversationEntry::UserInput { text: input.clone() });
        }
        UiEvent::AgentComplete { .. } => {
            // no-op in per-agent model
        }
        // ... all other arms: replace `s.entries.push(...)` with `conv.entries.push(...)`
        // ... and `s.auto_scroll` with `conv.auto_scroll`
    }
}
```

Read the full current function and update every `s.entries` → `conv.entries`, `s.auto_scroll` → `conv.auto_scroll`, etc.

- [ ] **Step 2: Update ConversationView to read from active agent**

In `ConversationView`, change:
```rust
let count = signal.read().entries.len();
```
To:
```rust
let entries_len = signal.read().active_entries().len();
```

And:
```rust
let entries = signal.read().entries.clone();
```
To:
```rust
let entries = signal.read().active_entries().to_vec();
```

- [ ] **Step 3: Check compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: errors in app.rs, agents_panel.rs, sessions_panel.rs (fixed next)

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/conversation.rs
git commit -m "refactor: route conversation events and view through active agent"
```

---

### Task 3: Update app.rs event routing and agent switch

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`
- Modify: `crates/vol-llm-ui/src/web/components/agents_panel.rs`

- [ ] **Step 1: Update app.rs — set active_agent on submit**

In the event loop or input handling, when the user submits with a target, set `active_agent`:
```rust
// In the submit handling, before sending:
conv_signal.with_mut(|s| { s.set_active(target.map(|t| t.to_string())); });
```

- [ ] **Step 2: Update app.rs — event subscriptions pass active_agent**

The event subscriptions at line 254 already call `reduce_conversation` which now uses `active_mut()`. No change needed if `active_agent` is set correctly before events arrive.

- [ ] **Step 3: Update agents_panel.rs — agent switch sets active_agent**

In the card click handler for agent selection, add `active_agent` set:
```rust
sig.with_mut(|s| {
    if is_selected { s.selected = None; }
    else {
        s.selected = Some(agent_id.clone());
        s.sub_tab = AgentSubTab::Conversation;
    }
});
// Also update ConversationState.active_agent
conv_signal.with_mut(|cs| { cs.set_active(Some(agent_id)); });
```

AgentsPanel needs access to `ConversationState` signal. Add `use_context::<Signal<ConversationState>>()`.

- [ ] **Step 4: Check compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: errors in sessions_panel.rs (fixed next)

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/app.rs crates/vol-llm-ui/src/web/components/agents_panel.rs
git commit -m "feat: set active_agent on submit and agent switch"
```

---

### Task 4: Update sessions panel resume to store per-agent

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

- [ ] **Step 1: Update resume callback to use agent key**

In the resume onclick handler, change:
```rust
conv.with_mut(|s| { s.entries = conv_entries; });
```
To:
```rust
let agent_id = agents.read().selected.clone().unwrap_or_default();
conv.with_mut(|s| {
    let ac = s.get_or_create(&agent_id);
    ac.entries = conv_entries;
});
```

- [ ] **Step 2: Check compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: clean

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/sessions_panel.rs
git commit -m "fix: store resumed entries under correct agent key"
```

---

### Task 5: Verify and wiki

- [ ] **Step 1: WASM check**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -3`
Expected: clean

- [ ] **Step 2: Run tests**

Run: `cargo test -p vol-llm-ui --no-default-features --features web 2>&1 | tail -5`
Expected: all pass

- [ ] **Step 3: Wiki ingest**

Update wiki for per-agent conversation state.
