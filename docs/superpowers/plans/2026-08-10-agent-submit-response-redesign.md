# Agent Submit 响应合并 & 三段覆盖统一 — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Merge agent.submit's dual response into a single enriched SubmitResult, and unify tools/skills/MCPs filtering into 3-tier override (global → AgentDef → overlay) on ReActAgent.

**Architecture:** Three resolve methods on ReActAgent (`resolve_tools`, `resolve_skills`, `resolve_mcps`) each apply global → AgentDef → overlay filtering and return the same type (`Arc<ToolRegistry>`, `SkillInjector`, `Arc<McpManager>`). RunContext is simplified to hold pre-resolved objects set at run start. The handler reads from agent's public methods and returns a single SubmitResult.

**Tech Stack:** Rust (vol workspace), JSON-RPC protocol, Tokio

## Global Constraints

- No doc tests — use `#[cfg(test)]` or `tests/` integration tests
- Coverage ≥ 80% for changed crates
- Check `./scripts/check-no-doc-tests.sh` before committing
- Run `./scripts/check-agent-boundaries.sh` to verify no crate boundary violations

---

## File Map

| File | Role | Task |
|------|------|------|
| `crates/vol-llm-mcp/src/manager.rs` | Add `filter()` + `empty()`, make `ServerState` Clone | 1 |
| `crates/vol-llm-tool/src/mcp_tool.rs` | Fix `mcp__{srv}__{tool}` double underscore | 1 |
| `crates/vol-llm-skill/src/injector.rs` | Add `skill_names()` | 2 |
| `crates/vol-llm-core/src/agent_def.rs` | Add `skills: Option<Vec<String>>` field | 3 |
| `crates/vol-llm-agent/src/agent_def.rs` | Add `skills` to `AgentFrontmatter` + `AgentLoader` | 3 |
| `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs` | `ProviderInfo`, `SubmitResult` without `response`, no Ack in submit path | 4 |
| `crates/vol-llm-agent/src/react/config_builder.rs` | Remove `skill_injector_filter` capture, expose `skill_loader` | 5 |
| `crates/vol-llm-agent/src/react/agent.rs` | `AgentConfig` cleanup, `ReActAgent` new fields + resolve/public methods | 5, 6 |
| `crates/vol-llm-agent/src/react/run_context.rs` | Remove `effective_registry`/`effective_tools`/`execute_tool`/`with_capability_overlays`, remove overlay fields | 7 |
| `crates/vol-llm-agent/src/react/agent.rs` `run_input()` | Resolve at start, set on RunContext | 8 |
| `crates/vol-agent-server/src/data_plane/core.rs` | Pass `base_tools` + `skill_loader` to `ReActAgent::new` | 9 |
| `crates/vol-agent-server/src/data_plane/handlers/agent.rs` | Handler: single `SubmitResult`, read from agent | 10 |
| `crates/vol-agent-server/src/control_plane/handlers/client.rs` | Control-plane: single `SubmitResult` | 11 |
| `crates/vol-agent-server/src/data_plane/handlers/capability.rs` | Add `def.skills` validation in `update_capabilities` | 11 |
| `crates/vol-llm-ui/src/web/client.rs` | Frontend: adapt to new response shape | 12 |
| `crates/vol-llm-ui/src/connection/remote.rs` | Frontend: adapt to new response shape | 12 |
| Tests across multiple crates | Update existing tests, add new ones | Throughout |

---

### Task 1: McpManager::filter + McpTool bug fix

**Files:**
- Modify: `crates/vol-llm-mcp/src/manager.rs`
- Modify: `crates/vol-llm-tool/src/mcp_tool.rs`
- Test: `crates/vol-llm-mcp/tests/` (new or existing)

**Interfaces:**
- Produces: `McpManager::filter(server_names: Option<&[String]>) -> Self`
- Produces: `McpManager::empty() -> Self`
- Produces: `ServerState` derives `Clone`

**McpManager::filter** returns a filtered McpManager sharing the same server states but with only the specified server names. `None` = all servers, `Some([])` = no servers, `Some(["k8s"])` = only k8s.

- [ ] **Step 1: Add Clone derive to ServerState**

Open `crates/vol-llm-mcp/src/manager.rs`. Find `struct ServerState` (around line 32) and add `Clone`:

```rust
#[derive(Clone)]
struct ServerState {
    config: McpServerConfig,
    status: ServerStatus,
    retry_count: usize,
    running_service: Option<RunningService<RoleClient, ClientInfo>>,
    cancel_token: CancellationToken,
    cached_tools: Vec<McpToolInfo>,
    cached_resources: Vec<Resource>,
    cached_resource_templates: Vec<ResourceTemplate>,
    cached_prompts: Vec<Prompt>,
    reconnect_handle: Option<tokio::task::JoinHandle<()>>,
}
```

If `RunningService` or `JoinHandle` doesn't implement Clone, wrap `running_service` and `reconnect_handle` in a manual clone that sets them to `None` (filtered managers don't need live connections — they're read-only views).

- [ ] **Step 2: Write test for McpManager::filter**

Add to `crates/vol-llm-mcp/tests/` or `#[cfg(test)] mod tests` in manager.rs:

