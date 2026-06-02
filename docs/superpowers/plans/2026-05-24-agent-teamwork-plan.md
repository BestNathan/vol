# Agent Teamwork Capability — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an AgentRuntime core, extend task tools with teamwork primitives (publisher/assignee/claim), and integrate into AgentServerCore and vol-agent-manager.

**Architecture:** New `vol-llm-runtime` crate holds `AgentRuntime` — the single owner of LLM registry, ToolRegistry, TaskStore, agent router, and lifecycle. `AgentServerCore` becomes a thin transport layer wrapping `AgentRuntime`. Task model gains `publisher`/`assignee` fields; tools gain identity awareness via `AgentDef` in `ToolContext`. `task_create` auto-fills publisher; new `task_claim` enables atomic task claiming; `task_list` gains assignee filtering.

**Tech Stack:** Rust 2021 edition, tokio, serde, existing workspace crates (vol-llm-core, vol-llm-tool, vol-llm-agent, vol-llm-task, vol-llm-agent-channel, vol-agent-manager).

**Circular Dependency Note:** `vol-llm-tool` cannot depend on `vol-llm-agent` (agent already depends on tool). Solution: move `AgentDef` (pure data struct) into `vol-llm-core`, which both crates already depend on. The frontmatter parsing (`AgentFrontmatter`, `AgentLoader`) stays in `vol-llm-agent`. `ToolContext` can then directly hold `Option<AgentDef>` — exactly what the user wanted, no intermediate types.

---

### Task 1: Move AgentDef to vol-llm-core

**Files:**
- Create: `crates/vol-llm-core/src/agent_def.rs`
- Modify: `crates/vol-llm-core/src/lib.rs`
- Modify: `crates/vol-llm-agent/src/agent_def.rs`
- Modify: `crates/vol-llm-agent/src/lib.rs`

- [ ] **Step 1: Move AgentDef and related types to vol-llm-core**

Move the following types from `crates/vol-llm-agent/src/agent_def.rs` into a new file `crates/vol-llm-core/src/agent_def.rs`:
- `AgentScope` (enum + impl Display + prefix())
- `AgentDef` (struct + all builder methods: new, with_type, with_description, etc.)
- `AgentMetadata` (struct + From<&AgentDef>)
- `AgentPath` (struct + root, push, depth, as_str, Display)
- `AgentDefError` (enum)

Keep in `vol-llm-agent/src/agent_def.rs`:
- `AgentFrontmatter` (struct + resolve_type, resolve_max_iterations)
- All tests (they can stay and import AgentDef from core)

- [ ] **Step 2: Add module to vol-llm-core/src/lib.rs**

```rust
pub mod agent_def;
pub use agent_def::{AgentDef, AgentMetadata, AgentPath, AgentScope, AgentDefError};
```

- [ ] **Step 3: Update vol-llm-agent to re-export from core**

In `crates/vol-llm-agent/src/agent_def.rs`, add `use` imports:

```rust
pub use vol_llm_core::agent_def::{AgentDef, AgentMetadata, AgentPath, AgentScope, AgentDefError};
// Keep: AgentFrontmatter, tests
```

In `crates/vol-llm-agent/src/lib.rs`, ensure re-exports still work:

```rust
pub use agent_def::{AgentDef, AgentFrontmatter, AgentMetadata, AgentPath, AgentScope, AgentDefError};
```

- [ ] **Step 4: Fix all imports across the workspace**

Update every file that uses `vol_llm_agent::AgentDef` (or similar paths) to import from `vol_llm_core`. Key files:

```bash
# Find all AgentDef usages
rg "AgentDef" crates/ --type rust -l
rg "AgentScope" crates/ --type rust -l
rg "AgentPath" crates/ --type rust -l
rg "AgentMetadata" crates/ --type rust -l
```

Update each to `use vol_llm_core::{AgentDef, ...}`. The `vol-llm-agent` crate should re-export for backward compatibility.

- [ ] **Step 5: Build check**

```bash
cargo check --workspace
```

Expected: compiles without error. Fix any import errors.

- [ ] **Step 6: Run tests**

```bash
cargo test --workspace
```

Expected: all tests pass (AgentDef tests now run in vol-llm-core context).

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-core/ crates/vol-llm-agent/ crates/vol-llm-tool/ crates/vol-llm-agent-channel/ crates/vol-agent-manager/
git commit -m "refactor: move AgentDef to vol-llm-core to resolve circular dep with vol-llm-tool

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 2: Extend Task model with publisher and assignee

**Files:**
- Modify: `crates/vol-llm-task/src/model.rs`

- [ ] **Step 1: Add fields to Task struct**

In `crates/vol-llm-task/src/model.rs`, add two fields to the `Task` struct after `kind`:

```rust
pub struct Task {
    pub id: TaskId,
    pub status: TaskStatus,
    pub kind: TaskKind,
    // NEW fields
    pub publisher: Option<String>,
    pub assignee: Option<String>,
    // existing fields
    pub subject: String,
    // ... rest unchanged
}
```

- [ ] **Step 2: Update Task::new() to initialize new fields**

```rust
pub fn new(kind: TaskKind, subject: String, dependencies: Vec<TaskId>) -> Self {
    Self {
        id: TaskId(0),
        status: TaskStatus::Pending,
        kind,
        publisher: None,       // NEW
        assignee: None,        // NEW
        subject,
        description: String::new(),
        // ... rest unchanged
    }
}
```

- [ ] **Step 3: Build and test**

```bash
cargo test -p vol-llm-task
```

Expected: all existing tests pass.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-task/src/model.rs
git commit -m "feat(vol-llm-task): add publisher and assignee fields to Task

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 3: Extend ToolContext with agent_def

**Files:**
- Modify: `crates/vol-llm-tool/src/tool.rs`

- [ ] **Step 1: Add agent_def field to ToolContext**

```rust
use vol_llm_core::AgentDef;

#[derive(Clone, Default)]
pub struct ToolContext {
    pub messages: Vec<Message>,
    pub sandbox: Option<SandboxRef>,
    pub agent_def: Option<AgentDef>,  // NEW — set by ReActAgent during tool execution
}
```

- [ ] **Step 2: Add with_agent_def builder method**

```rust
impl ToolContext {
    pub fn with_sandbox(mut self, sandbox: SandboxRef) -> Self {
        self.sandbox = Some(sandbox);
        self
    }

    // NEW
    pub fn with_agent_def(mut self, def: AgentDef) -> Self {
        self.agent_def = Some(def);
        self
    }
}
```

- [ ] **Step 3: Update Debug impl**

