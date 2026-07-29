# Dynamic Capability Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Allow users to dynamically adjust which tools, skills, and MCP servers are active for an agent mid-conversation via the web UI, without restarting the agent.

**Architecture:** A per-session `CapabilityOverlay` (versioned replacement list) stored in-memory in `AgentRuntime`. The frontend sends `agent.get_capabilities` / `agent.update_capabilities` JSON-RPC calls handled by a new `CapabilityHandler`. The ReAct agent loop checks the overlay version before each LLM call and rebuilds its filtered tool registry + skill injector when it changes.

**Tech Stack:** Rust (tokio, serde, async-trait), Dioxus WASM frontend, JSON-RPC 2.0 over WebSocket.

---

### Task 1: Protocol — add operations and payloads

**Files:**
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs:114-125` (AgentOperation)
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs:52-111` (Operation::method_name)
- Modify: `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs:899-973` (AgentPayload)

- [ ] **Step 1: Add `GetCapabilities` and `UpdateCapabilities` to `AgentOperation`**

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentOperation {
    Submit,
    Cancel,
    Subscribe,
    Unsubscribe,
    Approve,
    List,
    Event,
    Status,
    ContextConfig,
    ContextSnapshot,
    GetCapabilities,      // NEW
    UpdateCapabilities,   // NEW
}
```

- [ ] **Step 2: Add method name mappings in `Operation::method_name()`**

In the `AgentOperation` match arm within `method_name()`:

```rust
Operation::Agent(AgentOperation::GetCapabilities) => "agent.get_capabilities",
Operation::Agent(AgentOperation::UpdateCapabilities) => "agent.update_capabilities",
```

- [ ] **Step 3: Add `CapabilitiesPayload` variants to `AgentPayload`**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentPayload {
    // ... existing variants remain ...

    // NEW variants:
    GetCapabilities {
        agent_id: String,
        session_id: String,
    },
    GetCapabilitiesResult {
        effective_tools: Vec<String>,
        effective_skills: Vec<String>,
        effective_mcp_servers: Vec<String>,
        available_tools: Vec<serde_json::Value>,
        available_skills: Vec<serde_json::Value>,
        available_mcp_servers: Vec<serde_json::Value>,
        base_tools: Vec<String>,
        base_skills: Vec<String>,
        base_mcp_servers: Vec<String>,
    },
    UpdateCapabilities {
        agent_id: String,
        session_id: String,
        effective_tools: Vec<String>,
        effective_skills: Vec<String>,
        effective_mcp_servers: Vec<String>,
    },
    UpdateCapabilitiesResult {
        effective_tools: Vec<String>,
        effective_skills: Vec<String>,
        effective_mcp_servers: Vec<String>,
    },
}
```

The `base_*` fields in `GetCapabilitiesResult` carry the AgentDef defaults, so the frontend can show "Reset to default" correctly.

- [ ] **Step 4: Add payload decode arms in `Payload::data_json()`**

In the `data_json()` method, add arms for the new payloads to avoid "unsupported payload" panics:

```rust
Payload::Agent(AgentPayload::GetCapabilities { .. }) => {
    serde_json::to_value(&p).unwrap_or_default()
}
Payload::Agent(AgentPayload::GetCapabilitiesResult { .. }) => {
    serde_json::to_value(&p).unwrap_or_default()
}
Payload::Agent(AgentPayload::UpdateCapabilities { .. }) => {
    serde_json::to_value(&p).unwrap_or_default()
}
Payload::Agent(AgentPayload::UpdateCapabilitiesResult { .. }) => {
    serde_json::to_value(&p).unwrap_or_default()
}
```

- [ ] **Step 5: Build check**

Run: `cargo check -p vol-llm-agent-protocol`
Expected: compiles successfully.

- [ ] **Step 6: Commit**

```bash
git add crates/vol-llm-agent-protocol/src/agent_server_protocol.rs
git commit -m "feat(protocol): add GetCapabilities/UpdateCapabilities operations and payloads"
```

---

### Task 2: CapabilityOverlay — data structure and AgentRuntime integration

**Files:**
- Modify: `crates/vol-llm-runtime/src/lib.rs:62-74` (AgentRuntime struct)
- Create: `crates/vol-llm-runtime/src/capability_overlay.rs` (new module)

- [ ] **Step 1: Create `capability_overlay.rs` module**

Create `crates/vol-llm-runtime/src/capability_overlay.rs`:

```rust
/// Per-session capability adjustment, keyed by (agent_id, session_id).
/// Lives in AgentRuntime, purely in-memory. Survives frontend refresh,
/// dies on server restart.
#[derive(Debug, Clone)]
pub struct CapabilityOverlay {
    pub version: u64,
    pub effective_tools: Vec<String>,
    pub effective_skills: Vec<String>,
    pub effective_mcp_servers: Vec<String>,
}

impl CapabilityOverlay {
    pub fn new(
        tools: Vec<String>,
        skills: Vec<String>,
        mcp_servers: Vec<String>,
    ) -> Self {
        Self {
            version: 1,
            effective_tools: tools,
            effective_skills: skills,
            effective_mcp_servers: mcp_servers,
        }
    }

    /// Update overlay and bump version.
    pub fn update(
        &mut self,
        tools: Vec<String>,
        skills: Vec<String>,
        mcp_servers: Vec<String>,
    ) {
        self.effective_tools = tools;
        self.effective_skills = skills;
        self.effective_mcp_servers = mcp_servers;
        self.version += 1;
    }

    /// Check if the overlay matches the current state (no-op update).
    pub fn matches(
        &self,
        tools: &[String],
        skills: &[String],
        mcp_servers: &[String],
    ) -> bool {
        self.effective_tools == tools
            && self.effective_skills == skills
            && self.effective_mcp_servers == mcp_servers
    }
}

/// Convenience alias for the overlay map type.
pub type OverlayMap = std::collections::HashMap<(String, String), CapabilityOverlay>;
```