```rust
#[test]
fn test_filter_none_returns_all() {
    let configs = vec![
        McpServerConfig { name: "k8s".into(), transport: McpTransport::Http { url: "http://localhost:1".into(), headers: None, env: vec![] } },
        McpServerConfig { name: "docs-rs".into(), transport: McpTransport::Http { url: "http://localhost:2".into(), headers: None, env: vec![] } },
    ];
    let manager = McpManager::new(configs);
    let filtered = manager.filter(None);
    let names: Vec<String> = filtered.server_status().keys().cloned().collect();
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"k8s".to_string()));
    assert!(names.contains(&"docs-rs".to_string()));
}

#[test]
fn test_filter_some_returns_subset() {
    let configs = vec![
        McpServerConfig { name: "k8s".into(), transport: McpTransport::Http { url: "http://localhost:1".into(), headers: None, env: vec![] } },
        McpServerConfig { name: "docs-rs".into(), transport: McpTransport::Http { url: "http://localhost:2".into(), headers: None, env: vec![] } },
    ];
    let manager = McpManager::new(configs);
    let filtered = manager.filter(Some(&["k8s".to_string()]));
    let names: Vec<String> = filtered.server_status().keys().cloned().collect();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "k8s");
}

#[test]
fn test_filter_empty_returns_none() {
    let configs = vec![
        McpServerConfig { name: "k8s".into(), transport: McpTransport::Http { url: "http://localhost:1".into(), headers: None, env: vec![] } },
    ];
    let manager = McpManager::new(configs);
    let filtered = manager.filter(Some(&[]));
    assert!(filtered.server_status().is_empty());
}
```

- [ ] **Step 3: Run tests, verify they fail**

Run: `cargo test -p vol-llm-mcp -- filter`

Expected: compile errors — `filter` and `empty` not defined.

- [ ] **Step 4: Implement McpManager::filter and McpManager::empty**

```rust
// In impl McpManager block, add:

/// Return a filtered McpManager containing only the named servers.
/// None = all servers. Some([]) = no servers.
pub fn filter(&self, server_names: Option<&[String]>) -> Self {
    match server_names {
        None => self.clone(),
        Some(names) => {
            use std::collections::HashSet;
            let allowed: HashSet<&str> = names.iter().map(String::as_str).collect();
            let servers = self.servers.try_read()
                .unwrap_or_else(|_| panic!("McpManager servers lock poisoned"));
            let filtered: HashMap<String, ServerState> = servers
                .iter()
                .filter(|(name, _)| allowed.contains(name.as_str()))
                .map(|(name, state)| (name.clone(), state.clone()))
                .collect();
            Self {
                servers: Arc::new(tokio::sync::RwLock::new(filtered)),
                max_retries: self.max_retries,
                backoff_min: self.backoff_min,
                backoff_max: self.backoff_max,
            }
        }
    }
}

/// An McpManager with no servers.
pub fn empty() -> Self {
    Self {
        servers: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        max_retries: 0,
        backoff_min: std::time::Duration::from_secs(1),
        backoff_max: std::time::Duration::from_secs(30),
    }
}
```

- [ ] **Step 5: Run tests, verify they pass**

Run: `cargo test -p vol-llm-mcp -- filter`

Expected: PASS

- [ ] **Step 6: Fix McpTool naming — double underscore separator**

Open `crates/vol-llm-tool/src/mcp_tool.rs`, find line ~33:

```rust
// Before:
display_name: Box::leak(Box::new(format!("mcp__{sanitized}_{sanitized_tool}"))),
// After:
display_name: Box::leak(Box::new(format!("mcp__{sanitized}__{sanitized_tool}"))),
```

This makes it consistent with `filter_mcp_servers` which parses with `rest.find("__")`.

- [ ] **Step 7: Update affected tests**

Run `cargo test -p vol-llm-tool -- mcp` and fix any test that expects the old single-underscore format. The test `test_filter_mcp_servers_filters_mcp_by_name` in `registry.rs` uses dummy tools with manual names — update those to match `mcp__{server}__{tool}`.

Run: `cargo test -p vol-llm-tool -- filter_mcp`

Expected: PASS

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-mcp/src/manager.rs crates/vol-llm-tool/src/mcp_tool.rs tests/
git commit -m "feat(mcp): add McpManager::filter + McpManager::empty; fix McpTool double-underscore separator

- McpManager::filter(None) = all, filter(Some([])) = empty, filter(Some(names)) = sub-McpManager
- McpManager::empty() = no servers
- McpTool now uses mcp__{server}__{tool} (double underscore) for filter_mcp_servers compatibility

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 2: SkillInjector::skill_names

**Files:**
- Modify: `crates/vol-llm-skill/src/injector.rs`
- Test: tests in `injector.rs` (pre-existing `#[cfg(test)] mod tests`)

**Interfaces:**
- Produces: `SkillInjector::skill_names(&self) -> Vec<String>` (async, returns filtered skill names)

- [ ] **Step 1: Write the test**

Add to `#[cfg(test)] mod tests` in `injector.rs`:

```rust
#[tokio::test]
async fn test_skill_names_no_filter_returns_all() {
    let loader = SkillLoader::new_empty();
    let mut skill_a = SkillDef::new("skill-a", "# A").with_description("Skill A");
    skill_a.id = "user:skill-a".into();
    let mut skill_b = SkillDef::new("skill-b", "# B").with_description("Skill B");
    skill_b.id = "user:skill-b".into();
    loader.register(skill_a).await;
    loader.register(skill_b).await;

    let injector = SkillInjector::new(Arc::new(loader), AttentionAnchor::Head(0), None);
    let names = injector.skill_names().await;
    assert_eq!(names.len(), 2);
    assert!(names.contains(&"skill-a".to_string()));
    assert!(names.contains(&"skill-b".to_string()));
}

#[tokio::test]
async fn test_skill_names_with_filter_returns_subset() {
    let loader = SkillLoader::new_empty();
    let mut skill_a = SkillDef::new("skill-a", "# A").with_description("Skill A");
    skill_a.id = "user:skill-a".into();
    let mut skill_b = SkillDef::new("skill-b", "# B").with_description("Skill B");
    skill_b.id = "user:skill-b".into();
    loader.register(skill_a).await;
    loader.register(skill_b).await;

    let injector = SkillInjector::new(
        Arc::new(loader),
        AttentionAnchor::Head(0),
        Some(vec!["skill-a".into()]),
    );
    let names = injector.skill_names().await;
    assert_eq!(names.len(), 1);
    assert_eq!(names[0], "skill-a");
}

#[tokio::test]
async fn test_skill_names_empty_filter_returns_none() {
    let loader = SkillLoader::new_empty();
    let mut skill = SkillDef::new("test-skill", "# T").with_description("Test");
    skill.id = "user:test-skill".into();
    loader.register(skill).await;

    let injector = SkillInjector::new(
        Arc::new(loader),
        AttentionAnchor::Head(0),
        Some(vec![]),
    );
    let names = injector.skill_names().await;
    assert!(names.is_empty());
}
```