```rust
impl std::fmt::Debug for ToolContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ToolContext")
            .field("messages", &self.messages)
            .field("sandbox", &self.sandbox.as_ref().map(|_| "<sandbox>"))
            .field("agent_def", &self.agent_def)
            .finish()
    }
}
```

- [ ] **Step 4: Build check**

```bash
cargo check -p vol-llm-tool
```

Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-tool/src/tool.rs
git commit -m "feat(vol-llm-tool): add agent_def to ToolContext

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 4: ReActAgent injects AgentDef into ToolContext

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

- [ ] **Step 1: Inject AgentDef when executing tools in ReActAgent**

Find where `tool_registry.execute()` is called in `crates/vol-llm-agent/src/react/agent.rs`. Update to inject AgentDef directly into context (no conversion needed since AgentDef lives in core now):

```rust
// Before: self.tools.execute(&call, &context).await
// After:
if let Some(ref def) = self.config.def {
    context = context.with_agent_def(def.clone());
}
self.tools.execute(&call, &context).await
```

The exact line depends on the current code structure. Look for `self.tools.execute` or `tool_registry.execute` in `agent.rs`.

- [ ] **Step 3: Build and test**

```bash
cargo test -p vol-llm-agent
```

Expected: all tests pass (some may need minor updates if they construct ToolContext directly).

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-agent/src/agent_def.rs crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat(vol-llm-agent): inject AgentDef into ToolContext during tool execution

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 5: Extend task_create with assignee and auto-publisher

**Files:**
- Modify: `crates/vol-llm-task/src/tools/task_create.rs`

- [ ] **Step 1: Add assignee to params struct**

```rust
#[derive(Debug, Deserialize)]
struct TaskCreateParams {
    subject: String,
    #[serde(default)]
    description: String,
    #[serde(rename = "activeForm", default)]
    active_form: Option<String>,
    // NEW
    #[serde(default)]
    assignee: Option<String>,
}
```

- [ ] **Step 2: Update parameters JSON schema**

Add to the `parameters()` method's JSON:

```rust
"assignee": {
    "type": "string",
    "description": "Agent type to assign this task to. Omit for open claim."
}
```

- [ ] **Step 3: Populate publisher from context, assignee from params**

```rust
async fn execute(&self, args: &serde_json::Value, context: &ToolContext) -> ToolResultType<ToolResult> {
    let params: TaskCreateParams = serde_json::from_value(args.clone())
        .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse arguments: {}", e)))?;

    let mut task = Task::new(TaskKind::Agent, params.subject.clone(), vec![]);
    task.description = params.description;
    task.active_form = params.active_form;

    // NEW: publisher from context, assignee from params
    task.publisher = context.agent_def.as_ref().map(|a| a.r#type.clone());
    task.assignee = params.assignee;

    let id = self.store.create(task).await.map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
    // ... rest unchanged
}
```

- [ ] **Step 4: Build and test**

```bash
cargo test -p vol-llm-task
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-task/src/tools/task_create.rs
git commit -m "feat(vol-llm-task): extend task_create with assignee param and auto-publisher

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 6: Add task_claim tool

**Files:**
- Create: `crates/vol-llm-task/src/tools/task_claim.rs`
- Modify: `crates/vol-llm-task/src/tools/mod.rs`

- [ ] **Step 1: Write the task_claim tool**

Create `crates/vol-llm-task/src/tools/task_claim.rs`:

```rust
//! task_claim — atomically claim a pending task.

use std::sync::Arc;

use async_trait::async_trait;
use serde::Deserialize;
use vol_llm_tool::{
    ExecutableTool, ToolContext, ToolError, ToolResult, ToolResultType, ToolSensitivity,
};

use crate::model::{TaskStatus, TaskId};
use crate::store::TaskStore;

#[derive(Debug, Deserialize)]
struct TaskClaimParams {
    #[serde(rename = "taskId")]
    task_id: String,
}

pub struct TaskClaim {
    store: Arc<dyn TaskStore>,
}

impl TaskClaim {
    pub fn new(store: Arc<dyn TaskStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ExecutableTool for TaskClaim {
    fn name(&self) -> &'static str {
        "task_claim"
    }

    fn description(&self) -> &'static str {
        "Claim a pending task and execute it. \
         Sets task status to Running and assigns it to you. \
         Returns the task content (subject + description) so you can start working on it."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "taskId": {
                    "type": "string",
                    "description": "ID of the task to claim (e.g. 't1', 't42')"
                }
            },
            "required": ["taskId"]
        })
    }

    fn sensitivity(&self, _args: &serde_json::Value) -> ToolSensitivity {
        ToolSensitivity::Safe
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        context: &ToolContext,
    ) -> ToolResultType<ToolResult> {
        let params: TaskClaimParams = serde_json::from_value(args.clone())
            .map_err(|e| ToolError::InvalidArguments(format!("Failed to parse task ID: {}", e)))?;

        // Parse task ID
        let raw = params.task_id.trim_start_matches('t');
        let id_num: u64 = raw.parse().map_err(|_| {
            ToolError::InvalidArguments(format!("Invalid task ID: {}", params.task_id))
        })?;
        let task_id = TaskId(id_num);

        // Get caller identity
        let caller_type = context
            .agent_def
            .as_ref()
            .map(|a| a.r#type.clone())
            .ok_or_else(|| {
                ToolError::ExecutionFailed("agent identity required for task_claim".into())
            })?;

        // Atomic claim: get, validate, update
        let mut task = self
            .store
            .get(&task_id)
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?
            .ok_or_else(|| ToolError::ExecutionFailed(format!("Task {} not found", params.task_id)))?;

        if task.status != TaskStatus::Pending {
            return Err(ToolError::ExecutionFailed(format!(
                "Task {} is not in Pending status (current: {:?})",
                params.task_id, task.status
            )));
        }

        // Check dependencies are all completed
        let ready_ids = self
            .store
            .get_ready_tasks()
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;
        if !ready_ids.contains(&task_id) {
            let uncompleted: Vec<String> = task
                .dependencies
                .iter()
                .filter(|d| !ready_ids.contains(d))
                .map(|d| d.to_string())
                .collect();
            return Err(ToolError::ExecutionFailed(format!(
                "Task {} has uncompleted dependencies: [{}]",
                params.task_id,
                uncompleted.join(", ")
            )));
        }

        // Claim it
        task.status = TaskStatus::Running;
        task.assignee = Some(caller_type);
        task.started_at = Some(std::time::SystemTime::now());
        self.store
            .update(task.clone())
            .await
            .map_err(|e| ToolError::ExecutionFailed(e.to_string()))?;

        let output = format!(
            "Task {} claimed and now Running.\n\n---\n# {}\n\n{}",
            params.task_id, task.subject, task.description
        );

        Ok(ToolResult {
            success: true,
            content: output,
            error: None,
            data: Some(serde_json::json!({
                "task": {
                    "id": task.id.to_string(),
                    "subject": task.subject,
                    "description": task.description,
                    "status": "Running",
                    "publisher": task.publisher,
                    "assignee": task.assignee,
                }
            })),
            call_id: String::new(),
        })
    }
}
```

- [ ] **Step 2: Register task_claim in mod.rs**

In `crates/vol-llm-task/src/tools/mod.rs`:

```rust
mod task_claim;
pub use task_claim::TaskClaim;