- [ ] **Step 2: Add module declaration and field to `AgentRuntime`**

In `crates/vol-llm-runtime/src/lib.rs`:

Add after existing `use` statements:
```rust
mod capability_overlay;
pub use capability_overlay::CapabilityOverlay;
```

Add field to `AgentRuntime` struct:
```rust
pub struct AgentRuntime {
    working_dir: PathBuf,
    store_dir: PathBuf,
    pub llm_registry: ProviderLoader,
    pub tool_registry: Arc<ToolRegistry>,
    pub task_store: Arc<dyn TaskStore>,
    pub session_manager: Arc<dyn SessionManager>,
    pub mcp_manager: Arc<McpManager>,
    pub sandbox_registry: Arc<vol_llm_sandbox::registry::SandboxRegistry>,
    pub skill_loader: Arc<SkillLoader>,
    pub agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
    pub agent_status: Arc<std::sync::RwLock<HashMap<String, AgentStatus>>>,
    // NEW:
    pub capability_overlays: Arc<tokio::sync::RwLock<HashMap<(String, String), CapabilityOverlay>>>,
}
```

In `AgentRuntimeBuilder::build()`, initialize the new field right before `Ok(AgentRuntime { ... })`:

```rust
let capability_overlays = Arc::new(tokio::sync::RwLock::new(HashMap::new()));
```

Add it to the struct literal:
```rust
Ok(AgentRuntime {
    working_dir: self.working_dir,
    store_dir,
    llm_registry,
    tool_registry,
    task_store,
    session_manager,
    mcp_manager,
    sandbox_registry,
    skill_loader,
    agent_defs: Arc::new(std::sync::RwLock::new(HashMap::new())),
    agent_status: Arc::new(std::sync::RwLock::new(HashMap::new())),
    capability_overlays,  // NEW
})
```

- [ ] **Step 3: Expose capability_overlays in DataPlaneServerCore**

In `crates/vol-agent-server/src/data_plane/core.rs`:

Extract the overlay map after building runtime (alongside other extractions, around line 431-437):

```rust
let capability_overlays = runtime.capability_overlays.clone();
```

Store it in `DataPlaneServerCore`:

```rust
pub struct DataPlaneServerCore {
    // ... existing fields ...
    capability_overlays: Arc<tokio::sync::RwLock<HashMap<(String, String), CapabilityOverlay>>>,
}
```

Add accessor:
```rust
pub fn capability_overlays(&self) -> &Arc<tokio::sync::RwLock<HashMap<(String, String), CapabilityOverlay>>> {
    &self.capability_overlays
}
```

Update the struct literal in `build()` to include `capability_overlays`.

- [ ] **Step 4: Build check**

Run: `cargo check -p vol-llm-runtime -p vol-agent-server`
Expected: compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-runtime/src/capability_overlay.rs crates/vol-llm-runtime/src/lib.rs crates/vol-agent-server/src/data_plane/core.rs
git commit -m "feat(runtime): add CapabilityOverlay data model and AgentRuntime integration"
```

---

### Task 3: CapabilityHandler — JSON-RPC handler

**Files:**
- Create: `crates/vol-agent-server/src/data_plane/handlers/capability.rs`

- [ ] **Step 1: Write the handler implementation**

Create `crates/vol-agent-server/src/data_plane/handlers/capability.rs`:

```rust
use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;
use vol_llm_runtime::CapabilityOverlay;

use vol_llm_agent_protocol::agent_server_protocol::{
    AgentOperation, AgentPayload, AgentServerMessage, ErrorPayload,
    Operation, Payload, ProtocolError,
};
use vol_llm_agent_protocol::DomainHandler;
use vol_llm_core::agent_def::AgentDef;
use vol_llm_mcp::McpManager;
use vol_llm_skill::SkillLoader;
use vol_llm_tool::ToolRegistry;

/// Handler for agent.get_capabilities and agent.update_capabilities.
pub struct CapabilityHandler {
    overlays: Arc<RwLock<HashMap<(String, String), CapabilityOverlay>>>,
    tool_registry: Arc<ToolRegistry>,
    skill_loader: Arc<SkillLoader>,
    mcp_manager: Arc<McpManager>,
    agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
}

impl CapabilityHandler {
    pub fn new(
        overlays: Arc<RwLock<HashMap<(String, String), CapabilityOverlay>>>,
        tool_registry: Arc<ToolRegistry>,
        skill_loader: Arc<SkillLoader>,
        mcp_manager: Arc<McpManager>,
        agent_defs: Arc<std::sync::RwLock<HashMap<String, AgentDef>>>,
    ) -> Self {
        Self {
            overlays,
            tool_registry,
            skill_loader,
            mcp_manager,
            agent_defs,
        }
    }

    /// Build the effective capability list for an agent.
    /// Falls back to AgentDef defaults if no overlay exists.
    fn resolve_effective(
        &self,
        agent_id: &str,
        session_id: &str,
        overlays: &HashMap<(String, String), CapabilityOverlay>,
    ) -> (Vec<String>, Vec<String>, Vec<String>) {
        if let Some(overlay) = overlays.get(&(agent_id.to_string(), session_id.to_string())) {
            return (
                overlay.effective_tools.clone(),
                overlay.effective_skills.clone(),
                overlay.effective_mcp_servers.clone(),
            );
        }
        // Fall back to AgentDef base config
        let def = self
            .agent_defs
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(agent_id)
            .cloned();
        match def {
            Some(d) => (
                d.tools.unwrap_or_default(),
                d.skills.unwrap_or_default(),
                d.mcps.unwrap_or_default(),
            ),
            None => (vec![], vec![], vec![]),
        }
    }

