# Dynamic Capability Overlay — runtime tool/skill/MCP adjustment per session

**Date:** 2026-07-29
**Status:** draft

## Context

Currently, an agent's tool set, skills, and MCP servers are fixed at registration time
(`AgentRuntimeBuilder::build()` → `register_agent()`). The `AgentDef` allow/deny lists
are applied once, producing a filtered `Arc<ToolRegistry>` that stays immutable for the
entire conversation.

Users want to adjust these capabilities mid-conversation without restarting: tick a few
extra tools, enable an MCP server, or add a skill, and have the running agent pick up
the change on its next LLM call.

The full tool/skill/MCP pool is already available in memory (master `ToolRegistry`,
`SkillLoader`, `McpManager`). This feature is about **enable/disable from the existing
pool** — it does NOT add new tools to the registry or connect new MCP servers at runtime.

## Design

### Principle

- **AgentDef is the base default** — what the agent starts with.
- **CapabilityOverlay is the runtime adjustment** — a per-session replacement list that
  overrides the base configuration.
- **Overlay lives in memory only** — survives frontend refresh, dies on server restart.
- **Effective capability = overlay if set, else AgentDef base**.

### Data model

```rust
/// Per-session capability adjustment, keyed by (agent_id, session_id).
/// Lives in AgentRuntime, purely in-memory.
#[derive(Debug, Clone, Default)]
pub struct CapabilityOverlay {
    pub version: u64,
    pub effective_tools: Vec<String>,
    pub effective_skills: Vec<String>,
    pub effective_mcp_servers: Vec<String>,
}
```

`version` increments on every `update_capabilities` call. The agent loop polls this
version and rebuilds its filtered toolset when it changes.

The overlay map lives in `AgentRuntime`:

```rust
// AgentRuntime new field
capability_overlays: Arc<RwLock<HashMap<(String, String), CapabilityOverlay>>>
//                                        ^^^^^^^^^^^^^^^^
//                                        (agent_id,   session_id)
```

Why a replacement list (not a diff)? Simpler — no merge conflicts, frontend just sends
the final list, server overwrites. The frontend pre-fills the initial list from AgentDef
on first load.

### Protocol — two new operations

| Operation | Method | Direction | Purpose |
|-----------|--------|-----------|---------|
| `AgentOperation::GetCapabilities` | `agent.get_capabilities` | Client → DP | Restore overlay state after page refresh |
| `AgentOperation::UpdateCapabilities` | `agent.update_capabilities` | Client → DP | Apply user's new tool/skill/MCP selection |