pub fn register_all(registry: &mut vol_llm_tool::ToolRegistry, store: Arc<dyn TaskStore>) {
    registry.register(TaskCreate::new(store.clone()));
    registry.register(TaskGet::new(store.clone()));
    registry.register(TaskList::new(store.clone()));
    registry.register(TaskOutput::new(store.clone()));
    registry.register(TaskStop::new(store.clone()));
    registry.register(TaskUpdate::new(store.clone()));
    registry.register(TaskClaim::new(store));  // NEW
}
```

- [ ] **Step 3: Build and test**

```bash
cargo test -p vol-llm-task
```

Expected: compiles, existing tests pass.

- [ ] **Step 4: Write task_claim unit test**

Add to `crates/vol-llm-task/src/tools/task_claim.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Task, TaskKind};
    use crate::stores::InMemoryTaskStore;
    use vol_llm_core::AgentDef;
    use vol_llm_tool::ToolContext;

    fn make_context(agent_type: &str) -> ToolContext {
        ToolContext::default().with_agent_def(AgentDef::new(agent_type, String::new()))
    }

    #[tokio::test]
    async fn test_claim_pending_task() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let mut task = Task::new(TaskKind::Agent, "Test task".into(), vec![]);
        task.description = "Do something useful".into();
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("coding");
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await.unwrap();

        assert!(result.content.contains("Test task"));
        assert!(result.content.contains("Do something useful"));

        let stored = store.get(&task_id).await.unwrap().unwrap();
        assert_eq!(stored.status, TaskStatus::Running);
        assert_eq!(stored.assignee, Some("coding".into()));
    }

    #[tokio::test]
    async fn test_claim_non_pending_fails() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let mut task = Task::new(TaskKind::Agent, "Done task".into(), vec![]);
        task.status = TaskStatus::Completed;
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("qa");
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("not in Pending status"));
    }

    #[tokio::test]
    async fn test_claim_without_identity_fails() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let task = Task::new(TaskKind::Agent, "Test".into(), vec![]);
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = ToolContext::default(); // no agent_def
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("agent identity required"));
    }

    #[tokio::test]
    async fn test_claim_with_uncompleted_dependency_fails() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let dep = Task::new(TaskKind::Agent, "Dependency".into(), vec![]);
        let dep_id = store.create(dep).await.unwrap();

        let task = Task::new(TaskKind::Agent, "Depends on other".into(), vec![dep_id]);
        let task_id = store.create(task).await.unwrap();

        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("coding");
        let args = serde_json::json!({"taskId": task_id.to_string()});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("uncompleted dependencies"));
    }

    #[tokio::test]
    async fn test_claim_not_found() {
        let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
        let tool = TaskClaim::new(store.clone());
        let ctx = make_context("coding");
        let args = serde_json::json!({"taskId": "t999"});
        let result = tool.execute(&args, &ctx).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p vol-llm-task --lib -- task_claim
```

Expected: 5 tests pass.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-task/src/tools/task_claim.rs crates/vol-llm-task/src/tools/mod.rs
git commit -m "feat(vol-llm-task): add task_claim tool with atomic claim semantics

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 7: Enhance task_list with assignee filter

**Files:**
- Modify: `crates/vol-llm-task/src/tools/task_list.rs`

- [ ] **Step 1: Add assignee param to the params struct**

```rust
#[derive(Debug, Deserialize)]
struct TaskListParams {
    #[serde(default)]
    status: Option<String>,
    // NEW
    #[serde(default)]
    assignee: Option<String>,
}
```

- [ ] **Step 2: Update parameters JSON schema**

In the `parameters()` method, add:

```rust
"assignee": {
    "type": "string",
    "description": "Filter by assignee: 'me' (current agent), specific agent_type, or 'unassigned'"
}
```

- [ ] **Step 3: Add filtering logic in execute()**

After getting the task list from the store, apply assignee filter:

```rust
// Resolve assignee filter
if let Some(ref assignee_filter) = params.assignee {
    let effective_filter = match assignee_filter.as_str() {
        "me" => context.agent_def.as_ref().map(|a| a.r#type.clone()),
        "unassigned" => Some(String::new()), // special marker for None
        other => Some(other.to_string()),
    };

    if let Some(filter) = effective_filter {
        if filter.is_empty() {
            // "unassigned" — keep only tasks with no assignee
            tasks.retain(|t| t.assignee.is_none());
        } else {
            tasks.retain(|t| t.assignee.as_deref() == Some(&filter));
        }
    }
}
```

- [ ] **Step 4: Write tests**

Add to the test module in `task_list.rs`:

```rust
use vol_llm_core::AgentDef;
use vol_llm_tool::ToolContext;

fn make_context(agent_type: &str) -> ToolContext {
    ToolContext::default().with_agent_def(AgentDef::new(agent_type, String::new()))
}

#[tokio::test]
async fn test_list_by_assignee_me() {
    let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
    let mut t1 = Task::new(TaskKind::Agent, "Task 1".into(), vec![]);
    t1.assignee = Some("coding".into());
    store.create(t1).await.unwrap();

    let mut t2 = Task::new(TaskKind::Agent, "Task 2".into(), vec![]);
    t2.assignee = Some("qa".into());
    store.create(t2).await.unwrap();

    let mut t3 = Task::new(TaskKind::Agent, "Task 3".into(), vec![]);
    store.create(t3).await.unwrap();

    let tool = TaskList::new(store.clone());
    let ctx = make_context("coding");
    let args = serde_json::json!({"assignee": "me"});
    let result = tool.execute(&args, &ctx).await.unwrap();

    // Should only return Task 1 (assigned to "coding")
    let data = result.data.unwrap();
    let tasks = data.get("tasks").unwrap().as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["subject"], "Task 1");
}

#[tokio::test]
async fn test_list_unassigned() {
    let store: Arc<dyn TaskStore> = Arc::new(InMemoryTaskStore::new());
    let mut t1 = Task::new(TaskKind::Agent, "Assigned".into(), vec![]);
    t1.assignee = Some("coding".into());
    store.create(t1).await.unwrap();

    let t2 = Task::new(TaskKind::Agent, "Unassigned".into(), vec![]);
    store.create(t2).await.unwrap();

    let tool = TaskList::new(store.clone());
    let ctx = make_context("qa");
    let args = serde_json::json!({"assignee": "unassigned"});
    let result = tool.execute(&args, &ctx).await.unwrap();

    let data = result.data.unwrap();
    let tasks = data.get("tasks").unwrap().as_array().unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["subject"], "Unassigned");
}
```

- [ ] **Step 5: Run tests**

```bash
cargo test -p vol-llm-task --lib -- task_list
```

Expected: all tests pass (existing + new).

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-task/src/tools/task_list.rs
git commit -m "feat(vol-llm-task): enhance task_list with assignee filter (me/unassigned/specific)

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 8: Create vol-llm-runtime crate

**Files:**
- Create: `crates/vol-llm-runtime/Cargo.toml`
- Create: `crates/vol-llm-runtime/src/lib.rs`

- [ ] **Step 1: Create Cargo.toml**

```toml
[package]
name = "vol-llm-runtime"
version.workspace = true
edition.workspace = true