    /// Gather available pool lists from the registries.
    async fn gather_available(&self) -> AvailableLists {
        let tools: Vec<serde_json::Value> = self
            .tool_registry
            .tool_names()
            .iter()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                })
            })
            .collect();

        let skills: Vec<serde_json::Value> = self
            .skill_loader
            .list_metadata()
            .await
            .iter()
            .map(|m| {
                serde_json::json!({
                    "id": m.id,
                    "name": m.name,
                    "version": m.version,
                    "scope": m.scope.to_string(),
                    "description": m.description,
                })
            })
            .collect();

        let mcp_servers: Vec<serde_json::Value> = self
            .mcp_manager
            .server_status()
            .keys()
            .map(|name| {
                serde_json::json!({
                    "name": name,
                })
            })
            .collect();

        AvailableLists { tools, skills, mcp_servers }
    }
}

struct AvailableLists {
    tools: Vec<serde_json::Value>,
    skills: Vec<serde_json::Value>,
    mcp_servers: Vec<serde_json::Value>,
}

#[async_trait]
impl DomainHandler for CapabilityHandler {
    fn name(&self) -> &str {
        "capability"
    }

    fn operations(&self) -> Vec<Operation> {
        vec![
            Operation::Agent(AgentOperation::GetCapabilities),
            Operation::Agent(AgentOperation::UpdateCapabilities),
        ]
    }

    async fn handle(
        &self,
        message: AgentServerMessage,
    ) -> Result<Vec<AgentServerMessage>, ProtocolError> {
        let op = match &message.operation {
            Operation::Agent(op) => op.clone(),
            _ => return Err(ProtocolError::PayloadDecodeFailed("capability")),
        };

        match (op, message.payload) {
            (
                AgentOperation::GetCapabilities,
                Payload::Agent(AgentPayload::GetCapabilities {
                    agent_id,
                    session_id,
                }),
            ) => {
                let overlays = self.overlays.read().await;
                let (effective_tools, effective_skills, effective_mcp_servers) =
                    self.resolve_effective(&agent_id, &session_id, &overlays);

                // Get base defaults for reset
                let def = self
                    .agent_defs
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&agent_id)
                    .cloned();
                let (base_tools, base_skills, base_mcp_servers) = match def {
                    Some(d) => (
                        d.tools.unwrap_or_default(),
                        d.skills.unwrap_or_default(),
                        d.mcps.unwrap_or_default(),
                    ),
                    None => (vec![], vec![], vec![]),
                };

                let available = self.gather_available().await;
                drop(overlays);

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::GetCapabilities),
                    Payload::Agent(AgentPayload::GetCapabilitiesResult {
                        effective_tools,
                        effective_skills,
                        effective_mcp_servers,
                        available_tools: available.tools,
                        available_skills: available.skills,
                        available_mcp_servers: available.mcp_servers,
                        base_tools,
                        base_skills,
                        base_mcp_servers,
                    }),
                )])
            }

            (
                AgentOperation::UpdateCapabilities,
                Payload::Agent(AgentPayload::UpdateCapabilities {
                    agent_id,
                    session_id,
                    effective_tools,
                    effective_skills,
                    effective_mcp_servers,
                }),
            ) => {
                // --- Validation ---

                // 1. Check AgentDef disallowed_tools
                let def = self
                    .agent_defs
                    .read()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .get(&agent_id)
                    .cloned();
                if let Some(ref def) = def {
                    let disallowed: std::collections::HashSet<&str> = def
                        .disallowed_tools
                        .as_ref()
                        .map(|v| v.iter().map(String::as_str).collect())
                        .unwrap_or_default();
                    for tool in &effective_tools {
                        if disallowed.contains(tool.as_str()) {
                            return Ok(vec![AgentServerMessage::new_error(
                                message.message_id,
                                Operation::Agent(AgentOperation::UpdateCapabilities),
                                ErrorPayload {
                                    code: "tool_disallowed".to_string(),
                                    message: format!(
                                        "Tool '{}' is disallowed by agent definition",
                                        tool
                                    ),
                                    detail: None,
                                    terminal: false,
                                },
                            )]);
                        }
                    }

                    // 2. Check mcps allowlist constraint
                    if let Some(ref allowed_mcps) = def.mcps {
                        for server in &effective_mcp_servers {
                            if !allowed_mcps.contains(server) {
                                return Ok(vec![AgentServerMessage::new_error(
                                    message.message_id,
                                    Operation::Agent(AgentOperation::UpdateCapabilities),
                                    ErrorPayload {
                                        code: "mcp_not_allowed".to_string(),
                                        message: format!(
                                            "MCP server '{}' is not in agent's allowed mcps list",
                                            server
                                        ),
                                        detail: None,
                                        terminal: false,
                                    },
                                )]);
                            }
                        }
                    }
                }

                // 3. Validate tool names exist in master registry
                let master_tool_names: std::collections::HashSet<&str> = self
                    .tool_registry
                    .tool_names()
                    .into_iter()
                    .collect();
                for tool in &effective_tools {
                    if !master_tool_names.contains(tool.as_str()) {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::UpdateCapabilities),
                            ErrorPayload {
                                code: "unknown_tool".to_string(),
                                message: format!("Tool '{}' not found in registry", tool),
                                detail: None,
                                terminal: false,
                            },
                        )]);
                    }
                }

                // 4. Validate skill names exist
                let skill_metadata = self.skill_loader.list_metadata().await;
                let skill_names: std::collections::HashSet<&str> = skill_metadata
                    .iter()
                    .map(|m| m.name.as_str())
                    .collect();
                for skill in &effective_skills {
                    if !skill_names.contains(skill.as_str()) {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::UpdateCapabilities),
                            ErrorPayload {
                                code: "unknown_skill".to_string(),
                                message: format!("Skill '{}' not found", skill),
                                detail: None,
                                terminal: false,
                            },
                        )]);
                    }
                }

                // 5. Validate MCP server names exist
                let server_status = self.mcp_manager.server_status();
                for server in &effective_mcp_servers {
                    if !server_status.contains_key(server.as_str()) {
                        return Ok(vec![AgentServerMessage::new_error(
                            message.message_id,
                            Operation::Agent(AgentOperation::UpdateCapabilities),
                            ErrorPayload {
                                code: "unknown_mcp_server".to_string(),
                                message: format!("MCP server '{}' not found", server),
                                detail: None,
                                terminal: false,
                            },
                        )]);
                    }
                }

                // --- Apply overlay ---
                let key = (agent_id.clone(), session_id.clone());
                let mut overlays = self.overlays.write().await;

                if effective_tools.is_empty()
                    && effective_skills.is_empty()
                    && effective_mcp_servers.is_empty()
                {
                    // Empty lists = reset to default → remove overlay
                    overlays.remove(&key);
                } else if let Some(existing) = overlays.get_mut(&key) {
                    existing.update(
                        effective_tools.clone(),
                        effective_skills.clone(),
                        effective_mcp_servers.clone(),
                    );
                } else {
                    overlays.insert(
                        key,
                        CapabilityOverlay::new(
                            effective_tools.clone(),
                            effective_skills.clone(),
                            effective_mcp_servers.clone(),
                        ),
                    );
                }
                drop(overlays);

                Ok(vec![AgentServerMessage::new_result(
                    message.message_id,
                    Operation::Agent(AgentOperation::UpdateCapabilities),
                    Payload::Agent(AgentPayload::UpdateCapabilitiesResult {
                        effective_tools,
                        effective_skills,
                        effective_mcp_servers,
                    }),
                )])
            }

            (AgentOperation::GetCapabilities, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.get_capabilities"))
            }
            (AgentOperation::UpdateCapabilities, _) => {
                Err(ProtocolError::PayloadDecodeFailed("agent.update_capabilities"))
            }
            _ => Err(ProtocolError::PayloadDecodeFailed("capability")),
        }
    }
}
```

- [ ] **Step 2: Build check**

Run: `cargo check -p vol-agent-server`
Expected: compiles successfully.

- [ ] **Step 3: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/handlers/capability.rs
git commit -m "feat(handler): add CapabilityHandler for dynamic capability overlay"
```

