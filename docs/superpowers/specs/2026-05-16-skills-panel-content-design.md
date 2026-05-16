# Skills Panel Content Design

**Date:** 2026-05-16
**Status:** Draft
**Scope:** Backend JSON-RPC + Web UI

## Problem

The Skills panel in the web frontend shows "No skills discovered" because:

1. No JSON-RPC method exists to list or retrieve skills.
2. The `SkillsPanel` component initializes with an empty `SkillsState`.
3. There is no `UiEvent` type to populate skills from the backend.

The `SkillLoader` in `vol-llm-skill` already discovers and caches skills from `.agents/skills/` directories — it just needs to be exposed.

## Architecture

### Backend: JSON-RPC Server

Two new RPC methods:

| Method | Parameters | Returns |
|--------|-----------|---------|
| `skill.list` | `{ id: number }` | `Vec<SkillListEntry>` |
| `skill.get` | `{ id: number, name: String }` | `SkillDetail` |

**RPC data types:**

```rust
struct SkillListEntry {
    id: String,         // "user:skill-name" or "repo:skill-name"
    name: String,       // "brainstorming"
    version: String,    // "1.0.0"
    scope: String,      // "User" or "Repo"
    description: String,
    triggers: Vec<String>,
}

struct SkillDetail {
    name: String,
    version: String,
    scope: String,
    description: String,
    triggers: Vec<String>,
    content: String,           // full SKILL.md body
    file_listing: Vec<String>, // relative paths like ["SKILL.md", "references/style.md"]
}
```

### SkillLoader Integration

`JsonRpcServer::new()` gains an optional `Arc<SkillLoader>` parameter:

```rust
pub async fn new(
    agents: Vec<AgentRegistration>,
    working_dir: String,
    store_dir: String,
    mcp_manager: Option<Arc<McpManager>>,
    skill_loader: Option<Arc<SkillLoader>>,  // NEW
) -> Self
```

When `skill_loader` is `Some`, the `skill.list` and `skill.get` handlers call `loader.list_metadata()` and `loader.get(name)` respectively.

When `None`, `skill.list` returns `[]` (empty list) — the panel shows "No skills discovered".

### Data Flow

1. **Server startup** — `jsonrpc_agent_service.rs` creates `SkillLoader::new(Some(working_dir))`, calls `discover_all()`, wraps in `Arc`, passes to `JsonRpcServer::new()`.
2. **Frontend mount** — `SkillsPanel` calls `rpc_client.send_request("skill.list")`. Signal populates with entries.
3. **Row click** — User clicks a row. Frontend calls `rpc_client.send_request("skill.get", { name })`. Opens `SkillDetailDialog` modal.
4. **Dialog close** — User dismisses. Dialog state clears. No server call needed.

## Frontend Components

### SkillsPanel Changes

- Replace `use_signal(|| SkillsState::new())` with a signal that calls RPC on mount.
- Add `error: Option<String>` to `SkillsState` for fetch failures with retry button.
- Add click handler on each row: fetch detail via RPC, open dialog.
- Dialog signal managed at `App` level (same pattern as `McpDialogState`).

### New SkillDetailDialog Component

Modal overlay showing:

- Header: skill name + version badge + scope badge
- Triggers as tag pills
- Description paragraph
- Scrollable `pre`/`code` block for SKILL.md body
- File listing table at the bottom
- Close button

State shape:

```rust
pub struct SkillDialogState {
    pub open: bool,
    pub skill: Option<SkillDetail>,
    pub loading: bool,
}
```

Dialog signal managed at `App` level, passed via context to `SkillsPanel` and `SkillDetailDialog`.

## Error Handling

| Scenario | Behavior |
|----------|----------|
| No SkillLoader configured | `skill.list` returns `[]`. Panel shows "No skills discovered". |
| Skill not found | `skill.get` returns JSON-RPC error `{"code": -32000, "message": "Skill 'X' not found"}`. |
| Frontend fetch failure | Panel shows "Failed to load skills" with retry button. |
| Dialog load failure | Dialog shows "Failed to load skill details" inside modal body. User can close or retry. |

## Testing

- **Unit test:** Verify `skill.list` and `skill.get` parse correctly from JSON-RPC requests in `serde_helpers.rs`.
- **Integration test:** In `jsonrpc_integration.rs`, test round-trip: send `skill.list`, verify response structure.
- **Existing tests:** `SkillLoader` unit tests in `vol-llm-skill` already cover discovery logic.

## Files Changed

| File | Change |
|------|--------|
| `crates/vol-llm-skill/src/def.rs` | Derive `Serialize` on `SkillMetadata`, `SkillDef` (if not already) |
| `crates/vol-llm-agent-channel/src/jsonrpc/serde_helpers.rs` | Add `SkillList`, `SkillGet` variants to `JsonRpcRequest` enum |
| `crates/vol-llm-agent-channel/src/jsonrpc/connection.rs` | Add handlers for `skill.list` and `skill.get` |
| `crates/vol-llm-agent-channel/src/jsonrpc/server.rs` | Accept `Option<Arc<SkillLoader>>` in `JsonRpcServer::new()` |
| `crates/vol-llm-agent-channel/src/lib.rs` | Re-export `SkillLoader` |
| `crates/vol-llm-agent-channel/examples/jsonrpc_agent_service.rs` | Create `SkillLoader`, pass to server |
| `crates/vol-llm-ui/src/state/mod.rs` | Add `SkillDetail` struct, update `SkillsState` |
| `crates/vol-llm-ui/src/web/components/skills.rs` | Add RPC fetch on mount, row click handler, dialog open |
| `crates/vol-llm-ui/src/web/components/app.rs` | Add `SkillDialogState` signal, render `SkillDetailDialog` |
| `crates/vol-llm-ui/src/web/client.rs` | Add `SkillDetail` RPC response type |