[dependencies]
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
anyhow = { workspace = true }
uuid = { workspace = true }
async-trait = { workspace = true }

vol-llm-core = { path = "../vol-llm-core" }
vol-llm-tool = { path = "../vol-llm-tool" }
vol-llm-agent = { path = "../vol-llm-agent" }
vol-llm-task = { path = "../vol-llm-task" }
vol-llm-provider = { path = "../vol-llm-provider" }
vol-llm-mcp = { path = "../vol-llm-mcp" }
vol-llm-skill = { path = "../vol-llm-skill" }
vol-llm-tools-builtin = { path = "../vol-llm-tools-builtin" }
vol-session = { path = "../vol-session" }
```

Register in root `Cargo.toml` workspace members:

```toml
"crates/vol-llm-runtime",
```

- [ ] **Step 2: Write AgentRuntime struct and AgentRuntimeHandle**

Create `crates/vol-llm-runtime/src/lib.rs`:

```rust
//! AgentRuntime — core runtime for the multi-agent system.
//!
//! Owns all runtime resources: LLM registry, tool registry, task store,
//! agent router, agent definitions, and agent status tracking.
//! Provides lifecycle methods (run/stop) and agent registration.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use vol_llm_agent::agent_def::AgentDef;
use vol_llm_agent::AgentLoader;
use vol_llm_mcp::{McpConfig, McpManager};
use vol_llm_provider::{create_provider, ProviderLoader};
use vol_llm_skill::SkillLoader;
use vol_llm_task::stores::FileTaskStore;
use vol_llm_task::store::TaskStore;
use vol_llm_tool::ToolRegistry;

use vol_llm_agent::react::AgentConfig;
use vol_llm_agent::ReActAgent;
use vol_session::file_store::FileSessionEntryStore;
use vol_session::Session;

mod router;
pub use router::AgentRouter;

/// Runtime status of a registered agent.
#[derive(Debug, Clone, Default)]
pub struct AgentStatus {
    pub status: String, // "idle" | "running"
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

/// Handle returned by AgentRuntime::run(), used to control runtime lifecycle.
pub struct AgentRuntimeHandle {
    shutdown_tx: tokio::sync::oneshot::Sender<()>,
    pub join_handle: tokio::task::JoinHandle<()>,
}

impl AgentRuntimeHandle {
    /// Gracefully stop the runtime.
    /// Sends shutdown signal and waits for the runtime to finish.
    pub async fn stop(self) {
        let _ = self.shutdown_tx.send(());
        let _ = self.join_handle.await;
    }
}

/// Core agent runtime.
///
/// Owns all shared resources and manages agent lifecycle.
/// Created via AgentRuntimeBuilder, then started with run().
pub struct AgentRuntime {
    /// Working directory for agent discovery and config
    working_dir: PathBuf,
    /// Store directory for sessions, logs, and state
    store_dir: PathBuf,

    // Registries
    llm_registry: ProviderLoader,
    tool_registry: Arc<ToolRegistry>,
    task_store: Arc<dyn TaskStore>,
    mcp_manager: Arc<McpManager>,
    skill_loader: Arc<SkillLoader>,

    // Agent runtime
    router: AgentRouter,
    agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
    agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>>,
}

impl AgentRuntime {
    /// Create a new builder.
    pub fn builder(working_dir: impl Into<PathBuf>, store_dir: impl Into<PathBuf>) -> AgentRuntimeBuilder {
        AgentRuntimeBuilder::new(working_dir.into(), store_dir.into())
    }

    // Accessors
    pub fn working_dir(&self) -> &std::path::Path { &self.working_dir }
    pub fn store_dir(&self) -> &std::path::Path { &self.store_dir }
    pub fn llm_registry(&self) -> &ProviderLoader { &self.llm_registry }
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> { &self.tool_registry }
    pub fn task_store(&self) -> &Arc<dyn TaskStore> { &self.task_store }
    pub fn mcp_manager(&self) -> &Arc<McpManager> { &self.mcp_manager }
    pub fn skill_loader(&self) -> &Arc<SkillLoader> { &self.skill_loader }
    pub fn router(&self) -> &AgentRouter { &self.router }
    pub fn agent_defs(&self) -> &Arc<std::sync::RwLock<HashMap<String, AgentDef>>> { &self.agent_defs }
    pub fn agent_status(&self) -> &Arc<std::sync::RwLock<HashMap<String, AgentStatus>>> { &self.agent_status }

