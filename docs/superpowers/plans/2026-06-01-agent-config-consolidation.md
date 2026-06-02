# AgentConfig Consolidation & Contributor SOT Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete `AgentConfig::new()`, unify construction through builder, make `ReActAgent` the SOT for contributor access, and add stable agent list sorting.

**Architecture:** `AgentConfigBuilder::build()` becomes the single construction path that auto-injects system prompt (from `def.prompt`) and SkillInjector in the correct order. `ReActAgent` exposes `contributors()` / `snapshot_by_name()` / `add_contributor()` for external use. `context_builder` field becomes `pub(crate)`.

**Tech Stack:** Rust, vol-llm-agent, vol-llm-context, vol-llm-agent-channel, vol-llm-runtime

---

### Task 1: AgentConfigBuilder — auto-inject system prompt before SkillInjector

**Files:**
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs:163-242`

- [ ] **Step 1: Modify `build()` to inject system prompt first**

Replace lines 199-223 (the context_builder construction block):

```rust
// Build context builder — system prompt first (if agent def has one),
// then SkillInjector, then any manual contributors.
let context_builder = {
    let budget = self
        .context_builder
        .as_ref()
        .map(|cb| cb.token_budget())
        .unwrap_or_else(|| TokenBudget::new(128_000));

    let mut b = ContextBuilderBuilder::new(budget.total)
        .head_size(budget.head_size)
        .tail_size(budget.tail_size);

    // 1. System prompt from AgentDef.prompt (first, Head(0))
    if let Some(ref def) = self.def {
        if !def.prompt.is_empty() {
            b = b.add_contributor(Box::new(
                vol_llm_context::builtin::SimpleContributor::system(def.prompt.clone()),
            ));
        }
    }

    // 2. SkillInjector — always
    b = b.add_contributor(Box::new(SkillInjector::new(skill_loader)));

    // 3. Clone existing context_builder contributors (if any)
    if let Some(ref cb) = self.context_builder {
        b = b.add_contributors_from(cb);
    }

    // 4. Manual contributors from with_system_prompt / with_contributor
    for c in self.contributors {
        b = b.add_contributor(c);
    }

    b.build()
};
```

- [ ] **Step 2: Verify it compiles**

```bash
cargo check -p vol-llm-agent 2>&1 | tail -5
```

Expected: `Finished` with no errors.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/config_builder.rs
git commit -m "feat(agent): auto-inject system prompt contributor before SkillInjector in build()"
```

---

### Task 2: AgentConfig — delete `new()`, add contributor API

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:20-69`

- [ ] **Step 1: Delete the `new()` method and make `context_builder` pub(crate)**

Replace lines 45-69:

```rust
impl AgentConfig {
    /// Create a new builder for AgentConfig.
    pub fn builder() -> super::config_builder::AgentConfigBuilder {
        super::config_builder::AgentConfigBuilder::new()
    }

    /// Add a context contributor. Call before agent starts.
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>) {
        self.context_builder.add_contributor(contributor);
    }

    /// List contributor info (for RPC / UI queries).
    pub async fn contributor_infos(&self) -> Result<Vec<ContributorInfo>, ContextError> {
        self.context_builder.contributor_infos().await
    }

    /// Get message snapshot from a specific contributor.
    pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
        self.context_builder.snapshot_by_name(name).await
    }
}
```

Also, on line 33 change:
```rust
// before
pub context_builder: ContextBuilder,
// after
pub(crate) context_builder: ContextBuilder,
```

- [ ] **Step 2: Add required imports to agent.rs**

At the top of the file, add:
```rust
use vol_llm_context::{ContextError, ContextMessage, ContributorInfo, ContextContributor};
```

- [ ] **Step 3: Check compilation**

```bash
cargo check -p vol-llm-agent 2>&1 | tail -5
```

Expected: fails with errors about `AgentConfig::new` callers — this is expected, we fix them next.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "refactor(agent): delete AgentConfig::new(), add contributor API, restrict context_builder visibility"
```

---

### Task 3: ReActAgent — expose contributor API as SOT

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs` (near line 179)

- [ ] **Step 1: Add contributor methods to ReActAgent**

After the existing `config()` method block:

```rust
impl ReActAgent {
    /// Add a context contributor at runtime.
    pub fn add_contributor(&mut self, contributor: Box<dyn ContextContributor>) {
        self.config.context_builder.add_contributor(contributor);
    }

    /// List all contributors with metadata (SOT for external queries).
    pub async fn contributors(&self) -> Result<Vec<ContributorInfo>, ContextError> {
        self.config.contributor_infos().await
    }

    /// Get messages from a specific contributor by name.
    pub async fn snapshot_by_name(&self, name: &str) -> Result<Vec<ContextMessage>, ContextError> {
        self.config.snapshot_by_name(name).await
    }
}
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p vol-llm-agent 2>&1 | tail -5
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat(agent): expose contributor API on ReActAgent as SOT"
```

---

### Task 4: Agent list stable sorting

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/domain/agent.rs:162-189`

- [ ] **Step 1: Update RPC handlers to use agent methods + sort list**