---

### Task 4: Register CapabilityHandler in DataPlaneServerCore

**Files:**
- Modify: `crates/vol-agent-server/src/data_plane/core.rs:27-33` (handler imports)
- Modify: `crates/vol-agent-server/src/data_plane/core.rs:451-498` (handler registration)
- Modify: `crates/vol-agent-server/src/data_plane/handlers/mod.rs` (module declaration)

- [ ] **Step 1: Add module declaration**

Check `crates/vol-agent-server/src/data_plane/handlers/mod.rs` for the existing module list. Add:

```rust
pub mod capability;
```

- [ ] **Step 2: Add import in core.rs**

In the handler imports block (around line 27-33), add:

```rust
use crate::data_plane::handlers::capability::CapabilityHandler;
```

- [ ] **Step 3: Register CapabilityHandler**

In `DataPlaneServerCoreBuilder::build()`, after the existing handler registrations (around line 493) and before the `extra_handlers` loop, add:

```rust
handler_registry
    .register(Arc::new(CapabilityHandler::new(
        capability_overlays.clone(),
        tool_registry.clone(),
        skill_loader.clone(),
        mcp_manager.clone(),
        agent_defs.clone(),
    )))
    .map_err(|e| format!("failed to register CapabilityHandler: {e}"))?;
```

- [ ] **Step 4: Build check**

Run: `cargo check -p vol-agent-server`
Expected: compiles successfully.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-agent-server/src/data_plane/
git commit -m "feat(core): register CapabilityHandler in DataPlaneServerCore"
```

---

### Task 5: SkillInjector — add shared skill filter for runtime updates

**Files:**
- Modify: `crates/vol-llm-skill/src/injector.rs:9-58` (SkillInjector struct, new, format_metadata)

- [ ] **Step 1: Change `skill_filter` field to use shared `Arc<RwLock<...>>`**

In `crates/vol-llm-skill/src/injector.rs`, modify the struct:

```rust
use tokio::sync::RwLock;