    /// Register an agent into the runtime.
    ///
    /// Creates a ReActAgent internally with the runtime's shared resources.
    /// The agent is registered in the router and available for task dispatch.
    pub async fn register_agent(
        &self,
        agent_id: impl Into<String>,
        def: AgentDef,
    ) -> Result<(), String> {
        let agent_id = agent_id.into();
        let agent_dir = self.store_dir.join("agents").join(&agent_id);
        let sessions_dir = agent_dir.join("sessions");
        std::fs::create_dir_all(&sessions_dir)
            .map_err(|e| format!("failed to create agent dirs: {e}"))?;

        // Resolve LLM for this agent: prefer agent-specified model, fall back to first provider
        let llm = if let Some(ref model_name) = def.model {
            self.llm_registry.get(model_name)
                .map(|fc| create_provider(&fc.to_llm_config()))
                .transpose()
                .map_err(|e| format!("LLM config error: {e}"))?
                .or_else(|| {
                    // Fall back to first available
                    self.llm_registry.ids().first().and_then(|id| {
                        let fc = self.llm_registry.get(id)?;
                        create_provider(&fc.to_llm_config()).ok()
                    })
                })
                .ok_or_else(|| format!("No LLM provider available for agent '{}'", agent_id))?
        } else {
            // No model specified — use first available
            let first_id = self.llm_registry.ids().first()
                .ok_or_else(|| "No LLM providers configured".to_string())?;
            let fc = self.llm_registry.get(first_id)
                .ok_or_else(|| "Provider not found".to_string())?;
            create_provider(&fc.to_llm_config())
                .map_err(|e| format!("LLM creation error: {e}"))?
        };
        let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::from(llm);

        let session_store = Arc::new(FileSessionEntryStore::new(&sessions_dir));
        let session = Arc::new(Session::new(session_store));

        let mut config = AgentConfig::new(llm, self.tool_registry.clone(), session);
        config.def = Some(def);
        config.working_dir = agent_dir.clone();
        config.mcp_manager = Some(self.mcp_manager.clone());

        let agent = ReActAgent::new(config);
        let dispatcher = Arc::new(crate::router::AgentDispatcher::new(agent));

        self.router.register(agent_id.clone(), dispatcher).await;

        // Store agent def and set status
        self.agent_defs.write().unwrap().insert(agent_id.clone(), config.def.clone().unwrap());
        self.agent_status.write().unwrap().insert(agent_id, AgentStatus::idle());

        Ok(())
    }

    /// Discover and register all agents from .agents/agents/ directories.
    pub async fn discover_agents(&self) -> Result<(), String> {
        let loader = AgentLoader::new(Some(self.working_dir.clone()));
        loader.discover_all().await.map_err(|e| e.to_string())?;

        let agents = loader.list_metadata().await;
        for meta in agents {
            if let Some(def) = loader.get(&meta.name).await {
                self.agent_defs.write().unwrap().insert(meta.name.clone(), (*def).clone());
                let arc_def = Arc::try_unwrap(def).unwrap_or_else(|arc| (*arc).clone());
                self.register_agent(&meta.name, arc_def).await?;
            }
        }
        Ok(())
    }

    /// Start the runtime.
    ///
    /// Connects MCP, discovers skills and agents, and begins listening for shutdown signal.
    /// Returns a handle that can be used to gracefully stop the runtime.
    pub async fn run(&self) -> AgentRuntimeHandle {
        // Connect MCP in background
        let mcp = self.mcp_manager.clone();
        tokio::spawn(async move {
            let _ = mcp.connect().await;
        });

        // Discover skills in background
        let skill = self.skill_loader.clone();
        tokio::spawn(async move {
            let _ = skill.discover_all().await;
        });

        // Discover agents
        if let Err(e) = self.discover_agents().await {
            tracing::warn!(error = %e, "Failed to discover agents at runtime start");
        }

        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        let status_map = self.agent_status.clone();
        let join_handle = tokio::spawn(async move {
            let _ = shutdown_rx.try_recv();
            tracing::info!("AgentRuntime shutdown signal received, starting graceful shutdown...");

            // Wait for running agents to become idle (with timeout)
            let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
            loop {
                let all_idle = status_map.read().unwrap()
                    .values()
                    .all(|s| s.status == "idle");
                if all_idle {
                    break;
                }
                if tokio::time::Instant::now() > deadline {
                    tracing::warn!("Graceful shutdown timeout, forcing stop");
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            }
            tracing::info!("AgentRuntime stopped");
        });

        AgentRuntimeHandle { shutdown_tx, join_handle }
    }

    /// List all registered agent IDs.
    pub async fn list_agent_ids(&self) -> Vec<String> {
        self.router.list_ids().await
    }
}

#[cfg(test)]
impl AgentRuntime {
    /// Create a minimal test runtime with in-memory resources.
    pub async fn for_test() -> Self {
        use vol_llm_task::stores::InMemoryTaskStore;
        use vol_llm_agent::AgentPath;

        let store_dir = PathBuf::from("/tmp/vol-llm-runtime-test");
        let working_dir = PathBuf::from(".");

        // Use empty ProviderLoader (tests inject their own)
        let llm_registry = ProviderLoader::empty();

        // Build tool registry with built-in + task tools
        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        let task_store = Arc::new(InMemoryTaskStore::new());
        vol_llm_task::tools::register_all(&mut tool_registry, task_store.clone());

        let tool_registry = Arc::new(tool_registry);
        let mcp_manager = Arc::new(McpManager::new(vec![]));
        let skill_loader = Arc::new(SkillLoader::new_empty());
        let router = AgentRouter::new();

        let agent_defs = Arc::new(std::sync::RwLock::new(HashMap::new()));
        let agent_status = Arc::new(std::sync::RwLock::new(HashMap::new()));

        AgentRuntime {
            working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            mcp_manager,
            skill_loader,
            router,
            agent_defs,
            agent_status,
        }
    }
}

// === Builder ===

pub struct AgentRuntimeBuilder {
    working_dir: PathBuf,
    store_dir: PathBuf,
}

impl AgentRuntimeBuilder {
    pub fn new(working_dir: PathBuf, store_dir: PathBuf) -> Self {
        Self { working_dir, store_dir }
    }