- [ ] **Step 2: Run test, verify it fails**

Run: `cargo test -p vol-llm-skill -- skill_names`

Expected: compile error — `skill_names` method not found.

- [ ] **Step 3: Implement skill_names**

```rust
impl SkillInjector {
    /// Return skill names after applying the current filter.
    pub async fn skill_names(&self) -> Vec<String> {
        let metadata = self.loader.list_metadata().await;
        let filter_guard = self.skill_filter.read().await;
        match &*filter_guard {
            Some(filter) if !filter.is_empty() => {
                let set: std::collections::HashSet<&str> =
                    filter.iter().map(String::as_str).collect();
                metadata
                    .into_iter()
                    .filter(|m| set.contains(m.name.as_str()))
                    .map(|m| m.name)
                    .collect()
            }
            _ => metadata.into_iter().map(|m| m.name).collect(),
        }
    }
}
```

- [ ] **Step 4: Run test, verify it passes**

Run: `cargo test -p vol-llm-skill -- skill_names`

Expected: PASS for all three new tests.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-skill/src/injector.rs
git commit -m "feat(skill): add SkillInjector::skill_names()

Returns skill names after applying the shared filter.
None filter = all skills, Some([]) = none, Some(names) = only those.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 3: AgentDef — add skills field

**Files:**
- Modify: `crates/vol-llm-core/src/agent_def.rs`
- Modify: `crates/vol-llm-agent/src/agent_def.rs` (AgentFrontmatter + AgentLoader)

**Interfaces:**
- Produces: `AgentDef.skills: Option<Vec<String>>`

- [ ] **Step 1: Add skills field to core AgentDef**

Open `crates/vol-llm-core/src/agent_def.rs`, add after `mcps`:

```rust
pub struct AgentDef {
    // ... existing fields ...
    pub mcps: Option<Vec<String>>,
    /// Skill allowlist. None = all skills available.
    pub skills: Option<Vec<String>>,
}
```

Update `Default` impl to include `skills: None`.

- [ ] **Step 2: Update agent crate's AgentFrontmatter**

Open `crates/vol-llm-agent/src/agent_def.rs`. Add to `AgentFrontmatter` struct:

```rust
struct AgentFrontmatter {
    // ... existing fields ...
    skills: Option<Vec<String>>,
}
```

In the loader's `discover_all()`, where the `AgentDef` is constructed from frontmatter, add:

```rust
skills: fm.skills,
```

- [ ] **Step 3: Add tests**

```rust
#[test]
fn agent_def_default_skills_is_none() {
    let def = AgentDef::default();
    assert!(def.skills.is_none());
}

#[test]
fn agent_def_with_skills_sets_field() {
    let def = AgentDef::new("test", "prompt");
    // No with_skills builder yet; test default
    assert!(def.skills.is_none());
}
```

- [ ] **Step 4: Run tests**

Run: `cargo test -p vol-llm-core -- skills`
Run: `cargo test -p vol-llm-agent -- skills`

Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-core/src/agent_def.rs crates/vol-llm-agent/src/agent_def.rs
git commit -m "feat(agent): add skills field to AgentDef

AgentDef.skills: Option<Vec<String>> — per-agent skill allowlist.
None = all skills available. Parsed from markdown frontmatter.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 4: Protocol — SubmitResult merge + ProviderInfo

**Files:**
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs`

**Interfaces:**
- Produces: `ProviderInfo { name: String, model: String }`
- Modifies: `AgentPayload::SubmitResult` — replaces `response: serde_json::Value` with `accepted: bool, provider: ProviderInfo, tools: Vec<String>, mcps: Vec<String>, skills: Vec<String>`
- Keep `SubmitAck` variant unchanged (other code paths may still use Ack kind)

- [ ] **Step 1: Add ProviderInfo struct**

In `agent_server_protocol.rs`, before `AgentPayload`:

```rust
/// Provider and model used for the run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderInfo {
    pub name: String,
    pub model: String,
}
```

- [ ] **Step 2: Modify SubmitResult variant**

Change from:
```rust
SubmitResult {
    run_id: String,
    response: serde_json::Value,
},
```
To:
```rust
SubmitResult {
    run_id: String,
    accepted: bool,
    provider: ProviderInfo,
    tools: Vec<String>,
    mcps: Vec<String>,
    skills: Vec<String>,
},
```

- [ ] **Step 3: Update SubmitResult serde round-trip test**

Find `test agent_payload_round_trip_all_variants` in the test module. Update the `SubmitResult` test case:

```rust
AgentPayload::SubmitResult {
    run_id: "r1".into(),
    accepted: true,
    provider: ProviderInfo { name: "anthropic".into(), model: "claude-sonnet-5".into() },
    tools: vec!["bash".into(), "read".into()],
    mcps: vec!["k8s".into()],
    skills: vec!["code-review".into()],
},
```

- [ ] **Step 4: Fix tests referencing old SubmitResult**

Run `cargo test -p vol-llm-agent-protocol` to find all compile errors:

```bash
cargo test -p vol-llm-agent-protocol 2>&1 | head -80
```

Expected: compile errors in tests that construct `SubmitResult { run_id, response: ... }`. Fix each:

```rust
// Before:
AgentPayload::SubmitResult { run_id: "r1".into(), response: serde_json::json!({"agents": []}) }

