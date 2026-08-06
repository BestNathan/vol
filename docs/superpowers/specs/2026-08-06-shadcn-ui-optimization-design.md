# Shadcn/ui Component Optimization Design

**Date:** 2026-08-06
**Status:** draft

## Goal

Replace all custom UI primitives in `frontend/src/components/` with their shadcn/ui equivalents. Principle: 应换尽换 (replace everything replaceable).

## Scope

### New primitives to install

```bash
npx shadcn@latest add switch sheet tooltip skeleton accordion
```

### Replacement map

| Current | Replace with | Affected files |
|---------|-------------|----------------|
| Raw `<button>` elements | `<Button>` / `<Button variant>` | 10+ files |
| `layout/TabBar.tsx` + `layout/TabContent.tsx` | `<Tabs>` (installed, unused) | layout/*, App.tsx |
| `CapabilityDrawer.tsx` custom drawer | `<Sheet>` | CapabilityDrawer.tsx |
| `CapabilityDrawer.tsx` custom toggle | `<Switch>` | CapabilityDrawer.tsx |
| `CapabilityDrawer.tsx` collapsible sections | `<Accordion>` | CapabilityDrawer.tsx |
| `CapabilityDrawer.tsx` raw `<input>` | `<Input>` | CapabilityDrawer.tsx |
| Raw `<input type="checkbox">` | `<Checkbox>` | LogViewer.tsx |
| Custom anchor-zone `<span>` badges | `<Badge>` | ContextPanel.tsx |
| Raw `overflow-y-auto` divs | `<ScrollArea>` | All panel components |
| Loading spinners (`animate-spin` spans) | `<Skeleton>` | SessionsPanel, LogViewer, ContextPanel, CapabilityDrawer |

### NOT replaced (no shadcn equivalent)

- `Markdown.tsx` — react-markdown wrapper
- `ConnectionIndicator.tsx` — domain-specific WS status
- `NodesDropdown.tsx` — node-specific data dropdown
- `ConversationView.tsx` — domain chat view
- `FileTree.tsx` / `FileContentView.tsx` — domain file views
- All dialog-content components (already use `<Dialog>` correctly)

## Button variants mapping

Custom button styles → shadcn variants:

| Current custom style | shadcn variant |
|---------------------|----------------|
| `bg-[#408040] hover:bg-[#50a050]` (Approve/Resume) | `variant="success"` (custom, add to button.tsx) |
| `bg-[#804040] hover:bg-[#905050]` (Reject) | `variant="destructive"` |
| `text-[#4080ff] hover:underline` (Back link) | `variant="link"` |
| `text-yellow-400 hover:text-destructive/80` (Cancel) | `variant="ghost"` + color |
| `text-muted-foreground/60 hover:text-yellow-400/70` (+New Session) | `variant="ghost"` |

## CapabilityDrawer rewrite plan

`CapabilityDrawer.tsx` (404 lines) → rebuild with:
- **`<Sheet>`** — replaces the custom fixed right panel + backdrop (saves ~80 lines of positioning/animation code)
- **`<Switch>`** — replaces the hand-built toggle (`role="switch"` button with inner span, ~20 lines → `<Switch checked={...} />`)
- **`<Accordion>`** — replaces custom collapsible sections (~30 lines of collapse state management)
- **`<Input>`** — replaces raw search `<input>` with search icon

Estimated: 404 → ~220 lines (~45% reduction).

## ScrollArea adoption

Replace `overflow-y-auto` in panel containers with `<ScrollArea>` for consistent scrollbar styling. Key files:
- `SessionsPanel.tsx`
- `LogViewer.tsx`
- `ContextPanel.tsx`
- `AgentsPanel.tsx`
- `McpPanel.tsx`
- `SkillsPanel.tsx`
- `TasksPanel.tsx`
- `ToolsTab.tsx`
- `NodesPanel.tsx`
- `NodeDetailPanel.tsx`

## Skill update

Update `vol-web-dev/SKILL.md`:
- Add "shadcn/ui Usage" section referencing `components/ui/` primitives
- Document the `npx shadcn@latest add <name>` workflow
- Reference components.json configuration
- Cross-reference shadcn docs (https://ui.shadcn.com/docs/components)

## Risk assessment

- **Low risk**: Button, Checkbox, Badge, Input replacements — mechanical, same behavior
- **Medium risk**: Tabs, ScrollArea — structural changes but well-tested shadcn primitives
- **Higher risk**: CapabilityDrawer → Sheet rewrite — largest change, needs testing; functionality is self-contained so blast radius is limited to the capability drawer only
- **No risk**: Skill doc update — documentation only
