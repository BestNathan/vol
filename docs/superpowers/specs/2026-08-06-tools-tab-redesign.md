# Tools Tab Redesign

**Date:** 2026-08-06
**Status:** spec

## Purpose

Redesign the Tools tab to focus exclusively on displaying the current DP node's tool list with search filtering and inline Run actions. Remove Call History (which is per-agent, not per-node).

## Scope

- `frontend/src/components/panels/ToolsTab.tsx` — full rewrite
- `frontend/src/stores/tools.ts` — remove `toolCallsAtom` import from ToolsTab (atom stays for event-handlers)

## UI Layout

```
┌─────────────────────────────────┐
│ 🔧 Tools (12)         [Refresh] │  Header bar
├─────────────────────────────────┤
│ 🔍 Search tools...              │  Search input
├─────────────────────────────────┤
│ ┌─────────────────────────────┐ │
│ │ 🔧 tool_name     Safe  [Run]│ │  Tool row (compact)
│ │   description text...       │ │  Description, single-line truncate
│ ├─────────────────────────────┤ │
│ │ ⚠️ risky_tool   Approv [Run]│ │
│ │   does sensitive things...  │ │
│ └─────────────────────────────┘ │
│                                 │
│   No tools match "xxx"          │  Empty/search-empty state
└─────────────────────────────────┘
```

## Component Design

### `ToolsTab`

**Props:** none (reads state from atoms).

**State:**
- `tools: {name, description, parameters?}[]` — from `systemToolsAtom`
- `loading: boolean` — from `toolsLoadingAtom`
- `loadError: string | null` — local state
- `search: string` — local state, controls filter
- `dialogTool: SystemTool | null` — which tool's Run dialog is open

**Derived:** `filteredTools` = tools filtered by search (name + description, case-insensitive).

**Key behaviors:**
- Fetch `tool.list` on mount (skip if cached); Refresh button re-fetches
- Search filters in real-time as user types
- Click "Run" → opens `ToolCallDialog` (existing component, reused)
- Empty state: "No tools available" (no tools from server) vs "No tools match `<search>`" (search filter)

**Layout:**
1. Header row: "🔧 Tools (N)" label + Refresh Button (secondary, sm)
2. Search input: Input component with Search icon, full width
3. Tool list: ScrollArea wrapping compact rows
4. ToolCallDialog: rendered at bottom, same as before

### `ToolRow`

Each row:
- Left: icon (Wrench/lucide for safe tools, AlertTriangle for approval-required) + name (semibold) + sensitivity Badge (Safe green / Approval yellow)
- Right: Run Button (secondary, sm)
- Below name: description in `text-[12px] text-muted-foreground truncate`, with tooltip on hover

### States

| State | Display |
|-------|---------|
| Loading | Skeleton rows (3-4 placeholders) |
| Load error | Error banner with retry |
| Empty — no tools | Centered "No tools available" message |
| Empty — search filter | "No tools match `<search>`" |
| Normal | Tool rows as above |

## Data Flow

```
ToolsTab
  ├─ mount → getPanelClient().call('tool.list') → systemToolsAtom
  ├─ search: local useState, no RPC needed
  ├─ Run click → setDialogTool(tool) → ToolCallDialog renders
  │    └─ Execute → getPanelClient().call('tool.call', ...) → display result/error
  └─ Refresh → re-fetch tool.list
```

**No changes to backend.** `tool.list` and `tool.call` already work through `getAgentClient()` which routes to the active DP node in CP mode.

## Removals

1. **Call History section** — entire block removed from ToolsTab
2. **`CallHistoryItem` component** — removed from file
3. **`statusBadge`** — remove. Only consumed by `CallHistoryItem` which is being removed.
4. **`formatToolCallResult`** — keep inline in ToolsTab. Still needed by `executeTool` to format `tool.call` results for display in `ToolCallDialog`.
5. **`toolCallsAtom`** — keep the atom and `event-handlers.ts` writes as-is; ToolsTab simply stops reading it. The atom is harmless and can be cleaned up later when call-history is migrated to a per-agent view.

## shadcn/ui Components Used

All already installed:
- `Input` — search box
- `Button` — Run, Refresh
- `Badge` — sensitivity tags
- `ScrollArea` — list scroll
- `Tooltip` — description hover
- `Skeleton` — loading placeholders
- `Dialog` (via ToolCallDialog) — Run dialog

## Non-Goals

- No per-node tool cache (same as current — re-fetches on mount/refresh, single `systemToolsAtom`)
- No grouping/sorting beyond alphabetical (server already sorts by name)
- No change to `ToolCallDialog` component
- No backend changes
