# Agent Directory Discovery + Frontend Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace manual agent registration with directory-based `discover_agents()`, create 3 built-in agent definitions, fix `agent.list` metadata, and add frontend agent selector.

**Architecture:** Create `.agents/agents/*.md` files → update example to call `discover_agents()` → store agent defs in `AgentServerCore` → update `agent.list` to return full metadata → add `target` param to frontend `submit()` → add agent selector dropdown to `InputArea`.

**Tech Stack:** Rust, serde, YAML frontmatter (via AgentLoader), Dioxus 0.6 WASM

---

### Task 1: Create agent definition files

**Files:**
- Create: `.agents/agents/general-purpose.md`
- Create: `.agents/agents/explore.md`
- Create: `.agents/agents/review.md`

- [ ] **Step 1: Create general-purpose.md**

```markdown
---
name: general-purpose
type: general-purpose
description: General-purpose AI assistant for conversation and task help
max_iterations: 30
---

You are a helpful AI assistant. Answer questions clearly and concisely.
```

- [ ] **Step 2: Create explore.md**

```markdown
---
name: explore
type: explore
description: Code exploration specialist — search, grep, read, navigate codebases
tools: [read_file, glob, grep]
max_iterations: 30
---

You are a code exploration specialist. Your job is to understand and navigate
codebases. Use read_file, glob, and grep tools to search and read code.
Report findings clearly with file paths and line numbers.
```

- [ ] **Step 3: Create review.md**

```markdown
---
name: review
type: review
description: Code review specialist — analyze code quality, find issues, suggest improvements
tools: [read_file, glob, grep, bash]
max_iterations: 40
---

You are a code review specialist. Review code for bugs, security issues,
performance problems, and style violations. Use tools to read and understand
the code before commenting. Provide clear, actionable feedback with specific
file paths and line numbers.
```

- [ ] **Step 4: Commit**

```bash
git add .agents/agents/
git commit -m "feat: add general-purpose, explore, review agent definitions"
```

---

### Task 2: Update example to use discover_agents()

**Files:**
- Modify: `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs`

- [ ] **Step 1: Remove manual agent registration**

Remove lines 37-45 (the `let def = AgentDef::new(...)` + `core.register_agent(...)` block).

- [ ] **Step 2: Add discover_agents() call**

After `AgentServerCore::new(".", "~/.vol").await.expect(...)` (line 33), add:
```rust
core.discover_agents().await.expect("failed to discover agents");
```

- [ ] **Step 3: Update the startup log message**

Change line 57 `"Methods: agent.submit, agent.cancel, agent.approve"` to include `agent.list`:
```rust
tracing::info!("  Methods: agent.list, agent.submit, agent.cancel, agent.approve");
```

- [ ] **Step 4: Remove unused AgentDef import**

Check if `use vol_llm_agent::agent_def::AgentDef;` is now unused. Remove if so.

- [ ] **Step 5: Check compilation**

Run: `cargo check --example jsonrpc_agent_service -p vol-llm-agent-channel 2>&1 | tail -10`
Expected: clean

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs
git commit -m "feat: use discover_agents() instead of manual registration"
```

---

### Task 3: Store agent defs in AgentServerCore, update agent.list

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs`

- [ ] **Step 1: Add agent_defs field to AgentServerCore**

In `server_core.rs`, add to the struct (around line 88):
```rust
    agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_agent::AgentDef>>>,
```

- [ ] **Step 2: Add accessor method**

Add method:
```rust
    pub fn agent_defs(&self) -> &Arc<std::sync::RwLock<HashMap<String, vol_llm_agent::AgentDef>>> {
        &self.agent_defs
    }
```

- [ ] **Step 3: Populate agent_defs in build() and discover_agents()**

In `build()`, initialize the field (around line 345, where other fields are assigned):
```rust
    agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
```

In `discover_agents()`, after registering each agent, store the def:
```rust
    if let Some(def) = loader.get(&meta.name).await {
        self.agent_defs.write().unwrap().insert(meta.name.clone(), (*def).clone());
        let arc_def = ...;
        self.register_agent(&meta.name, arc_def).await?;
    }
```

- [ ] **Step 4: Update AgentHandler to accept agent_defs**

Change `AgentHandler` struct:
```rust
pub struct AgentHandler {
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_agent::AgentDef>>>,
}
```

Update `AgentHandler::new()` to accept the new field and update the caller in `server_core.rs`:
```rust
pub fn new(
    router: AgentRouter,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    agent_defs: Arc<std::sync::RwLock<HashMap<String, vol_llm_agent::AgentDef>>>,
) -> Self {
    Self { router, holders, agent_defs }
}
```

- [ ] **Step 5: Update agent.list handler**

