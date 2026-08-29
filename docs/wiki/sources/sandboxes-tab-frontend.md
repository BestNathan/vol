---
type: source
source_type: code
date: 2026-08-25
ingested: 2026-08-25
tags: [frontend, sandbox, ui, web, rpc, react]
---

# Sandboxes Tab in Web Frontend

**Authors/Creators:** Vol team
**Date:** 2026-08-25
**Link:** frontend/src/components/panels/SandboxesPanel.tsx, crates/vol-agent-server/src/data_plane/handlers/sandbox.rs

## TL;DR
Added a new "Sandboxes" tab to the React web frontend (between MCP and Logs) to display all registered sandboxes on the active data-plane node. Backend `SandboxHandler` was refactored to accept the full `SandboxRegistry` instead of a single sandbox instance, enabling `sandbox.list` to return all registered sandboxes.

> **Superseded (2026-08-28):** `SandboxRegistry` was deleted; `SandboxHandler` now takes `Arc<SandboxManager>`. The frontend panel and the `sandbox.list` RPC are unchanged. See [[sandbox-registry-manager-unification]].

## Key Takeaways
- `SandboxHandler` now holds `Arc<SandboxRegistry>` instead of `Arc<dyn Sandbox>`
- `sandbox.list` iterates `registry.names()` and returns `Vec<SandboxInfo>` for all registered sandboxes
- Other sandbox ops (exec, read_file, etc.) still use `registry.default()` for backward compatibility
- Frontend adds `SandboxesPanel` component with `kindBadgeClass` for color-coded kind badges (local=green, ssh=blue, tmp=gray, firecracker=orange, wasm=purple)
- New RPC method `sandbox.list` added to frontend protocol types
- Integration tests updated to use registry instead of single sandbox

## Detailed Summary

### Backend Changes

**`crates/vol-agent-server/src/data_plane/handlers/sandbox.rs`**
- `SandboxHandler::new()` signature changed: `new(registry: Arc<SandboxRegistry>)` instead of `new(sandbox: Arc<dyn Sandbox>)`
- `sandbox.list` handler: iterates `self.registry.names()`, calls `self.registry.get(name)` for each, builds `Vec<SandboxInfo>` with `name`, `kind`, `root_path`
- All other operations (exec, read_file, write_file, create_dir, read_dir, metadata) use `self.registry.default()` to maintain backward compatibility
- Tests updated: `setup()` creates a registry with a "local" sandbox registered; `test_handler_name` and `test_operations_count` use empty registry

**`crates/vol-agent-server/src/data_plane/core.rs`**
- Line 506: `SandboxHandler::new(sandbox_registry.default())` → `SandboxHandler::new(sandbox_registry.clone())`
- Handler now has access to all registered sandboxes, not just the default

**`crates/vol-agent-server/src/control_plane/core.rs`**
- Lines 48-56: Control plane also updated to create a `SandboxRegistry`, register "local" sandbox, and pass registry to `SandboxHandler::new()`
- Uses `/tmp/vol-control-plane-sandboxes` as the registry load directory

**`crates/vol-agent-server/tests/sandbox_protocol_integration.rs`**
- `create_test_server()`: creates registry, registers "local" sandbox, passes `Arc::new(registry)` to `SandboxHandler::new()`
- Handler registry variable renamed from `registry` to `handler_reg` to avoid name collision with sandbox registry

### Frontend Changes

**`frontend/src/types/index.ts`**
- Added `'sandboxes'` to `ActiveTab` union type (line 2)
- Added `SandboxInfo` interface: `{ name: string; kind: string; root_path: string }` (lines 145-148)

**`frontend/src/lib/protocol.ts`**
- Added `SandboxInfo` to imports from `@/types` (line 13)
- Added `'sandbox.list'` RPC method: `{ params: object; result: { sandboxes: SandboxInfo[] } }` (line 158)

**`frontend/src/stores/sandboxes.ts`** (new file)
- Jotai atom for `SandboxesState`: `{ sandboxes: SandboxInfo[]; loading: boolean; error: string | null }`
- Initial state: `loading: true`

**`frontend/src/components/panels/SandboxesPanel.tsx`** (new file)
- Main panel component with loading/error/empty states
- `kindBadgeClass(kind)` helper: returns Tailwind classes for color-coded kind badges
  - local: emerald (green)
  - ssh: blue
  - tmp: secondary (gray)
  - firecracker: orange
  - wasm: purple
  - unknown: secondary
- Responsive layout: mobile cards, desktop rows
- Shows sandbox name, kind badge, and root_path (truncated on desktop)
- Retry button on error

**`frontend/src/components/layout/TabBar.tsx`**
- Added `{ id: 'sandboxes', label: 'Sandboxes' }` after MCP in `TABS` array (line 13)

**`frontend/src/components/layout/TabContent.tsx`**
- Added `SandboxesPanel` import (line 8)
- Added `'sandboxes'` to `TABS` array (line 14)
- Added `sandboxes: SandboxesPanel` to `PANELS` record (line 22)

**`frontend/tests/unit/sandboxes-panel.test.ts`** (new file)
- 6 tests for `kindBadgeClass` helper: verifies correct color classes for each kind type

### Testing

**Backend**
- Unit tests: 10 passed (all existing sandbox handler tests updated to use registry)
- Integration tests: 4 passed (sandbox_protocol_integration.rs)

**Frontend**
- Unit tests: 150 passed (144 existing + 6 new for sandboxes-panel)
- TypeScript: no errors

## Entities Mentioned
- [[vol-agent-server-crate]]: SandboxHandler refactored to use registry
- [[vol-llm-sandbox-crate]]: SandboxRegistry now exposed via protocol
- [[vol-llm-agent-protocol-crate]]: sandbox.list RPC method (already existed, now used by frontend)

## Concepts Covered
- [[sandbox-lifecycle]]: Registry now fully exposed to frontend via sandbox.list
- [[jsonrpc-websocket]]: sandbox.list method added to frontend RPC client
- [[react-pattern]]: SandboxesPanel follows same pattern as McpPanel (loading/error/empty states, responsive layout)

## Notes
- Other sandbox ops (exec, read_file, etc.) still use `registry.default()` — no UI calls these yet; agents use them via `ToolContext`
- Control plane creates a minimal registry with just "local" sandbox — no TOML configs loaded
- Frontend tab order: Tasks, Agents, Tools, Workspace, Skills, MCP, **Sandboxes**, Logs
- Coverage: backend handler tests pass, frontend unit tests pass; no coverage gate run (not required for this change)