// After:
AgentPayload::SubmitResult {
    run_id: "r1".into(),
    accepted: true,
    provider: ProviderInfo { name: "anthropic".into(), model: "claude-sonnet-5".into() },
    tools: vec![],
    mcps: vec![],
    skills: vec![],
}
```

- [ ] **Step 5: Run full test suite for the protocol crate**

Run: `cargo test -p vol-llm-agent-protocol`

Expected: ALL PASS

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-protocol/src/agent_server_protocol.rs
git commit -m "feat(protocol): merge SubmitAck into SubmitResult; add ProviderInfo and capability fields

SubmitResult now carries run_id, accepted (from SubmitAck), provider, tools, mcps, skills.
Single JSON-RPC response replaces the old Ack+Result pair for agent.submit.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 5: AgentConfig + config_builder — remove skill_injector_filter, add skill_loader

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs` (AgentConfig)
- Modify: `crates/vol-llm-agent/src/react/config_builder.rs`

**Interfaces:**
- Removes: `AgentConfig.skill_injector_filter: Option<Arc<RwLock<Option<Vec<String>>>>>`
- Adds: `AgentConfig.skill_loader: Arc<SkillLoader>`
- Builder change: `build()` no longer captures `skill_injector_filter`

- [ ] **Step 1: Update AgentConfig — remove skill_injector_filter, add skill_loader**

In `agent.rs`, `AgentConfig` struct:
- Remove `pub skill_injector_filter: Option<Arc<tokio::sync::RwLock<Option<Vec<String>>>>>`
- Add `pub skill_loader: Arc<SkillLoader>`

Update `Default` impl: remove `skill_injector_filter: None`, add `skill_loader: Arc::new(SkillLoader::new_empty())`.

- [ ] **Step 2: Update config_builder — don't capture filter, expose skill_loader**

In `config_builder.rs`:
- Remove `skill_injector_filter` field from `AgentConfigBuilder`
- Remove `self.skill_injector_filter = Some(...)` line
- In `build()`, after creating `SkillLoader`, store it on config:

```rust
let skill_loader = Arc::new(SkillLoader::new(working_dir.clone()));
// ... (SkillTool registration unchanged) ...

// Before: self.skill_injector_filter = Some(skill_injector.skill_filter.clone());
// After: (remove; no filter capture needed)

Ok(AgentConfig {
    // ... other fields ...
    skill_loader,                          // NEW
    // skill_injector_filter: ...,         // REMOVED
})
```

- [ ] **Step 3: Fix all compile errors**

Run: `cargo check -p vol-llm-agent 2>&1 | head -80`

Fix any references to `config.skill_injector_filter` or `self.skill_injector_filter`. These will primarily be in:
- `run_context.rs` (will be handled in Task 7)
- Any tests that reference the field

- [ ] **Step 4: Run agent crate tests**

Run: `cargo test -p vol-llm-agent 2>&1`

Fix all compile errors and test failures. Some tests may reference `skill_injector_filter` — update them.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/config_builder.rs
git commit -m "refactor(agent): remove skill_injector_filter from AgentConfig; add skill_loader

skill_injector_filter was updated as a side effect in RunContext::effective_registry().
This is replaced by ReActAgent::resolve_skills() which creates a filtered SkillInjector on demand.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 6: ReActAgent — new fields + resolve methods + public methods

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs`

**Interfaces:**
- Modifies: `ReActAgent::new(config, base_tools, skill_loader)`
- Produces: `ReActAgent::llm() -> &Arc<dyn LLMClient>`
- Produces: `ReActAgent::tools() -> Arc<ToolRegistry>`
- Produces: `ReActAgent::skills() -> SkillInjector`
- Produces: `ReActAgent::mcps() -> Arc<McpManager>`
- Produces: `ReActAgent::resolve_tools(sid: &str) -> Arc<ToolRegistry>`
- Produces: `ReActAgent::resolve_skills(sid: &str) -> SkillInjector`
- Produces: `ReActAgent::resolve_mcps(sid: &str) -> Arc<McpManager>`

- [ ] **Step 1: Add fields to ReActAgent struct**

```rust
pub struct ReActAgent {
    config: Arc<AgentConfig>,
    base_tools: Arc<ToolRegistry>,
    skill_loader: Arc<SkillLoader>,
    run_state: Arc<RunningState>,
}
```

- [ ] **Step 2: Update ReActAgent::new signature**

```rust
pub fn new(
    config: AgentConfig,
    base_tools: Arc<ToolRegistry>,
    skill_loader: Arc<SkillLoader>,
) -> Self {
    Self {
        config: Arc::new(config),
        base_tools,
        skill_loader,
        run_state: Arc::new(RunningState::new()),
    }
}
```

- [ ] **Step 3: Write tests for resolve methods**

Add to `#[cfg(test)] mod tests` in agent.rs (or a new test file):