In `domain/agent.rs`, replace the `AgentOperation::List` match arm (lines 154-167):
```rust
            (AgentOperation::List, _) => {
                let defs = self.agent_defs.read().unwrap();
                let agents: Vec<serde_json::Value> = self
                    .holders
                    .lock()
                    .unwrap()
                    .keys()
                    .map(|k| {
                        let def = defs.get(k);
                        serde_json::json!({
                            "id": k,
                            "name": k,
                            "type": def.map_or("unknown", |d| &d.r#type),
                            "description": def.and_then(|d| if d.description.is_empty() { None } else { Some(d.description.as_str()) }).unwrap_or(""),
                            "scope": def.map_or("unknown", |d| d.scope.to_string()),
                        })
                    })
                    .collect();
                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::List),
                    Payload::Agent(AgentPayload::ListResult { agents }),
                )])
            }
```

- [ ] **Step 6: Update AgentHandler construction in server_core.rs**

In `build()`, pass `agent_defs` to `AgentHandler::new()`:
```rust
    .register(Arc::new(AgentHandler::new(
        router.clone(),
        Arc::clone(&holders),
        agent_defs.clone(),
    )))
```

- [ ] **Step 7: Update for_test() in server_core.rs**

Update the test helper to pass an empty `agent_defs` HashMap.

- [ ] **Step 8: Check compilation and run tests**

Run: `cargo check -p vol-llm-agent-channel && cargo test -p vol-llm-agent-channel 2>&1 | grep "test result"`
Expected: clean, all pass

- [ ] **Step 9: Commit**

```bash
git add crates/vol-llm-agent-channel/src/server_core.rs crates/vol-llm-agent-channel/src/domain/agent.rs
git commit -m "feat: store agent defs, return metadata in agent.list"
```

---

### Task 4: Add target param to frontend submit()

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`

- [ ] **Step 1: Update submit() to accept optional target**

Change the submit method signature and params:

```rust
    pub fn submit(&self, input: &str, target: Option<&str>) -> Result<String, String> {
        let id = self.alloc_id();
        let mut params = serde_json::json!({ "input": input });
        if let Some(t) = target {
            params["target"] = serde_json::Value::String(t.to_string());
        }
        let msg = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "agent.submit",
            "params": params,
            "id": id,
        });
```

- [ ] **Step 2: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: errors in `input_area.rs` and `app.rs` (callers need update — next task)

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs
git commit -m "feat: add optional target param to client.submit()"
```

---

### Task 5: Add agent selector to frontend

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs`
- Modify: `crates/vol-llm-ui/src/web/components/input_area.rs`
- Modify: `crates/vol-llm-ui/src/web/components/app.rs`

- [ ] **Step 1: Add selected field to AgentsState (state/mod.rs)**

In `AgentsState` struct, add field:
```rust
    pub selected: Option<String>,
```

In `AgentsState::new()`:
```rust
    selected: None,
```

- [ ] **Step 2: Update input_area.rs — add agent selector + wire submit**

Read `AgentsState` from context. Add a `<select>` dropdown before the textarea. Pass `selected` agent as target:

```rust
// Read agents state
let agents_signal: Signal<AgentsState> = use_context();
let agents = agents_signal.read();
let selected_agent = agents.selected.clone();

// In the submit closure, pass target:
let target = agents_signal.read().selected.clone();
match client.submit(&text, target.as_deref()) {
    Ok(run_id) => log::info!("Submitted: {}", run_id),
    Err(e) => log::error!("Submit failed: {}", e),
}
```

Add the selector HTML in `rsx!`:
```rust
if !agents.agents.is_empty() {
    select {
        class: "bg-[#1a1a2e] text-[#e0e0e0] border border-[#444466] rounded px-2 py-1 text-[14px] mb-2 outline-none",
        onchange: move |evt: Event<FormData>| {
            agents_signal.write_unchecked().selected = Some(evt.value());
        },
        for agent in &agents.agents {
            option {
                value: "{agent.id}",
                selected: selected_agent.as_ref() == Some(&agent.id),
                "{agent.name}"
            }
        }
    }
}
```

- [ ] **Step 3: Check WASM compilation**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -5`
Expected: clean

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs crates/vol-llm-ui/src/web/components/input_area.rs
git commit -m "feat: add agent selector dropdown to input area"
```

---

### Task 6: Verify full flow and wiki ingest

**Files:** verify all

- [ ] **Step 1: Full channel tests**

Run: `cargo test -p vol-llm-agent-channel 2>&1 | grep "test result"`
Expected: all pass

- [ ] **Step 2: WASM check**

Run: `cargo check -p vol-llm-ui --target wasm32-unknown-unknown --no-default-features --features web 2>&1 | tail -3`
Expected: clean

- [ ] **Step 3: Wiki ingest**

Update wiki documenting agent discovery feature.

- [ ] **Step 4: Commit**

```bash
git add docs/wiki/
git commit -m "docs: update wiki for agent directory discovery"
```