pub struct SkillInjector {
    loader: Arc<SkillLoader>,
    anchor: AttentionAnchor,
    cached_size: tokio::sync::Mutex<usize>,
    /// Shared filter: only include skills whose names are in this set.
    /// None or empty = include all discovered skills.
    /// Wrapped in Arc<RwLock> so it can be updated externally by the
    /// capability overlay system without replacing the contributor.
    pub skill_filter: Arc<RwLock<Option<Vec<String>>>>,
}
```

Modify `new()`:

```rust
pub fn new(
    loader: Arc<SkillLoader>,
    anchor: AttentionAnchor,
    skill_filter: Option<Vec<String>>,
) -> Self {
    Self {
        loader,
        anchor,
        cached_size: tokio::sync::Mutex::new(0),
        skill_filter: Arc::new(RwLock::new(skill_filter)),
    }
}
```

Update `from_workdir()`:

```rust
pub async fn from_workdir(working_dir: &std::path::Path, anchor: AttentionAnchor) -> Self {
    let loader = Arc::new(crate::loader::SkillLoader::new(Some(
        working_dir.to_path_buf(),
    )));
    Self::new(loader, anchor, None)
}
```

- [ ] **Step 2: Modify `format_metadata()` to read from shared filter**

```rust
pub async fn format_metadata(&self) -> String {
    let metadata = self.loader.list_metadata().await;
    if metadata.is_empty() {
        return String::new();
    }

    // Apply skill name filter if set (reads from shared filter)
    let filter_guard = self.skill_filter.read().await;
    let filtered: Vec<_> = if let Some(ref filter) = *filter_guard {
        if filter.is_empty() {
            metadata
        } else {
            let filter_set: std::collections::HashSet<&str> =
                filter.iter().map(String::as_str).collect();
            metadata
                .into_iter()
                .filter(|m| filter_set.contains(m.name.as_str()))
                .collect()
        }
    } else {
        metadata
    };
    drop(filter_guard);

    if filtered.is_empty() {
        return String::new();
    }

    let mut output = String::from("Available skills:\n");
    for m in &filtered {
        output.push_str(&format!("- {}: {}\n", m.name, m.description));
    }
    output.push_str("\nUse the `skill` tool to load any skill's full instructions.");
    output
}
```

- [ ] **Step 3: Update all existing `SkillInjector::new()` call sites**

Run: `grep -rn "SkillInjector::new" crates/`
Update each call site to pass `None` as the third argument.

- [ ] **Step 4: Build and test**

Run: `cargo check -p vol-llm-skill -p vol-llm-agent -p vol-agent-server`
Expected: compiles successfully.

Run: `cargo test -p vol-llm-skill`
Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-skill/src/injector.rs
# Add other files with call-site updates
git commit -m "feat(skill): add shared Arc<RwLock> skill filter to SkillInjector for runtime updates"
```

---

### Task 6: Agent loop — overlay version check and rebuild

**Files:**
- Modify: `crates/vol-llm-agent/src/react/agent.rs:398-425` (ReAct loop)
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:36-103` (RunContext struct)
- Modify: `crates/vol-llm-agent/src/react/run_context.rs:294-332` (effective_registry)

- [ ] **Step 1: Add `skill_injector_filter` to AgentConfig and RunContext**

In `crates/vol-llm-agent/src/react/agent.rs`, add to `AgentConfig`:

```rust
/// Shared skill filter from SkillInjector, for runtime updates via capability overlay.
pub skill_injector_filter: Option<Arc<tokio::sync::RwLock<Option<Vec<String>>>>>,
```

Initialize as `None` in `AgentConfig::default()`.

In `crates/vol-llm-agent/src/react/run_context.rs`, add to `RunContext`:

```rust
/// Reference to capability overlay map for runtime tool/skill/MCP adjustment.
pub capability_overlays: Option<Arc<tokio::sync::RwLock<std::collections::HashMap<(String, String), vol_llm_runtime::CapabilityOverlay>>>>,
/// Agent ID for overlay lookup.
pub agent_id: String,
/// Last seen overlay version — when this changes we rebuild.
pub(crate) current_overlay_version: Arc<std::sync::atomic::AtomicU64>,
```

Add import:
```rust
use std::sync::atomic::AtomicU64;
```

Initialize in `RunContext::new()`:

```rust
capability_overlays: None,
agent_id: String::new(),
current_overlay_version: Arc::new(AtomicU64::new(0)),
```

- [ ] **Step 2: Add setter on RunContext**

```rust
/// Wire capability overlays for runtime adjustment.
pub fn with_capability_overlays(
    mut self,
    overlays: Arc<tokio::sync::RwLock<std::collections::HashMap<(String, String), vol_llm_runtime::CapabilityOverlay>>>,
    agent_id: String,
) -> Self {
    self.capability_overlays = Some(overlays);
    self.agent_id = agent_id;
    self
}
```

- [ ] **Step 3: Modify `effective_registry()` to check overlay**

Replace the existing `effective_registry()` method:

```rust
fn effective_registry(&self) -> Arc<ToolRegistry> {
    // Check for runtime overlay first
    if let Some(ref overlays) = self.capability_overlays {
        if let Ok(guard) = overlays.try_read() {
            let key = (self.agent_id.clone(), self.session_id.clone());
            if let Some(overlay) = guard.get(&key) {
                let version = overlay.version;
                let last = self
                    .current_overlay_version
                    .load(std::sync::atomic::Ordering::Acquire);
                if version != last {
                    // Build filtered registry from overlay
                    let allowed: Option<Vec<&str>> = Some(
                        overlay.effective_tools.iter().map(String::as_str).collect(),
                    );
                    let disallowed: Option<Vec<&str>> = self
                        .config
                        .def
                        .as_ref()
                        .and_then(|d| d.disallowed_tools.as_ref())
                        .map(|v| v.iter().map(String::as_str).collect());
                    let mut filtered =
                        ToolRegistry::filter(&self.tools, allowed.as_deref(), disallowed.as_deref());

                    // Apply MCP server filter
                    if !overlay.effective_mcp_servers.is_empty() {
                        filtered = Arc::new(
                            (*filtered).clone().filter_mcp_servers(
                                &overlay.effective_mcp_servers,
                            ),
                        );
                    }

                    // Also update the skill injector's shared filter
                    if let Some(ref filter) = self.config.skill_injector_filter {
                        let new_filter = if overlay.effective_skills.is_empty() {
                            None  // empty = include all
                        } else {
                            Some(overlay.effective_skills.clone())
                        };
                        *filter.write().await = new_filter;
                    }

                    self.current_overlay_version
                        .store(version, std::sync::atomic::Ordering::Release);
                    tracing::debug!(
                        version = version,
                        tools = overlay.effective_tools.len(),
                        skills = overlay.effective_skills.len(),
                        "Capability overlay applied"
                    );
                    return filtered;
                }
            }
        }
    }

    // Fall back to AgentDef-based filtering (original logic)
    if let Some(def) = &self.config.def {
        let allowed: Option<Vec<&str>> = def
            .tools
            .as_ref()
            .map(|t| t.iter().map(std::string::String::as_str).collect());
        let disallowed: Option<Vec<&str>> = def
            .disallowed_tools
            .as_ref()
            .map(|t| t.iter().map(std::string::String::as_str).collect());
        ToolRegistry::filter(&self.tools, allowed.as_deref(), disallowed.as_deref())
    } else {
        self.tools.clone()
    }
}
```

- [ ] **Step 4: Wire capability_overlays into run_input, and capture skill_filter in config_builder**

In `crates/vol-llm-agent/src/react/config_builder.rs` (around line 272), after creating the SkillInjector, capture its filter Arc:

```rust
// In AgentConfigBuilder::build(), replace the existing SkillInjector creation:
let skill_injector = SkillInjector::new(
    skill_loader,
    AttentionAnchor::Head(1),
    None,  // no filter initially
);
let skill_injector_filter = Some(skill_injector.skill_filter.clone());
b = b.add_contributor(Box::new(skill_injector));
```

Store `skill_injector_filter` in the final config. The builder needs a field for it:

```rust
// In AgentConfigBuilder struct, add:
skill_injector_filter: Option<Arc<tokio::sync::RwLock<Option<Vec<String>>>>>,
```

And in the `AgentConfig` output:
```rust
skill_injector_filter: self.skill_injector_filter,
```

In `ReActAgent::run_input()` (around line 337 where `RunContext::new` is called), pass the capability overlays and agent_id:

```rust
let (mut run_ctx, plugin_rx) =
    RunContext::new(run_id.clone(), user_input.clone(), self.config.clone());