```rust
#[tokio::test]
async fn test_resolve_tools_no_def_no_overlay_returns_all() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("bash"));
    registry.register(DummyTool::new("read"));
    let base_tools = Arc::new(registry);

    let config = AgentConfig::builder()
        .with_llm(Arc::new(DummyLlm))
        .with_tools(base_tools.clone())
        .build()
        .unwrap();
    let skills = Arc::new(SkillLoader::new_empty());
    let agent = ReActAgent::new(config, base_tools, skills);

    let tools = agent.resolve_tools("any-session");
    let names = tools.definitions().iter().map(|d| d.name.clone()).collect::<Vec<_>>();
    assert!(names.contains(&"bash".to_string()));
    assert!(names.contains(&"read".to_string()));
}

#[tokio::test]
async fn test_resolve_tools_with_def_allowlist() {
    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("bash"));
    registry.register(DummyTool::new("read"));
    let base_tools = Arc::new(registry);

    let def = AgentDef::new("test", "prompt")
        .with_tools(vec!["bash".into()]);

    let config = AgentConfig::builder()
        .with_llm(Arc::new(DummyLlm))
        .with_tools(base_tools.clone())
        .with_def(def)
        .build()
        .unwrap();
    let skills = Arc::new(SkillLoader::new_empty());
    let agent = ReActAgent::new(config, base_tools, skills);

    let tools = agent.resolve_tools("any-session");
    let names: Vec<String> = tools.definitions().iter().map(|d| d.name.clone()).collect();
    assert_eq!(names, vec!["bash"]);
}

#[tokio::test]
async fn test_resolve_skills_no_def_no_overlay_returns_all() {
    let loader = SkillLoader::new_empty();
    let mut skill = SkillDef::new("test-skill", "# T").with_description("Test");
    skill.id = "user:test-skill".into();
    loader.register(skill).await;
    let skill_loader = Arc::new(loader);

    let mut registry = ToolRegistry::new();
    registry.register(DummyTool::new("bash"));
    let base_tools = Arc::new(registry);

    let config = AgentConfig::builder()
        .with_llm(Arc::new(DummyLlm))
        .with_tools(base_tools.clone())
        .build()
        .unwrap();
    let agent = ReActAgent::new(config, base_tools, skill_loader);

    let injector = agent.resolve_skills("any-session");
    let names = injector.skill_names().await;
    assert!(names.contains(&"test-skill".to_string()));
}
```

- [ ] **Step 4: Run tests, verify they fail**

Run: `cargo test -p vol-llm-agent -- resolve`

Expected: compile errors — resolve methods not defined.

- [ ] **Step 5: Implement resolve methods + public methods**

```rust
impl ReActAgent {
    // ── Public query methods ──

    pub fn llm(&self) -> &Arc<dyn LLMClient> {
        &self.config.llm
    }

    pub fn tools(&self) -> Arc<ToolRegistry> {
        self.resolve_tools(&self.current_session_id())
    }

    pub fn skills(&self) -> SkillInjector {
        self.resolve_skills(&self.current_session_id())
    }

    pub fn mcps(&self) -> Arc<McpManager> {
        self.resolve_mcps(&self.current_session_id())
    }

    // ── Internal resolve methods ──

    fn resolve_tools(&self, sid: &str) -> Arc<ToolRegistry> {
        let overlay = self.get_overlay(sid);

        // Tool allowlist: overlay non-empty > def.tools
        let allowed = overlay.as_ref()
            .and_then(|o| if o.effective_tools.is_empty() { None }
                        else { Some(o.effective_tools.as_slice()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.tools.as_deref()));

        // Blocklist: always from def
        let disallowed = self.config.def.as_ref()
            .and_then(|d| d.disallowed_tools.as_deref());

        let mut filtered = self.base_tools.filter(allowed, disallowed);

        // MCP tool filter: overlay non-empty > def.mcps
        let mcps = overlay.as_ref()
            .and_then(|o| if o.effective_mcp_servers.is_empty() { None }
                        else { Some(o.effective_mcp_servers.as_slice()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.mcps.as_deref()));
        if let Some(mcp_names) = mcps {
            filtered = Arc::new(filtered.filter_mcp_servers(mcp_names));
        }

        filtered
    }

    fn resolve_skills(&self, sid: &str) -> SkillInjector {
        let overlay = self.get_overlay(sid);

        let filter: Option<Vec<String>> = overlay.as_ref()
            .and_then(|o| if o.effective_skills.is_empty() { None }
                        else { Some(o.effective_skills.clone()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.skills.clone()));

        SkillInjector::new(
            self.skill_loader.clone(),
            AttentionAnchor::Head(1),
            filter,
        )
    }

    fn resolve_mcps(&self, sid: &str) -> Arc<McpManager> {
        let overlay = self.get_overlay(sid);

        let filter: Option<&[String]> = overlay.as_ref()
            .and_then(|o| if o.effective_mcp_servers.is_empty() { None }
                        else { Some(o.effective_mcp_servers.as_slice()) })
            .or_else(|| self.config.def.as_ref().and_then(|d| d.mcps.as_deref()));

        self.config.mcp_manager.as_ref()
            .map(|m| Arc::new(m.filter(filter)))
            .unwrap_or_else(|| Arc::new(McpManager::empty()))
    }

    fn get_overlay(&self, sid: &str) -> Option<CapabilityOverlay> {
        self.config.capability_overlays.as_ref()
            .and_then(|map| map.try_read().ok())
            .and_then(|guard| guard.get(&(self.config.agent_id.clone(), sid.to_string())).cloned())
    }

    fn current_session_id(&self) -> String {
        self.config.session.read().unwrap().id.clone()
    }
}
```

Add imports at top of file:

```rust
use vol_llm_core::capability_overlay::CapabilityOverlay;
use vol_llm_skill::{SkillInjector, SkillLoader};
use vol_llm_mcp::McpManager;
use vol_llm_context::AttentionAnchor;
```

- [ ] **Step 6: Fix all callers of ReActAgent::new**

Run: `cargo check -p vol-llm-agent -p vol-agent-server -p vol-llm-agents 2>&1`

Fix every `ReActAgent::new(config)` call to `ReActAgent::new(config, base_tools, skill_loader)`.

In `vol-agent-server/src/data_plane/core.rs` (line 256):
```rust
let base_tools = self.tool_registry.clone();
let skill_loader = self.skill_loader.clone();
let agent = ReActAgent::new(config, base_tools, skill_loader);
```