    /// Build the runtime. All internal registries are derived automatically.
    pub fn build(self) -> Result<AgentRuntime, String> {
        // Expand ~ in store_dir
        let store_dir = {
            let s = self.store_dir.to_string_lossy().to_string();
            if s.starts_with('~') {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                let rest = s.trim_start_matches('~').trim_start_matches('/');
                PathBuf::from(format!("{}/{}", home, rest))
            } else {
                self.store_dir
            }
        };

        let agents_root = store_dir.join("agents");
        std::fs::create_dir_all(&agents_root)
            .map_err(|e| format!("failed to create agents dir: {e}"))?;

        // LLM registry from .agents/providers/
        let llm_registry = ProviderLoader::load(Some(&self.working_dir));
        if llm_registry.is_empty() {
            return Err("No LLM provider configured in .agents/providers/*.toml".to_string());
        }

        // MCP manager from .mcp.json
        let mcp_manager = {
            let configs = McpConfig::load(Some(&self.working_dir))
                .map(|c| c.servers().to_vec())
                .unwrap_or_else(|e| {
                    tracing::warn!("Failed to load MCP config: {}", e);
                    vec![]
                });
            let manager = McpManager::new(configs);
            let mgr = manager.clone();
            tokio::spawn(async move { let _ = mgr.connect().await; });
            Arc::new(manager)
        };

        // Skill loader
        let skill_loader = {
            let loader = Arc::new(SkillLoader::new(Some(self.working_dir.clone())));
            let ld = Arc::clone(&loader);
            tokio::spawn(async move { let _ = ld.discover_all().await; });
            loader
        };

        // Task store (file-based for persistence)
        let tasks_dir = store_dir.join("tasks");
        std::fs::create_dir_all(&tasks_dir)
            .map_err(|e| format!("failed to create tasks dir: {e}"))?;
        let task_store: Arc<dyn TaskStore> = Arc::new(FileTaskStore::new(&tasks_dir));

        // Tool registry with built-in + task tools
        let mut tool_registry = ToolRegistry::new();
        vol_llm_tools_builtin::register_all(&mut tool_registry);
        vol_llm_task::tools::register_all(&mut tool_registry, task_store.clone());
        // Register web tools with default config
        let tool_config = vol_llm_tool::ToolConfig::default();
        vol_llm_tools_builtin::register_web_all(&mut tool_registry, &tool_config);
        let tool_registry = Arc::new(tool_registry);

        let router = AgentRouter::new();
        let agent_defs = Arc::new(std::sync::RwLock::new(HashMap::new()));
        let agent_status = Arc::new(std::sync::RwLock::new(HashMap::new()));

        Ok(AgentRuntime {
            working_dir: self.working_dir,
            store_dir,
            llm_registry,
            tool_registry,
            task_store,
            mcp_manager,
            skill_loader,
            router,
            agent_defs,
            agent_status,
        })
    }
}

fn expand_tilde(path: PathBuf) -> PathBuf {
    let s = path.to_string_lossy().to_string();
    if s.starts_with('~') {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let rest = s.trim_start_matches('~').trim_start_matches('/');
        PathBuf::from(format!("{}/{}", home, rest))
    } else {
        path
    }
}
```

Note: `AgentRouter` and `AgentDispatcher` are currently in `vol-llm-agent-channel`. They need to either move to `vol-llm-runtime` or be imported from the channel crate. To keep this plan focused, we'll initially import them from `vol-llm-agent-channel` and re-export. A follow-up cleanup can fully move them.

Actually, to keep the `vol-llm-runtime` crate self-contained and avoid a circular dependency (agent-channel depending on runtime, runtime depending on agent-channel), we need to either:
- Move `AgentRouter` and `AgentDispatcher` into `vol-llm-runtime` (preferred)
- Or put them in a shared crate

For this plan, we'll move `AgentRouter` and `AgentDispatcher` into `vol-llm-runtime`. They are runtime concepts, not transport concepts.

Create the router module at `crates/vol-llm-runtime/src/router.rs` — copy from `vol-llm-agent-channel/src/router.rs` and `vol-llm-agent-channel/src/dispatcher.rs`, adjusting imports.

- [ ] **Step 3: Copy AgentRouter and AgentDispatcher to vol-llm-runtime**

Move `crates/vol-llm-agent-channel/src/router.rs` and `crates/vol-llm-agent-channel/src/dispatcher.rs` into `crates/vol-llm-runtime/src/router.rs` and `crates/vol-llm-runtime/src/dispatcher.rs`. Update crate references from `crate::` to `crate::`. Update import paths.

In `vol-llm-agent-channel`, change these to re-exports from `vol-llm-runtime`:

```rust
// crates/vol-llm-agent-channel/src/router.rs
pub use vol_llm_runtime::router::AgentRouter;
```

In `vol-llm-agent-channel/src/dispatcher.rs`:
```rust
pub use vol_llm_runtime::dispatcher::AgentDispatcher;
```

Specific code to move — router.rs:

```rust
// crates/vol-llm-runtime/src/router.rs
use std::collections::HashMap;
use std::sync::Arc;

use crate::dispatcher::AgentDispatcher;
use vol_llm_agent_channel_crate::request::AgentRequest; // adjust import

/// Routes agent requests to the correct dispatcher by agent ID.
#[derive(Clone)]
pub struct AgentRouter {
    dispatchers: Arc<tokio::sync::RwLock<HashMap<String, Arc<AgentDispatcher>>>>,
}

impl AgentRouter {
    pub fn new() -> Self {
        Self { dispatchers: Arc::new(tokio::sync::RwLock::new(HashMap::new())) }
    }

    pub async fn register(&self, id: String, dispatcher: Arc<AgentDispatcher>) {
        self.dispatchers.write().await.insert(id, dispatcher);
    }

    pub async fn unregister(&self, id: &str) -> Option<Arc<AgentDispatcher>> {
        self.dispatchers.write().await.remove(id)
    }

    pub async fn send(&self, target_id: &str, request: AgentRequest) -> Result<AgentRequest, String> {
        let guard = self.dispatchers.read().await;
        let dispatcher = guard.get(target_id)
            .ok_or_else(|| format!("Agent '{}' not found", target_id))?;
        dispatcher.submit(request).await
    }