if let Some(ref overlays) = self.config.capability_overlays {
    run_ctx = run_ctx.with_capability_overlays(
        overlays.clone(),
        self.config.agent_id.clone(),
    );
}
```

(Add `mut` to the `run_ctx` binding.)

- [ ] **Step 5: Wire capability_overlays from DataPlaneServerCore into AgentConfig**

In `crates/vol-agent-server/src/data_plane/core.rs`, in `register_agent()` (around line 211 where `AgentConfig::builder()` is used), add:

```rust
config.capability_overlays = Some(self.capability_overlays.clone());
```

- [ ] **Step 6: Build check**

Run: `cargo check -p vol-llm-agent -p vol-agent-server`
Expected: compiles successfully.

- [ ] **Step 7: Commit**

```bash
git add crates/vol-llm-agent/src/react/agent.rs crates/vol-llm-agent/src/react/run_context.rs crates/vol-agent-server/src/data_plane/core.rs
git commit -m "feat(agent): add overlay version check in ReAct loop for dynamic capability adjustment"
```

---

In Task 6 Step 4 (or as a follow-up step):

- [ ] **Step 6: Add overlay cleanup in unregister_agent**

In `crates/vol-llm-runtime/src/lib.rs`, find the `unregister_agent` method. Add overlay cleanup:

```rust
// Cleanup capability overlays for this agent
{
    let mut overlays = self.capability_overlays.write().await;
    overlays.retain(|(agent_id, _), _| agent_id != removed_agent_id);
}
```

Commit along with the rest of Task 6.

---

### Task 7: Tests — protocol and handler

**Files:**
- Create: `crates/vol-agent-server/src/data_plane/handlers/capability_tests.rs` (test module)
- Modify: `crates/vol-llm-agent-protocol/tests/` or inline tests

- [ ] **Step 1: Add inline unit tests for CapabilityOverlay**

In `crates/vol-llm-runtime/src/capability_overlay.rs`, add:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_new_has_version_one() {
        let overlay = CapabilityOverlay::new(
            vec!["bash".into(), "read".into()],
            vec!["code-review".into()],
            vec!["k8s".into()],
        );
        assert_eq!(overlay.version, 1);
        assert_eq!(overlay.effective_tools.len(), 2);
    }

    #[test]
    fn overlay_update_bumps_version() {
        let mut overlay = CapabilityOverlay::new(vec!["bash".into()], vec![], vec![]);
        assert_eq!(overlay.version, 1);
        overlay.update(vec!["bash".into(), "write".into()], vec![], vec![]);
        assert_eq!(overlay.version, 2);
        assert_eq!(overlay.effective_tools.len(), 2);
    }

    #[test]
    fn overlay_matches_detects_no_change() {
        let overlay = CapabilityOverlay::new(
            vec!["bash".into(), "read".into()],
            vec![],
            vec![],
        );
        assert!(overlay.matches(&["bash".into(), "read".into()], &[], &[]));
        assert!(!overlay.matches(&["bash".into()], &[], &[]));
        assert!(!overlay.matches(&["bash".into(), "read".into(), "write".into()], &[], &[]));
    }

    #[test]
    fn overlay_version_persists_across_updates() {
        let mut overlay = CapabilityOverlay::new(vec![], vec![], vec![]);
        overlay.update(vec!["a".into()], vec![], vec![]);
        overlay.update(vec!["a".into(), "b".into()], vec![], vec![]);
        overlay.update(vec!["c".into()], vec![], vec![]);
        assert_eq!(overlay.version, 4);
    }
}
```

- [ ] **Step 2: Run unit tests**

Run: `cargo test -p vol-llm-runtime -- cap`
Expected: 4 tests pass.

- [ ] **Step 3: Add handler unit tests in capability_tests.rs**

Create `crates/vol-agent-server/src/data_plane/handlers/capability_tests.rs`:

