# Small-Screen Responsive Lists Design

**Date**: 2026-05-31
**Status**: Draft
**Author**: BestNathan

## Requirements

Apply the SkillsPanel dual-layout pattern (`sm:hidden` cards / `hidden sm:table`) to all remaining list components in the web frontend, so every list is usable on screens narrower than 480px.

### Goals

- Every data-list panel must have both a desktop (table/row) and mobile (card) layout
- Card style follows SkillsPanel conventions: rounded border, click highlight, truncated text
- No new abstractions — each component owns its two layouts inline
- Existing desktop behavior unchanged

### Non-Goals

- Changing the tab navigation or overall app layout for small screens
- Adding a bottom nav bar or other mobile-specific chrome
- Responsive design for the conversation view or input area

---

## Design

### Pattern (from SkillsPanel)

```
Desktop: hidden sm:table w-full border-collapse
Mobile:  sm:hidden flex flex-col gap-2
```

Card wrapper: `border rounded-lg p-3 cursor-pointer hover:bg-accent/50 transition-colors`

---

### 1. SessionsPanel

**File:** `crates/vol-llm-ui/src/web/components/sessions_panel.rs`

**Current:** Pure `<table>` with columns Session ID | Entries | Created | Action

**After:**

Desktop — table (unchanged layout):
```
Session ID         | Entries  | Created    | Action
abc123def...       | 12       | 2h ago     | Resume
```

Mobile — cards:
```
┌──────────────────────────────────┐
│ abc123def456...       12 entries │
│ 2 hours ago            [Resume]  │
└──────────────────────────────────┘
```

Card layout:
- Row 1: `flex justify-between items-center` — truncated session_id (left) + entry_count badge (right, `bg-muted rounded-full px-2 py-0.5 text-xs`)
- Row 2: `flex justify-between items-center mt-1.5` — relative time text (left, `text-muted-foreground text-xs`) + Resume button (right, `text-xs px-3 py-1`)

---

### 2. ToolsTabContent

**File:** `crates/vol-llm-ui/src/web/components/tools_tab.rs`

**Current:** Two sections — "Available Tools" list and "Tool Call History" list, both as table-like rows.

**After:**

**Available Tools — mobile cards:**
```
┌──────────────────────────────────┐
│ 📦 skill                  [Run]  │
│ Auto-discovered skill tool       │
└──────────────────────────────────┘
```
Card layout:
- Row 1: tool name (left, bold, truncated) + Run button (right, small)
- Row 2: description (optional, `text-muted-foreground text-xs truncate`)

**Tool Call History — mobile cards:**
```
┌──────────────────────────────────┐
│ #3  ✓  skill            120ms   │
│ ▶ args: {"q":"eth"}             │
└──────────────────────────────────┘
```
Card layout:
- Row 1: `flex gap-2 items-center` — sequence_num + status_icon(✓ green/✗ red) + tool_name + duration(right-aligned)
- Row 2: expandable arguments preview (click to toggle, monospace `text-xs`)

---

### 3. McpPanel (4 sub-lists)

**File:** `crates/vol-llm-ui/src/web/components/mcp_panel.rs`

All four sub-tabs (Servers, Tools, Resources, Prompts) follow the same dual-layout pattern.

**Servers — mobile cards:**
```
┌──────────────────────────────────┐
│ ● connected   my-mcp-server      │
│ http://localhost:8080            │
└──────────────────────────────────┘
```
Card layout:
- Row 1: status indicator (● green / ○ gray) + server name
- Row 2: URL or config detail (`text-muted-foreground text-xs truncate`)

**Tools — mobile cards:**
```
┌──────────────────────────────────┐
│ read_file                       │
│ Read contents of a file          │
└──────────────────────────────────┘
```
Card layout:
- Row 1: tool name (bold, truncated)
- Row 2: description (optional, `text-xs text-muted-foreground`)

**Resources — mobile cards:**
```
┌──────────────────────────────────┐
│ file:///docs/api.md              │
│ API Documentation                │
└──────────────────────────────────┘
```
Card layout:
- Row 1: URI (`font-mono text-xs truncate`)
- Row 2: resource name or description

**Prompts — mobile cards:**
```
┌──────────────────────────────────┐
│ code_review                      │
│ Review code for best practices   │
└──────────────────────────────────┘
```
Card layout:
- Row 1: prompt name (bold)
- Row 2: description (optional, `text-xs text-muted-foreground truncate`)

---

### 4. AgentsPanel (already responsive, minor tweaks)

**File:** `crates/vol-llm-ui/src/web/components/agents_panel.rs`

Current state already works on small screens (`w-full sm:w-auto`, `flex-col sm:flex-row`). No layout change needed beyond visual consistency — ensure the agent cards use the same `border rounded-lg` style as other list cards.

---

## Crate / File Structure

```
crates/vol-llm-ui/src/web/components/
├── sessions_panel.rs     # MODIFIED: add mobile card layout
├── tools_tab.rs          # MODIFIED: add mobile card layouts for tools + history
├── mcp_panel.rs          # MODIFIED: add mobile card layouts for 4 sub-lists
└── agents_panel.rs       # MODIFIED: minor style consistency tweaks
```

---

## Testing Strategy

- Open each tab on a viewport narrower than 480px, verify cards render
- Open each tab on a viewport wider than 480px, verify tables unchanged
- Resize browser across the 480px boundary, verify layout switches correctly
- Test on real phone (port 8080 LAN access)