Both use the same payload shape:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilitiesState {
    // The current effective lists (what the agent is actually using)
    pub effective_tools: Vec<String>,
    pub effective_skills: Vec<String>,
    pub effective_mcp_servers: Vec<String>,

    // Full available pool — for the frontend to render checkboxes
    pub available_tools: Vec<ToolInfo>,
    pub available_skills: Vec<SkillInfo>,
    pub available_mcp_servers: Vec<McpServerInfo>,
}
```

`get_capabilities` request: `{ agent_id, session_id }` → response: `CapabilitiesState`.

`update_capabilities` request: `{ agent_id, session_id, effective_tools, effective_skills, effective_mcp_servers }` → response: `CapabilitiesState` (with updated effective lists + same available lists).

### Handler — new CapabilityHandler

New file: `crates/vol-agent-server/src/data_plane/handlers/capability.rs`

```rust
pub struct CapabilityHandler {
    runtime: Arc<AgentRuntime>,
}
```

Implements `DomainHandler`. Operations: `GetCapabilities`, `UpdateCapabilities`.

**GetCapabilities:**
1. Look up overlay for `(agent_id, session_id)`. If none, fall back to AgentDef base config.
2. Gather available pools from: `tool_registry.tool_names()`, `skill_loader.list_metadata()`, `mcp_manager.server_status()`.
3. Return `CapabilitiesState`.

**UpdateCapabilities:**
1. Validate: every tool name in `effective_tools` must exist in master registry, and must not be in `AgentDef.disallowed_tools`.
2. Validate: every skill in `effective_skills` must exist in `skill_loader`.
3. Validate: every MCP server in `effective_mcp_servers` must exist in `mcp_manager`.
4. If validation fails → return error with details (which names are invalid/forbidden).
5. Create or update overlay: overwrite effective lists, `version += 1`.
6. Return `CapabilitiesState`.

### Runtime reactivity — agent loop picks up overlay changes

The ReAct agent loop currently holds a fixed `Arc<ToolRegistry>` and fixed skill injector.
Two changes are needed:

**1. Agent runtime stores a reference to the overlay:**

```rust
// In the agent's run state (e.g. RunContext or agent loop state)
current_overlay_version: u64,
```

**2. Before each LLM call, check for overlay changes:**

```text
loop {
    // Check overlay version
    let overlay = runtime.capability_overlays.read().get(&(agent_id, session_id));
    if overlay.version != current_overlay_version {
        // Rebuild filtered tool registry
        tool_registry = master_registry.filter(overlay.effective_tools);
        // Rebuild MCP filter
        tool_registry = tool_registry.filter_mcp_servers(overlay.effective_mcp_servers);
        // Rebuild skill injector
        skill_injector = skill_loader.build_injector(overlay.effective_skills);
        // Rebuild context with new system prompt (skills changed)
        context = context_builder
            .with_skill_injector(skill_injector)
            .build();
        current_overlay_version = overlay.version;
    }

    // LLM call with current tool_registry and context
    response = llm.generate(context, tool_registry.definitions());
    // ... execute tool calls, observe, loop
}
```

**Tools:** re-filtering `ToolRegistry` is cheap (HashMap clone + filter).

**MCP servers:** `filter_mcp_servers()` already exists on `ToolRegistry`.

**Skills:** the skill list change means the `SkillInjector` needs to inject different
skill instructions into the system prompt. This requires rebuilding the context (system
prompt changes), but conversation history (Session messages) stays intact — only the
system-level instructions change.

### Constraints

- **disallowed_tools in AgentDef cannot be overridden.** The handler rejects any
  `effective_tools` entry that matches an item in `disallowed_tools`.
- **mcps field restricts MCP server options.** If `AgentDef.mcps` is `Some([...])`
  (explicit allowlist), the overlay cannot add servers outside that list. If `None`
  (all servers allowed), the overlay can select any.
- **Non-existent tool/skill/server names are rejected** with specific error messages.

### Overlay lifecycle

| Event | Action |
|-------|--------|
| First `update_capabilities` for a session | Create overlay entry, seeded from AgentDef base |
| Subsequent `update_capabilities` | Replace lists, bump version |
| `get_capabilities` (no overlay exists) | Return AgentDef base, version=0 |
| Agent run starts | Read overlay if exists, else use AgentDef base |
| Agent unregistered (`unregister_agent`) | Remove all overlay entries for that agent_id |
| Frontend explicitly resets | `update_capabilities` with empty lists → delete overlay entry |

Cleanup on `unregister_agent` prevents memory leaks from stale sessions. Overlay entries
are keyed by `(agent_id, session_id)`; when an agent is unregistered, all its session
overlays are dropped. Individual session cleanup can be added later if needed.

### Implementation note: AgentRuntime reference in agent loop

The ReAct agent's inner loop must read `capability_overlays` from `AgentRuntime` to check
the version. Currently the agent receives an `AgentConfig` but not a reference to
`AgentRuntime`. During implementation, verify the exact injection path: either add an
`Arc<AgentRuntime>` reference to `AgentConfig`, or pass the overlay map separately.
The final approach will be determined during implementation-plan phase.

### Frontend — checkboxes + apply button

Each capability panel (Tools, Skills, MCP) gains:

- Checkboxes next to each item (checked = currently enabled)
- An "[Apply]" button that calls `agent.update_capabilities`
- A "[Reset to default]" button that clears the overlay (calls update with AgentDef defaults)
- Visual indicator when overlay is modified but not yet applied
- On page load: call `agent.get_capabilities` to restore state

The available list + current effective list both come from `CapabilitiesState`.

## Files changed

| File | Change |
|------|--------|
| `crates/vol-llm-agent-protocol/src/agent_server_protocol.rs` | Add `GetCapabilities`, `UpdateCapabilities` to `AgentOperation`; add `CapabilitiesPayload` |
| `crates/vol-llm-runtime/src/lib.rs` | Add `capability_overlays` field to `AgentRuntime`; add overlay management methods |
| `crates/vol-agent-server/src/data_plane/handlers/capability.rs` | **New file** — `CapabilityHandler` |
| `crates/vol-agent-server/src/data_plane/core.rs` | Register `CapabilityHandler` in handler registry |
| `crates/vol-llm-agent/src/react/mod.rs` or run loop | Add overlay version check before LLM call; rebuild filtered registry on change |
| `crates/vol-llm-ui/src/web/components/tools_panel.rs` | Add checkboxes + apply button |
| `crates/vol-llm-ui/src/web/components/skills.rs` | Add checkboxes + apply button |
| `crates/vol-llm-ui/src/web/components/mcp_panel.rs` | Add checkboxes + apply button |
| `crates/vol-llm-ui/src/state/mod.rs` | Add `CapabilitiesState` to UI state model |

## Verification

```bash
cargo test -p vol-llm-agent-protocol -p vol-llm-runtime -p vol-agent-server -p vol-llm-agent
```

### New tests

| Layer | Test |
|-------|------|
| Unit | `CapabilityOverlay` — create, update, version bump |
| Unit | `CapabilityHandler` — rejects disallowed tools |
| Unit | `CapabilityHandler` — rejects non-existent tool/skill/server names |
| Unit | `CapabilityHandler` — respects mcps allowlist constraint |
| Unit | `CapabilityHandler` — `get_capabilities` returns AgentDef base when no overlay |
| Unit | `CapabilityHandler` — `update_capabilities` creates overlay on first call |
| Integration | Agent loop detects overlay version change → rebuilds tool registry |
| Integration | Agent loop detects overlay version change → rebuilds skill injector → system prompt changes |
| Integration | Agent loop does NOT rebuild when version unchanged |
| Integration | Overlay cleanup on session close |