```rust
#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::RwLock;

    use vol_llm_agent_protocol::agent_server_protocol::{
        AgentOperation, AgentPayload, AgentServerMessage, MessageKind, Operation, Payload,
    };
    use vol_llm_agent_protocol::DomainHandler;
    use vol_llm_core::agent_def::AgentDef;
    use vol_llm_mcp::{McpConfig, McpManager};
    use vol_llm_runtime::CapabilityOverlay;
    use vol_llm_skill::SkillLoader;
    use vol_llm_tool::ToolRegistry;

    use super::super::capability::CapabilityHandler;

    fn test_agent_def() -> AgentDef {
        let mut def = AgentDef::default();
        def.tools = Some(vec!["bash".into(), "read".into()]);
        def.disallowed_tools = Some(vec!["dangerous".into()]);
        def.skills = Some(vec!["code-review".into()]);
        def
    }

    fn test_handler() -> CapabilityHandler {
        let overlays = Arc::new(RwLock::new(HashMap::new()));
        let tool_registry = {
            let mut r = ToolRegistry::new();
            // We need at least one tool registered for tool_names() to work
            r
        };
        let skill_loader = Arc::new(SkillLoader::new_empty());
        let mcp_manager = Arc::new(McpManager::new(vec![]));
        let agent_defs = {
            let mut map = HashMap::new();
            map.insert("test-agent".into(), test_agent_def());
            Arc::new(std::sync::RwLock::new(map))
        };
        CapabilityHandler::new(
            overlays,
            Arc::new(tool_registry),
            skill_loader,
            mcp_manager,
            agent_defs,
        )
    }

    fn msg(id: &str, op: Operation, payload: Payload) -> AgentServerMessage {
        AgentServerMessage {
            protocol: "agent-server/1".into(),
            message_id: id.into(),
            sender: "client".into(),
            receiver: "data-plane".into(),
            kind: MessageKind::Command,
            operation: op,
            payload,
            meta: Default::default(),
        }
    }

    #[tokio::test]
    async fn get_capabilities_returns_defaults_when_no_overlay() {
        let handler = test_handler();
        let replies = handler
            .handle(msg(
                "1",
                Operation::Agent(AgentOperation::GetCapabilities),
                Payload::Agent(AgentPayload::GetCapabilities {
                    agent_id: "test-agent".into(),
                    session_id: "sess-1".into(),
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(json["effective_tools"].as_array().unwrap().len(), 2);
        assert!(json["effective_tools"].as_array().unwrap().iter().any(|v| v == "bash"));
        assert_eq!(json["base_tools"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn update_capabilities_rejects_disallowed_tool() {
        let handler = test_handler();
        let replies = handler
            .handle(msg(
                "1",
                Operation::Agent(AgentOperation::UpdateCapabilities),
                Payload::Agent(AgentPayload::UpdateCapabilities {
                    agent_id: "test-agent".into(),
                    session_id: "sess-1".into(),
                    effective_tools: vec!["dangerous".into()],
                    effective_skills: vec![],
                    effective_mcp_servers: vec![],
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(json["code"], "tool_disallowed");
    }

    #[tokio::test]
    async fn update_capabilities_creates_overlay_and_returns_result() {
        let handler = test_handler();
        let replies = handler
            .handle(msg(
                "1",
                Operation::Agent(AgentOperation::UpdateCapabilities),
                Payload::Agent(AgentPayload::UpdateCapabilities {
                    agent_id: "test-agent".into(),
                    session_id: "sess-1".into(),
                    effective_tools: vec!["bash".into()],
                    effective_skills: vec!["code-review".into()],
                    effective_mcp_servers: vec![],
                }),
            ))
            .await
            .unwrap();

        let json = replies[0].payload.data_json();
        assert_eq!(json["effective_tools"].as_array().unwrap().len(), 1);
        assert_eq!(json["effective_skills"].as_array().unwrap().len(), 1);
        // Subsequent get should return overlay values
        let replies2 = handler
            .handle(msg(
                "2",
                Operation::Agent(AgentOperation::GetCapabilities),
                Payload::Agent(AgentPayload::GetCapabilities {
                    agent_id: "test-agent".into(),
                    session_id: "sess-1".into(),
                }),
            ))
            .await
            .unwrap();
        let json2 = replies2[0].payload.data_json();
        assert_eq!(json2["effective_tools"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn update_capabilities_respects_mcps_allowlist() {
        let handler = test_handler();
        // test_agent_def has no mcps restriction (None = all allowed), so we test
        // with a def that has explicit mcps
        let replies = handler
            .handle(msg(
                "1",
                Operation::Agent(AgentOperation::UpdateCapabilities),
                Payload::Agent(AgentPayload::UpdateCapabilities {
                    agent_id: "test-agent".into(),
                    session_id: "sess-1".into(),
                    effective_tools: vec![],
                    effective_skills: vec![],
                    effective_mcp_servers: vec!["nonexistent".into()],
                }),
            ))
            .await
            .unwrap();
        // nonexistent MCP server should fail validation
        let json = replies[0].payload.data_json();
        assert_eq!(json["code"], "unknown_mcp_server");
    }
}
```

Add module declaration at the end of `capability.rs`:
```rust
#[cfg(test)]
#[path = "capability_tests.rs"]
mod tests;
```

- [ ] **Step 4: Run handler tests**

Run: `cargo test -p vol-agent-server -- capability`
Expected: tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-runtime/src/capability_overlay.rs crates/vol-agent-server/src/data_plane/handlers/capability.rs crates/vol-agent-server/src/data_plane/handlers/capability_tests.rs
git commit -m "test: add unit tests for CapabilityOverlay and CapabilityHandler"
```

---

### Task 8: Frontend — capability state and apply logic

**Files:**
- Modify: `crates/vol-llm-ui/src/state/mod.rs` (add capability state)
- Modify: `crates/vol-llm-ui/src/web/client.rs` (add RPC methods, if needed)

- [ ] **Step 1: Add capability state to UI state**

In `crates/vol-llm-ui/src/state/mod.rs`, add new types:

```rust
/// Capability overlay state received from the server.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct CapabilityOverlayState {
    pub effective_tools: Vec<String>,
    pub effective_skills: Vec<String>,
    pub effective_mcp_servers: Vec<String>,
    pub available_tools: Vec<serde_json::Value>,
    pub available_skills: Vec<serde_json::Value>,
    pub available_mcp_servers: Vec<serde_json::Value>,
    pub base_tools: Vec<String>,
    pub base_skills: Vec<String>,
    pub base_mcp_servers: Vec<String>,
    pub loading: bool,
    pub dirty: bool,
}