    pub async fn list_ids(&self) -> Vec<String> {
        self.dispatchers.read().await.keys().cloned().collect()
    }
}
```

```rust
// crates/vol-llm-runtime/src/dispatcher.rs
use std::collections::VecDeque;
use std::sync::Arc;

use tokio::sync::{oneshot, Mutex};
use vol_llm_agent::ReActAgent;
use crate::request::AgentRequest; // or from agent-channel if not moved

/// Dispatches agent requests serially, one at a time.
pub struct AgentDispatcher {
    agent: Arc<Mutex<ReActAgent>>,
    queue: Arc<Mutex<VecDeque<(AgentRequest, oneshot::Sender<Result<String, String>>)>>>,
    notify: Arc<tokio::sync::Notify>,
}
// ... (rest of the existing implementation)
```

Note: this step involves significant code movement. The exact implementation depends on the current source of router.rs and dispatcher.rs. If moving the full types causes import chain issues, an alternative is to define the traits in `vol-llm-runtime` and have `vol-llm-agent-channel` implement them.

For this plan, let's keep it simpler: `vol-llm-runtime` depends on `vol-llm-agent-channel` for `AgentRouter` and `AgentDispatcher` (since agent-channel is a lower layer). This avoids the import tangle.

- [ ] **Step 4: Build and test**

```bash
cargo check -p vol-llm-runtime
cargo test -p vol-llm-runtime
```

Expected: compiles, for_test() can be instantiated.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-runtime/ Cargo.toml
git commit -m "feat(vol-llm-runtime): add AgentRuntime with lifecycle and agent registration

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 9: Refactor AgentServerCore to use AgentRuntime

**Files:**
- Modify: `crates/vol-llm-agent-channel/src/server_core.rs`
- Modify: `crates/vol-llm-agent-channel/Cargo.toml`

- [ ] **Step 1: Add vol-llm-runtime dependency**

In `crates/vol-llm-agent-channel/Cargo.toml`:

```toml
vol-llm-runtime = { path = "../vol-llm-runtime" }
```

- [ ] **Step 2: Refactor AgentServerCore to delegate to AgentRuntime**

In `crates/vol-llm-agent-channel/src/server_core.rs`, replace the inline resource fields with a single `runtime` field:

```rust
use vol_llm_runtime::AgentRuntime;

pub struct AgentServerCore {
    runtime: AgentRuntime,
    holders: Arc<std::sync::Mutex<HashMap<String, Arc<ConnectionHolder>>>>,
    handler_registry: HandlerRegistry,
}

impl AgentServerCore {
    // Delegate to runtime for all resource access
    pub fn runtime(&self) -> &AgentRuntime { &self.runtime }
    pub fn working_dir(&self) -> &std::path::Path { self.runtime.working_dir() }
    pub fn store_dir(&self) -> &std::path::Path { self.runtime.store_dir() }
    pub fn llm(&self) -> &Arc<dyn LLMClient> { self.runtime.llm_registry().first().unwrap() } // compat
    pub fn tool_registry(&self) -> &Arc<ToolRegistry> { self.runtime.tool_registry() }
    pub fn mcp_manager(&self) -> &Arc<McpManager> { self.runtime.mcp_manager() }
    pub fn skill_loader(&self) -> &Arc<SkillLoader> { self.runtime.skill_loader() }
    pub fn router(&self) -> &AgentRouter { self.runtime.router() }
    pub fn agent_defs(&self) -> &Arc<RwLock<HashMap<String, AgentDef>>> { self.runtime.agent_defs() }
    pub fn agent_status(&self) -> &Arc<RwLock<HashMap<String, AgentStatus>>> { self.runtime.agent_status() }

    pub async fn register_agent(&self, agent_id: impl Into<String>, def: AgentDef) -> Result<(), String> {
        let agent_id = agent_id.into();
        self.runtime.register_agent(&agent_id, def).await?;

        // Additionally set up ConnectionHolder (transport concern)
        let holder = ConnectionHolder::new(
            agent_id.clone(),
            "client".to_string(),
            Some(self.runtime.agent_status().clone()),
        );
        self.holders.lock().unwrap().insert(agent_id, Arc::new(holder));

        Ok(())
    }

    pub async fn discover_agents(&self) -> Result<(), String> {
        self.runtime.discover_agents().await?;
        // After runtime registers agents, set up holders for each
        for agent_id in self.runtime.list_agent_ids().await {
            if !self.holders.lock().unwrap().contains_key(&agent_id) {
                let holder = ConnectionHolder::new(
                    agent_id.clone(),
                    "client".to_string(),
                    Some(self.runtime.agent_status().clone()),
                );
                self.holders.lock().unwrap().insert(agent_id, Arc::new(holder));
            }
        }
        Ok(())
    }

    // serve() and handle() unchanged — they delegate to self.handler_registry
}
```

- [ ] **Step 3: Update AgentServerCoreBuilder**

```rust
pub struct AgentServerCoreBuilder {
    runtime: AgentRuntime,
    extra_handlers: Vec<Arc<dyn DomainHandler>>,
}

impl AgentServerCoreBuilder {
    pub fn new(runtime: AgentRuntime) -> Self {
        Self { runtime, extra_handlers: Vec::new() }
    }

    pub fn register_handler(mut self, handler: Arc<dyn DomainHandler>) -> Self {
        self.extra_handlers.push(handler);
        self
    }