In `vol-llm-agents` examples/tests:
```rust
let base_tools = Arc::new(tool_registry);
let skill_loader = Arc::new(SkillLoader::new_empty());
let agent = ReActAgent::new(config, base_tools, skill_loader);
```

- [ ] **Step 7: Run tests**

Run: `cargo test -p vol-llm-agent -- resolve`

Expected: PASS for new tests.

- [ ] **Step 8: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs
git commit -m "feat(agent): add ReActAgent resolve methods for 3-tier tools/skills/mcps override

New fields: base_tools, skill_loader.
Public: llm(), tools(), skills(), mcps().
Internal: resolve_tools(sid), resolve_skills(sid), resolve_mcps(sid).
All three follow: overlay non-empty > AgentDef > global.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 7: RunContext — simplify

**Files:**
- Modify: `crates/vol-llm-agent/src/react/run_context.rs`

**Changes:**
- Remove `effective_registry()`, `effective_tools()`, `execute_tool()` methods
- Remove `capability_overlays`, `agent_id`, `current_overlay_version` fields
- Remove `with_capability_overlays()` method
- Keep `tools: Arc<ToolRegistry>` field — now pre-resolved
- `execute()` call on `tools` stays but simplified — no `effective_registry()` wrapper

- [ ] **Step 1: Remove fields from RunContext**

Delete lines:
```rust
pub capability_overlays: Option<Arc<tokio::sync::RwLock<HashMap<(String, String), CapabilityOverlay>>>>,
pub agent_id: String,
pub(crate) current_overlay_version: Arc<AtomicU64>,
```

From `Clone` impl, remove the corresponding clone lines.

From `new()`, remove the default initialization lines.

- [ ] **Step 2: Remove with_capability_overlays method**

Delete the entire `with_capability_overlays()` method (lines 178-193).

- [ ] **Step 3: Simplify effective_registry → remove**

Delete `effective_registry()` entirely (lines 358-437). Delete `effective_tools()` (lines 333-335).

- [ ] **Step 4: Update execute_tool**

```rust
// Before:
pub async fn execute_tool(&self, call: &ToolCall, ctx: &ToolContext) -> Result<ToolResult, String> {
    self.effective_registry().execute(call, ctx).await...
}

// After:
pub async fn execute_tool(&self, call: &ToolCall, ctx: &ToolContext) -> Result<ToolResult, String> {
    self.tools.execute(call, ctx).await.map_err(|e| ...)
}
```

- [ ] **Step 5: Remove any remaining overlay imports**

Remove `use vol_llm_core::capability_overlay::CapabilityOverlay;` if no other usage.

- [ ] **Step 6: Fix compile errors & run tests**

Run: `cargo check -p vol-llm-agent 2>&1 | head -40`

Fix any remaining references. Then run tests:

```bash
cargo test -p vol-llm-agent -- run_context 2>&1
```

Fix test failures — tests that reference `capability_overlays`/`effective_tools`/`with_capability_overlays` need to be updated or removed.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/run_context.rs
git commit -m "refactor(agent): simplify RunContext — remove effective_registry and overlay fields

RunContext no longer does 3-tier filtering. Resolved tools/skills/mcps are set at run start
by ReActAgent::run_input(). Removes the skill_injector_filter side effect entirely.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 8: run_input — resolve at start

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs` (run_input method)

**Changes:**
- Remove `with_capability_overlays` call
- Add resolve calls after RunContext creation
- Use `self.resolve_tools()` instead of `run_ctx.effective_tools()`
- Use `self.resolve_skills()` to replace skills contributor

- [ ] **Step 1: Update run_input to resolve at start**

In `run_input()`, after `RunContext::new()`:

```rust
// Remove:
// if let Some(ref overlays) = self.config.capability_overlays {
//     run_ctx = run_ctx.with_capability_overlays(overlays.clone(), self.config.agent_id.clone());
// }

// Add:
let sid = run_ctx.session_id.clone();
let resolved_tools = self.resolve_tools(&sid);
let resolved_skills = self.resolve_skills(&sid);

// Set pre-resolved tools on run context
run_ctx.tools = resolved_tools;
```

- [ ] **Step 2: Replace effective_tools() calls in agent loop**

In the agent loop (around line 443):

```rust
// Before: let tools_defs = run_ctx.effective_tools();
// After:  let tools_defs = run_ctx.tools.definitions();
```

In execute_tool call (around line 601):

```rust
// Before: run_ctx.execute_tool(call, &tool_ctx)
// After:  run_ctx.tools.execute(call, &tool_ctx).await.map_err(...)
```

Wait — `execute_tool()` is still defined on RunContext. Let's keep it but simplify its implementation:

```rust
// run_context.rs
pub async fn execute_tool(&self, call: &ToolCall, ctx: &ToolContext) -> Result<ToolResult, String> {
    self.tools.execute(call, ctx).await
        .map_err(|e| format!("Tool execution failed: {e}"))
}
```

- [ ] **Step 3: Replace skills contributor in context builder**

After RunContext is created, replace the "skills" contributor with the resolved SkillInjector:

```rust
// In run_input, after context builder setup:
run_ctx.replace_contributor("skills", Box::new(resolved_skills));
```

If `replace_contributor` doesn't exist, add it to RunContext:

```rust
impl RunContext {
    pub fn replace_contributor(&self, name: &str, contributor: Box<dyn ContextContributor>) {
        self.config.context_builder.write().unwrap()
            .replace_contributor(name, contributor);
    }
}
```

If ContextBuilder doesn't have `replace_contributor`, add the method:

```rust
impl ContextBuilder {
    pub fn replace_contributor(&mut self, name: &str, contributor: Box<dyn ContextContributor>) {
        // Find and replace by name
        for c in &mut self.contributors {
            if c.name() == name {
                *c = contributor;
                return;
            }
        }
        // Not found — add
        self.contributors.push(contributor);
    }
}
```

- [ ] **Step 4: Run agent crate tests + fix failures**

Run: `cargo test -p vol-llm-agent 2>&1`

Fix any test that references the old flow. Tests creating RunContext with capability_overlays need updating.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/run_context.rs
git commit -m "feat(agent): resolve tools/skills/mcps at run start in run_input()

Replaces RunContext::effective_registry() with pre-resolved Arc<ToolRegistry>,
SkillInjector. Agent loop uses run_ctx.tools.definitions() directly.
Removes skill_injector_filter side effect.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 9: core.rs — update agent construction

**Files:**
- Modify: `crates/vol-agent-server/src/data_plane/core.rs`

**Changes:**
- Remove pre-filtering of tool registry (was: `filter_mcp_servers` + `filter`)
- Pass `base_tools` and `skill_loader` to `ReActAgent::new`

- [ ] **Step 1: Simplify register_agent**

In `register_agent()`, remove the tool pre-filtering block (lines ~198-211):

```rust
// REMOVE:
// if let Some(ref server_names) = def.mcps {
//     tool_registry = tool_registry.filter_mcp_servers(server_names);
// }
// let allowed_refs = ...;
// let disallowed_refs = ...;
// let tools = tool_registry.filter(allowed_refs, disallowed_refs);