impl CapabilityOverlayState {
    pub fn new() -> Self {
        Self {
            loading: true,
            dirty: false,
            ..Default::default()
        }
    }

    pub fn is_modified(&self) -> bool {
        self.dirty
    }
}
```

- [ ] **Step 2: Add `capabilities` field to GlobalState**

In `GlobalState` struct, add:

```rust
pub capabilities: CapabilityOverlayState,
```

Initialize in `GlobalState::new()`:
```rust
capabilities: CapabilityOverlayState::new(),
```

- [ ] **Step 3: Commit**

```bash
git add crates/vol-llm-ui/src/state/mod.rs
git commit -m "feat(ui): add CapabilityOverlayState to UI state model"
```

---

### Task 9: Frontend — checkbox overlay for Tools/Skills/MCP panels

**Files:**
- Modify: `crates/vol-llm-ui/src/web/components/tools_panel.rs`
- Modify: `crates/vol-llm-ui/src/web/components/skills.rs`
- Modify: `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

**Note:** This task adapts the existing Dioxus components to add checkboxes and apply/reset buttons. The exact Dioxus RSX syntax depends on the current component structure. The steps below show the logical changes needed; adapt the Dioxus syntax to match existing patterns.

- [ ] **Step 1: Add checkbox and apply button to tools panel**

In `tools_panel.rs`, add to the component state:

```rust
struct CapabilityEditState {
    selected_tools: std::collections::HashSet<String>,
    dirty: bool,
}
```

In the tool list rendering, change each tool row from a plain display to:

```rust
// Pseudo-Dioxus: for each tool in available list, render:
rsx! {
    div {
        class: "flex items-center gap-2",
        input {
            r#type: "checkbox",
            checked: selected_tools.contains(&tool.name),
            onchange: move |evt| {
                if evt.value() == "true" {
                    selected_tools.insert(tool.name.clone());
                } else {
                    selected_tools.remove(&tool.name);
                }
                dirty = true;
            },
        }
        span { "{tool.name}" }
    }
}
```

Add Apply and Reset buttons:

```rust
rsx! {
    div {
        class: "flex gap-2 mt-2",
        button {
            class: "px-3 py-1 bg-blue-500 text-white rounded",
            disabled: !dirty,
            onclick: move |_| {
                // Call agent.update_capabilities with current selection
                let payload = serde_json::json!({
                    "agent_id": active_agent_id,
                    "session_id": active_session_id,
                    "effective_tools": selected_tools.iter().collect::<Vec<_>>(),
                    "effective_skills": current_skills,
                    "effective_mcp_servers": current_mcp_servers,
                });
                // Send via JsonRpcClient
                // After success: dirty = false, update global state
            },
            "Apply"
        }
        button {
            class: "px-3 py-1 bg-gray-300 rounded",
            onclick: move |_| {
                // Reset to base_tools
                selected_tools = base_tools.iter().cloned().collect();
                dirty = true;
            },
            "Reset to default"
        }
    }
}
```

- [ ] **Step 2: Apply similar pattern to skills panel**

In `skills.rs`, add checkboxes for each skill in the available list. Reuse the same pattern — checkboxes + Apply + Reset.

- [ ] **Step 3: Apply similar pattern to MCP panel**

In `mcp_panel.rs`, add checkboxes for each MCP server in the available list. Reuse the same pattern.

- [ ] **Step 4: Call get_capabilities on page load**

When the frontend connects or switches agents/sessions, call `agent.get_capabilities` to restore the current overlay state:

```rust
// In the relevant on-mount or agent-switch handler:
let response = client.call("agent.get_capabilities", serde_json::json!({
    "agent_id": agent_id,
    "session_id": session_id,
})).await;
// Parse CapabilityOverlayState from response
// Update global state
```

- [ ] **Step 5: Commit**

```bash
git add crates/vol-llm-ui/src/web/components/
git commit -m "feat(ui): add capability checkboxes with apply/reset to Tools, Skills, MCP panels"
```

---

### Task 10: End-to-end verification and cleanup

**Files:**
- All modified crates

- [ ] **Step 1: Full build**

Run: `cargo build -p vol-agent-server`
Expected: compiles successfully.

- [ ] **Step 2: Run all tests**

Run: `cargo test -p vol-llm-agent-protocol -p vol-llm-runtime -p vol-agent-server -p vol-llm-agent`
Expected: all tests pass, including new ones.

- [ ] **Step 3: Run coverage check**

Run: `make coverage-threshold PKG=vol-llm-runtime PCT=80`
Expected: coverage ≥ 80%.

- [ ] **Step 4: Manual verification checklist**

- Start the agent server
- Open the web UI
- Navigate to Tools panel → verify checkboxes show current state
- Toggle a tool checkbox → verify "dirty" indicator appears
- Click "Apply" → verify `agent.update_capabilities` succeeds
- Verify the agent picks up the new tool on the next LLM call
- Click "Reset to default" → verify tools revert to AgentDef defaults
- Verify page refresh preserves state (call `agent.get_capabilities` on load)
- Verify disallowed tools cannot be selected

- [ ] **Step 5: Wiki ingest**

After verification, run wiki-ingest to document the new capability overlay system:

```bash
# Follow wiki-ingest skill instructions
```

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "chore: final cleanup and verification for dynamic capability overlay"
```