Replace lines 227-289 (ContextConfig and ContextSnapshot handlers) to use `agent.contributors()` / `agent.snapshot_by_name()`:

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

    let contributors = agent.contributors().await.map(|infos| {
        infos.into_iter().map(|info| {
            serde_json::json!({
                "name": info.name,
                "anchor_zone": info.anchor_zone,
                "estimated_tokens": info.estimated_tokens,
                "message_count": info.message_count,
            })
        }).collect::<Vec<_>>()
    }).unwrap_or_default();

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

    let messages = agent.snapshot_by_name(&contributor_name).await
        .map(|msgs| {
            msgs.into_iter()
                .map(|m| serde_json::json!({"role": m.role, "content": m.content}))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(vec![AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::ContextSnapshot),
        Payload::Agent(AgentPayload::ContextSnapshotResult { messages }),
    )])
}
```

- [ ] **Step 2: Add stable sorting to agent.list handler**

Replace lines 162-189 (the `List` operation handler):

```rust
(AgentOperation::List, _) => {
    let defs = self.agent_defs.read().unwrap();
    let mut agents: Vec<serde_json::Value> = self
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
                "scope": def.map_or("repo", |d| match d.scope {
                    vol_llm_core::AgentScope::Repo => "repo",
                    vol_llm_core::AgentScope::User => "user",
                }),
                "status": "idle",
                "current_input": None::<String>,
            })
        })
        .collect();

    // Stable sort: repo first, user second; alphabetical by name within group
    fn scope_rank(scope: &str) -> u8 {
        match scope {
            "repo" => 0,
            _ => 1,
        }
    }
    agents.sort_by(|a, b| {
        let sa = a.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        let sb = b.get("scope").and_then(|v| v.as_str()).unwrap_or("");
        let na = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let nb = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
        scope_rank(sa).cmp(&scope_rank(sb))
            .then_with(|| na.cmp(nb))
    });

    Ok(vec![AgentServerMessage::new_result(
        message.message_id,
        Operation::Agent(AgentOperation::List),
        Payload::Agent(AgentPayload::ListResult { agents }),
    )])
}
```

- [ ] **Step 3: Check compilation**

```bash
cargo check -p vol-llm-agent-channel 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/domain/agent.rs
git commit -m "feat(agent-channel): use agent SOT for context RPC, stable sort agent.list"
```

---

### Task 5: server_core.rs — switch to builder

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs:182-192` (register_agent)
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs:491-498` (for_test)

- [ ] **Step 1: Update `register_agent()`**

Replace lines 182-192:

```rust
let agent_id_str = agent_id.clone();
let mut config = AgentConfig::builder()
    .with_def(def.clone())
    .with_llm(llm)
    .with_tools(tools)
    .with_session(session)
    .with_working_dir(agent_dir.clone())
    .build()
    .expect("AgentConfig build failed — LLM, tools, and session are all provided");

config.mcp_manager = Some(mcp);

let holder = ConnectionHolder::new(
    agent_id_str.clone(),
    "client".to_string(),
    Some(self.agent_status.clone()),
);
config.plugin_registry.register(holder.clone());

let agent = vol_llm_agent::ReActAgent::new(config);
```

- [ ] **Step 2: Update `for_test()`**

Replace lines 491-498:

```rust
let config = AgentConfig::builder()
    .with_llm(Arc::new(TestLlm))
    .with_tools(tools)
    .with_session(session)
    .build()
    .expect("AgentConfig build failed for test");
```

- [ ] **Step 3: Check compilation**

```bash
cargo check -p vol-llm-agent-channel 2>&1 | tail -10
```

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent-channel/src/server_core.rs
git commit -m "refactor(server-core): switch AgentConfig construction to builder pattern"
```

---

### Task 6: vol-llm-runtime — switch to builder

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs:117-122`

- [ ] **Step 1: Update `build_agent()` in AgentRuntime**

Replace lines 117-122:

```rust
let mut config = AgentConfig::builder()
    .with_def(def.clone())
    .with_llm(llm)
    .with_tools(self.tool_registry.clone())
    .with_session(session)
    .with_working_dir(agent_dir.clone())
    .build()
    .expect("AgentConfig build failed — all required fields provided");

config.mcp_manager = Some(self.mcp_manager.clone());

let agent = ReActAgent::new(config);
```

- [ ] **Step 2: Check compilation**

```bash
cargo check -p vol-llm-runtime 2>&1 | tail -10
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-runtime/src/lib.rs
git commit -m "refactor(runtime): switch AgentConfig construction to builder pattern"
```

---

### Task 7: Fix test code — switch to builder

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:774-779`

- [ ] **Step 1: Update test helper**

Replace `make_config()`:

```rust
fn make_config() -> AgentConfig {
    AgentConfig::builder()
        .with_llm(Arc::new(MockLlm))
        .with_tools(Arc::new(ToolRegistry::new()))
        .with_session(Arc::new(Session::new(Arc::new(InMemoryEntryStore::new()))))
        .build()
        .expect("Test config build failed")
}
```

- [ ] **Step 2: Run agent tests**

```bash
cargo test -p vol-llm-agent 2>&1 | tail -15
```

Expected: all tests pass.

- [ ] **Step 3: Run full workspace check**

```bash
cargo check --workspace 2>&1 | tail -10
```

Expected: clean compile with no errors.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "test(agent): switch test helper to AgentConfig builder"
```

---

### Task 8: Final verification

- [ ] **Step 1: Run all tests**

```bash
cargo test --workspace 2>&1 | tail -20
```

- [ ] **Step 2: Check there are no remaining `AgentConfig::new` calls**

```bash
grep -rn "AgentConfig::new" crates/ --include="*.rs" | grep -v "\.git\|target\|fn new\|//.*new"
```

Expected: empty (no callers remain).

- [ ] **Step 3: Verify contributor_infos shows system prompt after builder builds**

```bash
cargo test -p vol-llm-agent -- test_builder_with_def 2>&1
```

- [ ] **Step 4: Commit any remaining changes**

```bash
git status
```