// INSTEAD: Use full registry, filtering moves to ReActAgent.resolve_tools()
```

Change the config builder to use full registry:

```rust
let config = AgentConfig::builder()
    .with_def(def.clone())
    .with_llm(llm)
    .with_tools(self.tool_registry.clone())  // full, unfiltered
    .with_session(session)
    .with_sandbox_registry(self.sandbox_registry.clone())
    .with_working_dir(agent_dir.clone())
    .build()
    .expect("AgentConfig build failed");
```

- [ ] **Step 2: Update ReActAgent::new call**

```rust
// Before:
let agent = ReActAgent::new(config);

// After:
let base_tools = self.tool_registry.clone();
let skill_loader = self.skill_loader.clone();
let agent = ReActAgent::new(config, base_tools, skill_loader);
```

- [ ] **Step 3: Ensure skill_loader is accessible from DataPlaneServerCore**

`DataPlaneServerCore` already has `self.runtime.skill_loader` (from `AgentRuntime`). Add a direct accessor if needed:

```rust
// Already exists: self.runtime.skill_loader
// In register_agent: let skill_loader = self.runtime.skill_loader.clone();
```

- [ ] **Step 4: Run server crate tests**

Run: `cargo test -p vol-agent-server 2>&1`

Fix compile errors. Tests that construct agents need the new args.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/core.rs
git commit -m "refactor(server): simplify agent construction — no tool pre-filtering

Tool/skill/MCP filtering is now done by ReActAgent.resolve_*() methods.
core.rs passes full unfiltered registries to ReActAgent::new.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 10: Data-plane handler — single SubmitResult

**Files:**
- Modify: `crates/vol-agent-server/src/data_plane/handlers/agent.rs`

**Changes:**
- Return single `SubmitResult` instead of `SubmitAck` + `SubmitResult` vec
- Read provider/tools/skills/mcps from agent
- Convert to protocol types

- [ ] **Step 1: Update the Submit handler**

Replace lines 76-152 (the Submit match arm):

```rust
(AgentOperation::Submit, Payload::Agent(AgentPayload::Submit { input, target })) => {
    let target_id = {
        let holders = self.holders.lock().unwrap();
        target
            .filter(|t| holders.contains_key(t))
            .or_else(|| holders.keys().next().cloned())
            .unwrap_or_else(|| "agent".to_string())
    };

    // ... session handling unchanged ...

    let run_id = input.run_id.clone()
        .unwrap_or_else(|| uuid::Uuid::new_v4().simple().to_string());
    let run_id_clone = run_id.clone();
    let request = AgentRequest::new(&target_id, input);

    // Read resolved config from agent instance
    let agent = self.router.get_agent(&target_id).await;
    let (provider, tools, mcps, skills) = match agent {
        Some(ref a) => {
            let tools_obj = a.tools();
            let skills_obj = a.skills();
            let mcps_obj = a.mcps();
            let llm = a.llm();
            (
                ProviderInfo {
                    name: llm.provider().to_string(),
                    model: llm.model().to_string(),
                },
                tools_obj.definitions().iter().map(|d| d.name.clone()).collect(),
                mcps_obj.server_status().keys().cloned().collect(),
                skills_obj.skill_names().await,
            )
        }
        None => (
            ProviderInfo { name: "unknown".into(), model: "unknown".into() },
            vec![],
            vec![],
            vec![],
        ),
    };

    match self.router.send(&target_id, request).await {
        Ok(rx) => {
            let router = self.router.clone();
            tokio::spawn(async move {
                Self::process_run_result(rx, &run_id_clone, &router).await;
            });

            // Single response — no more Ack + Result pair
            Ok(vec![AgentServerMessage::new_result(
                message.message_id,
                Operation::Agent(AgentOperation::Submit),
                Payload::Agent(AgentPayload::SubmitResult {
                    run_id,
                    accepted: true,
                    provider,
                    tools,
                    mcps,
                    skills,
                }),
            )])
        }
        Err(e) => Ok(vec![AgentServerMessage::new_error(
            message.message_id,
            Operation::Agent(AgentOperation::Submit),
            vol_llm_agent_protocol::agent_server_protocol::ErrorPayload {
                code: "agent_submit_failed".to_string(),
                message: e.to_string(),
                detail: None,
                terminal: true,
            },
        )]),
    }
}
```

- [ ] **Step 2: Fix imports in agent.rs handler**

Add:
```rust
use vol_llm_agent_protocol::agent_server_protocol::ProviderInfo;
```

- [ ] **Step 3: Run tests, fix compile errors**

Run: `cargo check -p vol-agent-server 2>&1`

- [ ] **Step 4: Update server integration tests**

Run: `cargo test -p vol-agent-server 2>&1`

Fix tests that expect dual Ack+Result responses. Update `agent_run_client.rs` and `jsonrpc_e2e_test.rs`:

```rust
// Before: expecting two messages (Ack then Result)
// After: expecting single SubmitResult with accepted + provider + tools/mcps/skills
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/handlers/agent.rs
git commit -m "feat(server): single SubmitResult response for agent.submit