    pub async fn build(self) -> Result<AgentServerCore, String> {
        let router = self.runtime.router().clone();
        let holders: Arc<Mutex<HashMap<String, Arc<ConnectionHolder>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let agent_defs = self.runtime.agent_defs().clone();
        let agent_status = self.runtime.agent_status().clone();

        let mut handler_registry = HandlerRegistry::new();
        handler_registry
            .register(Arc::new(AgentHandler::new(
                router.clone(),
                Arc::clone(&holders),
                agent_defs.clone(),
                agent_status.clone(),
            ))).map_err(|e| format!("AgentHandler: {e}"))?;
        handler_registry
            .register(Arc::new(FileHandler::new(self.runtime.working_dir().to_path_buf())))
            .map_err(|e| format!("FileHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SessionHandler::new(
                self.runtime.store_dir().join("agents"),
                router.clone(),
            ))).map_err(|e| format!("SessionHandler: {e}"))?;
        handler_registry
            .register(Arc::new(McpHandler::new(Some(self.runtime.mcp_manager().clone()))))
            .map_err(|e| format!("McpHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SkillHandler::new(Some(self.runtime.skill_loader().clone()))))
            .map_err(|e| format!("SkillHandler: {e}"))?;
        handler_registry
            .register(Arc::new(ToolHandler::new(self.runtime.tool_registry().clone())))
            .map_err(|e| format!("ToolHandler: {e}"))?;
        handler_registry
            .register(Arc::new(LogHandler))
            .map_err(|e| format!("LogHandler: {e}"))?;
        handler_registry
            .register(Arc::new(SystemHandler))
            .map_err(|e| format!("SystemHandler: {e}"))?;

        for extra in self.extra_handlers {
            handler_registry.register(extra)
                .map_err(|e| format!("External handler: {e}"))?;
        }

        Ok(AgentServerCore {
            runtime: self.runtime,
            holders,
            handler_registry,
        })
    }
}
```

- [ ] **Step 4: Update for_test() to use AgentRuntime**

```rust
impl AgentServerCore {
    pub async fn for_test() -> Self {
        let runtime = AgentRuntime::for_test().await;
        // ... rest using runtime
    }
}
```

- [ ] **Step 5: Build and test**

```bash
cargo check -p vol-llm-agent-channel
```

Expected: compiles without error.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-channel/src/server_core.rs crates/vol-llm-agent-channel/Cargo.toml
git commit -m "refactor(vol-llm-agent-channel): use AgentRuntime in AgentServerCore

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 10: Integrate AgentRuntime into vol-agent-manager

**Files:**
- Modify: `crates/vol-agent-manager/src/main.rs`
- Modify: `crates/vol-agent-manager/src/ws/router.rs`
- Modify: `crates/vol-agent-manager/Cargo.toml`

- [ ] **Step 1: Add vol-llm-runtime dependency**

In `crates/vol-agent-manager/Cargo.toml`:

```toml
vol-llm-runtime = { path = "../vol-llm-runtime" }
```

- [ ] **Step 2: Create AgentRuntime in main.rs**

In `crates/vol-agent-manager/src/main.rs`, replace the manual agent loading setup:

```rust
use vol_llm_runtime::AgentRuntime;

#[tokio::main]
async fn main() -> Result<()> {
    // ... tracing_subscriber init ...

    let config_path = parse_args().unwrap_or_else(|| "config.toml".to_string());
    let config = ManagerConfig::from_path(&config_path)
        .unwrap_or_else(|e| {
            tracing::warn!("Failed to load {}: {}, using defaults", config_path, e);
            ManagerConfig::default()
        });

    let state_manager = Arc::new(AgentStateManager::new());
    let metrics = Arc::new(MetricsCollector::new());
    let event_bus = Arc::new(EventBus::new());
    let task_dispatcher = Arc::new(TaskDispatcher::new());

    // Create AgentRuntime
    let runtime = AgentRuntime::builder(
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        config.store_dir.clone().unwrap_or_else(|| PathBuf::from("~/.vol-agent-store")),
    ).build()?;
    let runtime_handle = runtime.run().await;
    let runtime = Arc::new(runtime);

    let agent_loader = Arc::new(vol_llm_agent::AgentLoader::new(None));
    if let Err(e) = agent_loader.discover_all().await {
        tracing::warn!(error = %e, "Failed to discover agent definitions");
    }
    let instance_registry = Arc::new(vol_agent_manager::instance::AgentInstanceRegistry::new());

    let llm_config = vol_llm_provider::LLMConfig::with_env_key(
        vol_llm_core::LLMProvider::Anthropic,
        "qwen3.5-plus",
        "ANTHROPIC_AUTH_TOKEN",
        "https://coding.dashscope.aliyuncs.com/apps/anthropic",
    );

    let app_state = AppRouterState {
        state_manager: state_manager.clone(),
        metrics: metrics.clone(),
        event_bus: event_bus.clone(),
        task_dispatcher: task_dispatcher.clone(),
        config: config.clone(),
        instance_registry: instance_registry.clone(),
        agent_loader: agent_loader.clone(),
        llm_config,
        runtime: runtime.clone(),  // NEW
    };

    // ... rest unchanged (router build, health checker, serve) ...
}
```

Update `AppRouterState` in `crates/vol-agent-manager/src/lib.rs`:

```rust
pub struct AppRouterState {
    // ... existing fields ...
    pub runtime: Arc<vol_llm_runtime::AgentRuntime>,  // NEW
}
```

- [ ] **Step 3: Update ws/router.rs to use runtime's tool registry**

In `run_agent_instance()`, instead of building tools manually, use the runtime's shared tool registry:

```rust
async fn run_agent_instance(
    agent_def: vol_llm_agent::AgentDef,
    session: Arc<vol_session::Session>,
    llm_config: vol_llm_provider::LLMConfig,
    broadcast_tx: tokio::sync::broadcast::Sender<serde_json::Value>,
    agent_type: String,
    session_id: String,
    user_input: String,
    agent_loader: Arc<vol_llm_agent::AgentLoader>,
    runtime: Arc<vol_llm_runtime::AgentRuntime>,  // NEW param
) {
    let llm = match create_provider(&llm_config) {
        Ok(client) => client,
        Err(e) => { /* error handling unchanged */ return; }
    };
    let llm: Arc<dyn vol_llm_core::LLMClient> = Arc::from(llm);

    // Use runtime's shared tool registry (already has built-in + web + task tools)
    let tools_arc = runtime.tool_registry().clone();

    let working_dir = agent_def.working_dir.clone().unwrap_or_else(|| std::path::PathBuf::from("."));
    let agent_tool = AgentTool::new(
        agent_loader,
        llm.clone(),
        AgentPath::root(),
        3,
        tools_arc.clone(),
        working_dir,
    );

    // Build agent from definition.
    let system_prompt = agent_def.prompt.clone();
    let agent_config = AgentConfig::builder()
        .with_def(agent_def)
        .with_llm(llm)
        .with_session(session)
        .with_system_prompt(system_prompt)
        .with_tools(tools_arc)
        .with_tool(agent_tool)
        .build();

    // ... rest unchanged ...
}
```

Update the call site to pass `runtime`:

```rust
let runtime = state.runtime.clone();
async move {
    run_agent_instance(
        agent_def, session, llm_config, broadcast_tx,
        agent_type, session_id, content, agent_loader, runtime,
    ).await;
}
```

- [ ] **Step 4: Build and test**

```bash
cargo check -p vol-agent-manager
```

Expected: compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-manager/
git commit -m "feat(vol-agent-manager): integrate AgentRuntime for shared resources

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 11: Cleanup — remove duplicate task tool registration from ws/router.rs

**Files:**
- Modify: `crates/vol-agent-manager/src/ws/router.rs`

- [ ] **Step 1: Remove now-redundant manual tool registration**

Since the runtime's shared tool_registry already has all tools (built-in, web, task), remove the manual `ToolRegistry::new()` + `register_all()` calls from `run_agent_instance()`. The function now only needs `AgentTool` added separately (since it needs the registry Arc).

The code in Task 10 already reflects this — verify the manual registry construction lines are gone.

- [ ] **Step 2: Remove unused imports**

Remove these imports from `ws/router.rs` (no longer needed):
```rust
// REMOVE:
use vol_llm_task::InMemoryTaskStore;
use vol_llm_tool::{ToolConfig, ToolRegistry};
// KEEP: AgentTool, AgentPath, AgentConfig, ReActAgent still needed
```

- [ ] **Step 3: Build and test**

```bash
cargo check -p vol-agent-manager
cargo build -p vol-agent-manager
```

Expected: compiles cleanly, no unused import warnings from ws/router.rs.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-manager/src/ws/router.rs
git commit -m "refactor(vol-agent-manager): remove duplicate tool registration, use runtime's registry

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```

---

### Task 12: End-to-end verification

**Files:** None (test run)

- [ ] **Step 1: Full workspace build**

```bash
cargo build --workspace
```

Expected: all crates compile.

- [ ] **Step 2: Run all tests**

```bash
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 3: Run specific integration checks**

```bash
# Task model tests
cargo test -p vol-llm-task --lib

# Agent tests (config builder, agent tool, etc.)
cargo test -p vol-llm-agent --lib

# Agent channel tests
cargo test -p vol-llm-agent-channel --lib

# Agent manager tests
cargo test -p vol-agent-manager --lib
```

Expected: all pass.

- [ ] **Step 4: Commit final state**

```bash
git add -A
git status
# Verify no unintended changes
git commit -m "chore: final verification — all tests pass after teamwork integration

Co-Authored-By: Claude Opus 4.7 <noreply@anthropic.com>"
```