Handler reads provider/tools/skills/mcps from agent instance and returns one
enriched SubmitResult. Replaces the old Ack+Result pattern.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 11: Control-plane handler + CapabilityHandler validation

**Files:**
- Modify: `crates/vol-agent-server/src/control_plane/handlers/client.rs`
- Modify: `crates/vol-agent-server/src/data_plane/handlers/capability.rs`

**Changes:**
- Control-plane: return single SubmitResult (matching data-plane handler)
- CapabilityHandler: validate skills against `def.skills` allowlist

- [ ] **Step 1: Update control-plane client handler**

In `client.rs`, the `agent.submit` handler (around line 86-149) currently returns a `SubmitAck` wrapped as Result. Update to return a `SubmitResult`. Since the control-plane doesn't have a direct agent instance, return a minimal config:

```rust
// Return minimal SubmitResult (control-plane proxies, doesn't have agent instance)
Ok(vec![AgentServerMessage::new_result(
    message.message_id,
    Operation::Agent(AgentOperation::Submit),
    Payload::Agent(AgentPayload::SubmitResult {
        run_id: run_id.clone(),
        accepted: true,
        provider: ProviderInfo { name: "unknown".into(), model: "unknown".into() },
        tools: vec![],
        mcps: vec![],
        skills: vec![],
    }),
)])
```

- [ ] **Step 2: Add skills validation to CapabilityHandler**

In `capability.rs`, find the `update_capabilities` validation. Add after existing tool/MCP validation:

```rust
// Validate skills against AgentDef allowlist (if set)
if let Some(ref def_skills) = def.skills {
    for skill_name in &effective_skills {
        if !def_skills.contains(skill_name) {
            return Err(...); // skill_not_allowed
        }
    }
}
```

Also update `resolve_effective()` to include `def.skills` in the fallback path (currently skills always returns `vec![]` from AgentDef since it had no skills field).

- [ ] **Step 3: Run tests**

Run: `cargo test -p vol-agent-server 2>&1`

- [ ] **Step 4: Commit**

```bash
git add crates/vol-agent-server/src/control_plane/handlers/client.rs \
        crates/vol-agent-server/src/data_plane/handlers/capability.rs
git commit -m "feat(server): update control-plane handler + CapabilityHandler skills validation

Control-plane agent.submit returns single SubmitResult.
CapabilityHandler validates skills against AgentDef.skills allowlist.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 12: Frontend adaptation

**Files:**
- Modify: `crates/vol-llm-ui/src/web/client.rs`
- Modify: `crates/vol-llm-ui/src/connection/remote.rs`

**Changes:**
- Adapt submit response parsing to new `SubmitResult` shape
- SubmitResult now has `provider`, `tools`, `mcps`, `skills` instead of `response`

- [ ] **Step 1: Update web client**

In `crates/vol-llm-ui/src/web/client.rs`, find where `agent.submit` response is parsed. Update to handle new fields:

```rust
// Before: response contained response.run_id, response.response
// After: response contains response.run_id, response.accepted,
//        response.provider, response.tools, response.mcps, response.skills
```

- [ ] **Step 2: Update remote connection**

In `crates/vol-llm-ui/src/connection/remote.rs`, same adaptation.

- [ ] **Step 3: Run frontend dev server and verify**

```bash
make web-dev  # starts on :5173
```

Submit a message and verify the response is handled correctly.

- [ ] **Step 4: Commit**

```bash
git add crates/vol-llm-ui/src/web/client.rs crates/vol-llm-ui/src/connection/remote.rs
git commit -m "feat(frontend): adapt to new agent.submit SubmitResult shape

SubmitResult now includes provider, tools, mcps, skills instead of response.
Removes dual-message Ack+Result handling.

Co-Authored-By: Claude <noreply@anthropic.com>"
```

---

### Task 13: Integration tests + verification

**Files:**
- Modify: `crates/vol-agent-server/tests/agent_run_client.rs`
- Modify: `crates/vol-agent-server/tests/jsonrpc_e2e_test.rs`
- Modify: `crates/vol-llm-agent-protocol/tests/agent_server_protocol_codec_test.rs`

- [ ] **Step 1: Run full test suite**

```bash
cargo test -p vol-llm-agent-protocol -p vol-llm-agent -p vol-agent-server 2>&1
```

Fix all remaining failures.

- [ ] **Step 2: Run boundary check**

```bash
./scripts/check-agent-boundaries.sh
```

Verify no crate boundary violations.

- [ ] **Step 3: Run no-doc-test check**

```bash
./scripts/check-no-doc-tests.sh
```

- [ ] **Step 4: Coverage check**

```bash
make coverage-threshold PKG=vol-llm-agent PCT=80
make coverage-threshold PKG=vol-llm-agent-protocol PCT=80
make coverage-threshold PKG=vol-agent-server PCT=80
```

Add any missing test coverage.

- [ ] **Step 5: Full build**

```bash
cargo build -p vol-agent-server --release
```

- [ ] **Step 6: Smoke test**

```bash
./scripts/smoke-test.sh --all
```

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "test: update integration tests for agent.submit redesign

Fix all tests to match new single SubmitResult response format.
All crates pass boundary check, doc-test check, and 80% coverage.

Co-Authored-By: Claude <noreply@anthropic.com>"
```
