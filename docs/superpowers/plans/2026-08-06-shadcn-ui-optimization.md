# Shadcn/ui Component Optimization — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace all custom UI primitives with shadcn/ui equivalents — buttons, tabs, drawer, toggles, checkboxes, badges, scroll areas, loading states.

**Architecture:** Incremental replacement with zero functional regressions. Each task targets one component type across a file group. The CapabilityDrawer gets the largest rewrite (Sheet + Switch + Accordion). All other changes are mechanical 1:1 substitutions.

**Tech Stack:** React 18, TypeScript 5.6, Tailwind CSS v4, shadcn/ui (Radix primitives), Jotai state

## Global Constraints

- Zero visual regressions — same colors, sizes, spacing
- Zero functional regressions — all click handlers, aria attributes, disabled states preserved
- No changes to stores, hooks, lib, or types
- No changes to domain-specific components (Markdown, ConnectionIndicator, NodesDropdown, ConversationView, FileTree, FileContentView, NodeDetailPanel)
- Existing dialog-content components keep their `<Dialog>` usage as-is
- All shadcn primitives installed via `npx shadcn@latest add`

---

### Task 1: Install new shadcn/ui primitives + add Button success variant

**Files:**
- Create: `frontend/src/components/ui/switch.tsx`
- Create: `frontend/src/components/ui/sheet.tsx`
- Create: `frontend/src/components/ui/skeleton.tsx`
- Create: `frontend/src/components/ui/accordion.tsx`
- Create: `frontend/src/components/ui/tooltip.tsx`
- Modify: `frontend/src/components/ui/button.tsx` (add success variant)

**Interfaces:**
- Produces: `Switch`, `Sheet`/`SheetContent`/`SheetHeader`/`SheetTitle`, `Skeleton`, `Accordion`/`AccordionItem`/`AccordionTrigger`/`AccordionContent`, `Tooltip`/`TooltipTrigger`/`TooltipContent`
- Produces: `Button` with new `variant="success"` — `bg-emerald-600 text-white hover:bg-emerald-700 shadow-sm`

- [ ] **Step 1: Install primitives via shadcn CLI**

```bash
cd /root/vol/frontend && npx shadcn@latest add switch sheet skeleton accordion tooltip
```

Expected: 5 new files created under `src/components/ui/`.

- [ ] **Step 2: Add success variant to Button**

Edit `frontend/src/components/ui/button.tsx` — add the `success` variant to the `cva` definition:

```tsx
const buttonVariants = cva(
  "inline-flex items-center justify-center gap-2 whitespace-nowrap rounded-md text-sm font-medium transition-colors focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring disabled:pointer-events-none disabled:opacity-50 [&_svg]:pointer-events-none [&_svg]:size-4 [&_svg]:shrink-0",
  {
    variants: {
      variant: {
        default:
          "bg-primary text-primary-foreground shadow hover:bg-primary/90",
        destructive:
          "bg-destructive text-destructive-foreground shadow-sm hover:bg-destructive/90",
        outline:
          "border border-input bg-background shadow-sm hover:bg-accent hover:text-accent-foreground",
        secondary:
          "bg-secondary text-secondary-foreground shadow-sm hover:bg-secondary/80",
        ghost: "hover:bg-accent hover:text-accent-foreground",
        link: "text-primary underline-offset-4 hover:underline",
        success:
          "bg-emerald-600 text-white shadow-sm hover:bg-emerald-700",
      },
      // ... size variants unchanged
    },
    defaultVariants: {
      variant: "default",
      size: "default",
    },
  }
)
```

- [ ] **Step 3: Verify build**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
```

Expected: no type errors.

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/ui/switch.tsx frontend/src/components/ui/sheet.tsx \
  frontend/src/components/ui/skeleton.tsx frontend/src/components/ui/accordion.tsx \
  frontend/src/components/ui/tooltip.tsx frontend/src/components/ui/button.tsx
git commit -m "feat: install shadcn switch/sheet/skeleton/accordion/tooltip, add Button success variant"
```

---

### Task 2: Replace raw buttons with `<Button>` in dialogs

**Files:**
- Modify: `frontend/src/components/dialogs/ApprovalDialog.tsx:85-98`
- Modify: `frontend/src/components/dialogs/DebugPanel.tsx:55-60`
- Modify: `frontend/src/components/dialogs/SkillDetailDialog.tsx:69-79` (version/scope badges → `<Badge>`)
- Modify: `frontend/src/components/dialogs/ContextDialog.tsx` (check for raw buttons)

**Interfaces:**
- Consumes: `Button` with `success` variant from Task 1, `Badge` (already installed)

- [ ] **Step 1: ApprovalDialog — Approve/Reject buttons**

Replace raw buttons with `<Button>`:

```tsx
// OLD (lines 85-98):
<button type="button" onClick={() => void resolve(true)}
  className="px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#408040] text-foreground hover:bg-[#50a050]">
  Approve
</button>
<button type="button" onClick={() => void resolve(false)}
  className="px-3 py-1.5 border-none rounded-md cursor-pointer text-[13px] bg-[#804040] text-foreground hover:bg-[#905050]">
  Reject
</button>

// NEW:
<Button variant="success" size="sm" onClick={() => void resolve(true)}>Approve</Button>
<Button variant="destructive" size="sm" onClick={() => void resolve(false)}>Reject</Button>
```

Add `import { Button } from '@/components/ui/button'` at top, remove the old `DialogContent` only import pattern (check existing imports).

- [ ] **Step 2: DebugPanel — WS tab button**

Replace the raw `<button>` WS tab with `<Button>`:

```tsx
// OLD (line 55-58):
<button type="button"
  className="px-3 py-1 text-[12px] font-semibold cursor-pointer border-b-2 border-primary text-foreground">
  WS
</button>

// NEW:
<Button variant="ghost" size="sm" className="border-b-2 border-primary rounded-none font-semibold text-[12px]">WS</Button>
```

Add `import { Button } from '@/components/ui/button'`.

- [ ] **Step 3: SkillDetailDialog — version and scope badges**

Replace custom `<span>` badges with `<Badge>`:

```tsx
// OLD version badge (line 69-71):
<span className="text-[11px] text-muted-foreground bg-secondary px-1.5 py-0.5 rounded flex-shrink-0">
  v{skill.version}
</span>

// NEW:
<Badge variant="secondary" className="text-[11px] flex-shrink-0">v{skill.version}</Badge>

// OLD scope badge (line 74-79):
<span className="text-[11px] px-1.5 py-0.5 rounded flex-shrink-0"
  style={{ color: scopeColor(skill.scope), background: '#2a2a44' }}>
  {skill.scope}
</span>

// NEW:
<Badge variant="outline" className="text-[11px] flex-shrink-0"
  style={{ color: scopeColor(skill.scope), borderColor: scopeColor(skill.scope) }}>
  {skill.scope}
</Badge>
```

Add `import { Badge } from '@/components/ui/badge'`.

- [ ] **Step 4: Verify typecheck**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
```

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/dialogs/ApprovalDialog.tsx \
  frontend/src/components/dialogs/DebugPanel.tsx \
  frontend/src/components/dialogs/SkillDetailDialog.tsx
git commit -m "refactor: replace raw buttons/badges with shadcn Button/Badge in dialogs"
```

---

### Task 3: Replace raw buttons and checkbox in panels

**Files:**
- Modify: `frontend/src/components/panels/SessionsPanel.tsx:115-125` (Resume button)
- Modify: `frontend/src/components/panels/LogViewer.tsx:266-282` (Back button, auto-scroll checkbox)
- Modify: `frontend/src/components/panels/ContextPanel.tsx:130-135` (anchor-zone badges)
- Modify: `frontend/src/components/panels/AgentsPanel.tsx` (check for raw buttons)
- Modify: `frontend/src/components/panels/TasksPanel.tsx` (check for raw buttons)
- Modify: `frontend/src/components/panels/ToolsTab.tsx` (check for raw buttons)
- Modify: `frontend/src/components/panels/NodesPanel.tsx` (check for raw buttons)

**Interfaces:**
- Consumes: `Button` with `success` variant from Task 1, `Checkbox`, `Badge` (installed)

- [ ] **Step 1: Read each panel file, locate all raw `<button>` elements**

```bash
grep -n '<button' /root/vol/frontend/src/components/panels/*.tsx
```

- [ ] **Step 2: SessionsPanel — Resume button**

Replace raw button with `<Button variant="success" size="sm">`:

```tsx
// OLD (line 115-125):
<button type="button"
  className="px-2.5 py-0.5 bg-[#408040] text-foreground border-none rounded-[3px] cursor-pointer text-[12px] flex-shrink-0 hover:bg-[#50a050] disabled:bg-[#333355] disabled:cursor-not-allowed"
  disabled={resumingId !== null}
  onClick={...}>
  {resumingId === session.id ? 'Resuming...' : 'Resume'}
</button>

// NEW:
<Button variant="success" size="sm" className="text-[12px] flex-shrink-0"
  disabled={resumingId !== null}
  onClick={...}>
  {resumingId === session.id ? 'Resuming...' : 'Resume'}
</Button>
```

- [ ] **Step 3: LogViewer — "← Back to run list" button**

Replace with `<Button variant="link" size="sm">`:

```tsx
// OLD (line 266-272):
<button type="button"
  className="text-[#4080ff] hover:underline text-[12px] cursor-pointer whitespace-nowrap"
  onClick={backToList}>
  ← Back to run list
</button>

// NEW:
<Button variant="link" size="sm" className="text-[12px] whitespace-nowrap"
  onClick={backToList}>
  ← Back to run list
</Button>
```

- [ ] **Step 4: LogViewer — Auto-scroll checkbox**

Replace raw `<input type="checkbox">` with `<Checkbox>`:

```tsx
// OLD (lines 275-283):
<label className="ml-auto flex items-center gap-1.5 text-[12px] text-muted-foreground whitespace-nowrap flex-shrink-0 cursor-pointer">
  <input type="checkbox" className="accent-[#80a0ff] cursor-pointer"
    checked={autoScroll}
    onChange={(e) => setAutoScroll(e.target.checked)} />
  Auto-scroll
</label>

// NEW:
<label className="ml-auto flex items-center gap-1.5 text-[12px] text-muted-foreground whitespace-nowrap flex-shrink-0 cursor-pointer">
  <Checkbox checked={autoScroll}
    onCheckedChange={(checked) => setAutoScroll(checked === true)} />
  Auto-scroll
</label>
```

Add `import { Checkbox } from '@/components/ui/checkbox'`, add `import { Button } from '@/components/ui/button'`.

- [ ] **Step 5: ContextPanel — anchor-zone badges**

Replace custom `<span>` with `<Badge>`:

```tsx
// OLD (line 130-135):
<span className="text-[9px] font-bold px-1.5 py-0.5 rounded flex-shrink-0"
  style={{ color: anchorZoneColor(c.anchor_zone), background: '#2a2a44' }}>
  {c.anchor_zone}
</span>

// NEW:
<Badge variant="outline" className="text-[9px] font-bold flex-shrink-0"
  style={{ color: anchorZoneColor(c.anchor_zone), borderColor: anchorZoneColor(c.anchor_zone) }}>
  {c.anchor_zone}
</Badge>
```

Add `import { Badge } from '@/components/ui/badge'`.

- [ ] **Step 6: Scan and replace remaining raw buttons in other panels**

Check `AgentsPanel.tsx`, `TasksPanel.tsx`, `ToolsTab.tsx`, `NodesPanel.tsx` for raw `<button>` elements. Replace each with the appropriate `<Button>` variant:
- Filter chips / status toggles → `<Button variant="ghost" size="sm">`
- Primary actions → `<Button variant="default" size="sm">`
- Subtle actions → `<Button variant="outline" size="sm">`

- [ ] **Step 7: Verify typecheck**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
```

- [ ] **Step 8: Commit**

```bash
git add frontend/src/components/panels/
git commit -m "refactor: replace raw buttons/checkbox/badges with shadcn equivalents in panels"
```

---

### Task 4: Replace raw buttons in inputs and layout

**Files:**
- Modify: `frontend/src/components/inputs/InputArea.tsx:162-188` (Cancel, +New Session buttons)
- Modify: `frontend/src/components/layout/StatusBar.tsx:49-56` (debug panel toggle)

- [ ] **Step 1: InputArea — Cancel and +New Session buttons**

```tsx
// Cancel button (lines 162-172) — OLD:
<button type="button" onClick={handleCancel} disabled={!runId}
  className={cn('text-yellow-400 cursor-pointer',
    'hover:text-destructive/80 hover:underline',
    'disabled:text-muted-foreground/70 disabled:cursor-not-allowed disabled:hover:no-underline')}>
  Cancel
</button>

// NEW:
<Button variant="ghost" size="sm" onClick={handleCancel} disabled={!runId}
  className="text-yellow-400 hover:text-destructive/80 text-[10px] sm:text-[11px]">
  Cancel
</Button>

// +New Session button (lines 182-188) — OLD:
<button type="button" onClick={handleNewSession} disabled={isRunning}
  className="text-muted-foreground/60 hover:text-yellow-400/70 hover:underline cursor-pointer disabled:opacity-50 disabled:cursor-not-allowed">
  + New Session
</button>

// NEW:
<Button variant="ghost" size="sm" onClick={handleNewSession} disabled={isRunning}
  className="text-muted-foreground/60 hover:text-yellow-400/70 text-[10px] sm:text-[11px]">
  + New Session
</Button>
```

Add `import { Button } from '@/components/ui/button'`.

- [ ] **Step 2: StatusBar — debug panel toggle**

```tsx
// OLD (lines 49-56):
<button type="button" aria-label="Toggle debug panel" title="Debug panel"
  onClick={() => setDebugPanel((prev) => ({ ...prev, open: !prev.open }))}
  className="hover:text-white cursor-pointer">
  🐛
</button>

// NEW:
<Button variant="ghost" size="icon" aria-label="Toggle debug panel" title="Debug panel"
  onClick={() => setDebugPanel((prev) => ({ ...prev, open: !prev.open }))}
  className="hover:text-white text-[14px]">
  🐛
</Button>
```

Add `import { Button } from '@/components/ui/button'`.

- [ ] **Step 3: Verify typecheck and commit**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
git add frontend/src/components/inputs/InputArea.tsx frontend/src/components/layout/StatusBar.tsx
git commit -m "refactor: replace raw buttons with shadcn Button in InputArea and StatusBar"
```

---

### Task 5: Replace TabBar/TabContent with shadcn `<Tabs>`

**Files:**
- Modify: `frontend/src/components/layout/TabBar.tsx` (rewrite to use TabsList/TabsTrigger)
- Modify: `frontend/src/components/layout/TabContent.tsx` (rewrite to use TabsContent)
- Modify: `frontend/src/App.tsx:129-143` (wrap TabBar+TabContent in `<Tabs>`)

**Interfaces:**
- Consumes: `Tabs`, `TabsList`, `TabsTrigger`, `TabsContent` from `@/components/ui/tabs` (installed)
- Consumes: `activeTabAtom` from `@/stores/ui`, `ActiveTab` from `@/types`

- [ ] **Step 1: Rewrite TabBar.tsx**

Replace the custom `<div>` + `<button>` tab bar with `<TabsList>` + `<TabsTrigger>`:

```tsx
// frontend/src/components/layout/TabBar.tsx
import { useAtom } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import type { ActiveTab } from '@/types'
import { TabsList, TabsTrigger } from '@/components/ui/tabs'

const TABS: { id: ActiveTab; label: string }[] = [
  { id: 'tasks', label: 'Tasks' },
  { id: 'agents', label: 'Agents' },
  { id: 'tools', label: 'Tools' },
  { id: 'workspace', label: 'Workspace' },
  { id: 'skills', label: 'Skills' },
  { id: 'mcp', label: 'MCP' },
  { id: 'logs', label: 'Logs' },
]

export function TabBar() {
  const [active, setActive] = useAtom(activeTabAtom)

  return (
    <TabsList className="flex flex-nowrap bg-card border-b border-border rounded-none h-auto p-0 w-full justify-start overflow-x-auto flex-shrink-0">
      {TABS.map(tab => (
        <TabsTrigger
          key={tab.id}
          value={tab.id}
          onClick={() => setActive(tab.id)}
          className="px-2 sm:px-4 py-1 sm:py-1.5 text-[11px] sm:text-[13px] whitespace-nowrap flex-shrink-0 rounded-none border-b-2 border-transparent data-[state=active]:border-primary data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:shadow-none"
        >
          {tab.label}
        </TabsTrigger>
      ))}
    </TabsList>
  )
}
```

The key: flatten shadcn TabsList styling (`rounded-none h-auto p-0`, no `inline-flex`, use `w-full justify-start`) so it matches the original horizontal tab bar look.

- [ ] **Step 2: Rewrite TabContent.tsx**

Replace the switch statement with `<TabsContent>` wrappers:

```tsx
// frontend/src/components/layout/TabContent.tsx
import { useAtomValue } from 'jotai'
import { activeTabAtom } from '@/stores/ui'
import { TabsContent } from '@/components/ui/tabs'
import { AgentsPanel } from '@/components/panels/AgentsPanel'
import { ToolsTab } from '@/components/panels/ToolsTab'
import { McpPanel } from '@/components/panels/McpPanel'
import { SkillsPanel } from '@/components/panels/SkillsPanel'
import { TasksPanel } from '@/components/panels/TasksPanel'
import { FileContentView } from '@/components/panels/FileContentView'
import { LogViewer } from '@/components/panels/LogViewer'

const TABS = ['tasks', 'agents', 'tools', 'workspace', 'skills', 'mcp', 'logs'] as const

const PANELS: Record<string, React.ComponentType> = {
  tasks: TasksPanel,
  agents: AgentsPanel,
  tools: ToolsTab,
  workspace: FileContentView,
  skills: SkillsPanel,
  mcp: McpPanel,
  logs: LogViewer,
}

export function TabContent() {
  const active = useAtomValue(activeTabAtom)

  return (
    <div className="flex-1 min-h-0 overflow-hidden flex flex-col">
      {TABS.map(tab => {
        const Panel = PANELS[tab]
        if (!Panel) return null
        return (
          <TabsContent key={tab} value={tab} forceMount={tab === active}
            className="flex-1 min-h-0 overflow-hidden mt-0 data-[state=inactive]:hidden">
            {tab === active ? <Panel /> : null}
          </TabsContent>
        )
      })}
    </div>
  )
}
```

The `forceMount` + conditional render pattern keeps the DOM light (only the active tab renders) while maintaining `Tabs` value tracking.

- [ ] **Step 3: Update App.tsx to wrap with `<Tabs>`**

In `App.tsx`, import `Tabs` and wrap `TabBar` + `TabContent`:

```tsx
// Add import:
import { Tabs } from '@/components/ui/tabs'
import { activeTabAtom } from '@/stores/ui'

// In AppInner, add:
const activeTab = useAtomValue(activeTabAtom)

// Replace (line 138-141):
<TabBar />
<TabContent />

// With:
<Tabs value={activeTab} className="flex-1 min-h-0 overflow-hidden flex flex-col">
  <TabBar />
  <TabContent />
</Tabs>
```

- [ ] **Step 4: Verify typecheck**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
```

- [ ] **Step 5: Commit**

```bash
git add frontend/src/components/layout/TabBar.tsx \
  frontend/src/components/layout/TabContent.tsx \
  frontend/src/App.tsx
git commit -m "refactor: replace custom TabBar/TabContent with shadcn Tabs"
```

---

### Task 6: Rewrite CapabilityDrawer with Sheet + Switch + Accordion + Input

**Files:**
- Modify: `frontend/src/components/inputs/CapabilityDrawer.tsx` (404 lines → ~250 lines)
  - Replace custom fixed right panel + backdrop with `<Sheet>`
  - Replace custom toggle buttons with `<Switch>`
  - Replace custom collapsible sections with `<Accordion>`
  - Replace raw search `<input>` with `<Input>`

**Interfaces:**
- Consumes: `Sheet`, `SheetContent`, `SheetHeader`, `SheetTitle` from `@/components/ui/sheet`
- Consumes: `Switch` from `@/components/ui/switch`
- Consumes: `Accordion`, `AccordionItem`, `AccordionTrigger`, `AccordionContent` from `@/components/ui/accordion`
- Consumes: `Input` from `@/components/ui/input`
- Consumes: all existing atoms/stores without changes

- [ ] **Step 1: Replace the backdrop + fixed panel with `<Sheet>`**

The Sheet component handles backdrop, slide-in animation, and Esc-to-close natively. Remove the manual `visible` state, backdrop `<div>`, and the fixed panel `<div>`.

```tsx
// NEW: wrap return in Sheet
return (
  <Sheet open={open} onOpenChange={(next) => { if (!next) closeDrawer() }}>
    <SheetContent side="right" className="w-full sm:w-80 p-0 flex flex-col">
      {/* Header, search, sections go here */}
    </SheetContent>
  </Sheet>
)
```

Remove: `visible` state, the 200ms delay `useEffect`, the backdrop `<div>`, the outer fixed-panel `<div>` with `animate-in`/`slide-in-from-right-full` classes. All of these are handled by `<Sheet>`.

- [ ] **Step 2: Replace the header close button**

```tsx
// The Sheet has a built-in close button (X) — keep or hide via shadcn prop.
// Remove the custom close button (✕) since Sheet provides one.
{/* Header without close button */}
<div className="flex items-center justify-between px-3 py-3 border-b border-border flex-shrink-0">
  <SheetTitle className="text-[14px] font-semibold text-foreground pl-1">Capabilities</SheetTitle>
</div>
```

- [ ] **Step 3: Replace custom search input with `<Input>`**

```tsx
// OLD (lines 263-274):
<input type="text" value={search} onChange={(e) => setSearch(e.target.value)}
  placeholder="Search capabilities..."
  className="w-full pl-8 pr-2 py-1.5 bg-[#12121e] border border-[#2a2a44] rounded text-[16px] sm:text-[12px] text-foreground/80 placeholder:text-muted-foreground/60 focus:outline-none focus:border-primary" />

// NEW:
<div className="relative">
  <Search className="absolute left-3 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground/70" />
  <Input type="text" value={search} onChange={(e) => setSearch(e.target.value)}
    placeholder="Search capabilities..."
    className="pl-8 pr-2 py-1.5 bg-[#12121e] border-[#2a2a44] text-[16px] sm:text-[12px] text-foreground/80 placeholder:text-muted-foreground/60" />
</div>
```

Add `import { Search } from 'lucide-react'` (already in dependencies).

- [ ] **Step 4: Replace custom collapsible sections with `<Accordion>`**

Replace the `collapsed` state + `toggleCollapsed` + custom header buttons with `<Accordion>`:

```tsx
// NEW: Wrap sections in Accordion
{sections.map((section) => (
  <AccordionItem key={section.group} value={section.group} className="border-0">
    <AccordionTrigger className="px-3 py-1 hover:bg-secondary rounded text-[11px] font-semibold text-muted-foreground uppercase tracking-[0.5px] hover:no-underline">
      {section.title} ({filterCapabilityItems(section.items, section.base, search).length})
    </AccordionTrigger>
    <AccordionContent className="px-3 pb-1">
      {/* filtered items with CapabilityToggle */}
    </AccordionContent>
  </AccordionItem>
))}
```

Remove: `collapsed` state, `toggleCollapsed` callback, the entire `SectionGroup` sub-component (its header button and collapse logic are now in Accordion).

- [ ] **Step 5: Replace custom toggle switch with `<Switch>`**

```tsx
// In CapabilityToggle — OLD (lines 370-387):
<button type="button" role="switch" aria-checked={checked} aria-label={name}
  onClick={onToggle}
  className={cn('inline-flex w-8 h-4 rounded-full relative transition-colors flex-shrink-0 border-0 p-0 cursor-pointer',
    checked ? 'bg-[#4080ff]' : 'bg-[#3a3a55]')}>
  <span className={cn('absolute top-[2px] w-3 h-3 rounded-full transition-all',
    checked ? 'right-[2px] bg-white' : 'left-[2px] bg-[#888]')} />
</button>

// NEW:
<Switch checked={checked} onCheckedChange={onToggle} aria-label={name} />
```

- [ ] **Step 6: Simplify CapabilityToggle — remove saving feedback from toggle row**

The saving spinner/checkmark/error can stay next to the name text. Keep those `<span>` elements, just remove the custom toggle button.

- [ ] **Step 7: Verify typecheck**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
```

- [ ] **Step 8: Commit**

```bash
git add frontend/src/components/inputs/CapabilityDrawer.tsx
git commit -m "refactor: rewrite CapabilityDrawer with shadcn Sheet/Switch/Accordion/Input"
```

---

### Task 7: Adopt ScrollArea + Skeleton across panels

**Files:**
- Modify: `frontend/src/components/panels/SessionsPanel.tsx` (loading spinner → Skeleton, overflow-y-auto → ScrollArea)
- Modify: `frontend/src/components/panels/LogViewer.tsx` (loading spinner → Skeleton, overflow-y-auto → ScrollArea)
- Modify: `frontend/src/components/panels/ContextPanel.tsx` (overflow-y-auto → ScrollArea)
- Modify: `frontend/src/components/panels/AgentsPanel.tsx`
- Modify: `frontend/src/components/panels/TasksPanel.tsx`
- Modify: `frontend/src/components/panels/ToolsTab.tsx`
- Modify: `frontend/src/components/panels/SkillsPanel.tsx`
- Modify: `frontend/src/components/panels/McpPanel.tsx`
- Modify: `frontend/src/components/panels/NodesPanel.tsx`

**Interfaces:**
- Consumes: `ScrollArea` from `@/components/ui/scroll-area` (installed)
- Consumes: `Skeleton` from `@/components/ui/skeleton` (Task 1)

- [ ] **Step 1: Audit panel loading states**

Find all loading spinner patterns:

```bash
grep -n 'animate-spin' /root/vol/frontend/src/components/panels/*.tsx
```

Replace each with `<Skeleton>`:

```tsx
// OLD pattern in SessionsPanel, LogViewer, etc:
<div className="flex-1 flex items-center justify-center gap-2 text-muted-foreground text-[14px]">
  <span className="w-4 h-4 rounded-full border-2 border-border border-t-[#80a0ff] animate-spin" />
  Loading sessions...
</div>

// NEW:
<div className="flex-1 flex flex-col items-center justify-center gap-3 p-4">
  <Skeleton className="h-4 w-48" />
  <Skeleton className="h-4 w-32" />
  <Skeleton className="h-4 w-40" />
</div>
```

- [ ] **Step 2: Replace overflow-y-auto containers with ScrollArea**

In each panel, replace the outermost `overflow-y-auto` container:

```tsx
// OLD:
<div className="flex-1 overflow-y-auto p-2">
  {/* content */}
</div>

// NEW:
<ScrollArea className="flex-1">
  <div className="p-2">
    {/* content */}
  </div>
</ScrollArea>
```

Add `import { ScrollArea } from '@/components/ui/scroll-area'` to each file.

Note: some panels nest `overflow-y-auto` inside dialog content — those stay as-is since dialogs already use `<ScrollArea>` or have their own scroll strategy.

- [ ] **Step 3: Verify typecheck**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
```

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/panels/
git commit -m "refactor: adopt ScrollArea and Skeleton across panels"
```

---

### Task 8: Update vol-web-dev skill with shadcn/ui reference

**Files:**
- Modify: `frontend/.claude/skills/vol-web-dev/SKILL.md` → actually it's at `.claude/skills/vol-web-dev/SKILL.md`

Wait — check actual path…

**Files:**
- Modify: `.claude/skills/vol-web-dev/SKILL.md` (add shadcn/ui section)

- [ ] **Step 1: Add shadcn/ui Usage section**

Insert after the "Tailwind CSS (v4)" section and before "Build and Check Commands":

```markdown
## shadcn/ui Components

The project uses [shadcn/ui](https://ui.shadcn.com/docs/components) primitives built on Radix UI. Installed components live in `frontend/src/components/ui/`.

### Installed Primitives

| Component | Source | Usage |
|-----------|--------|-------|
| Button | `@/components/ui/button` | All buttons, with variants: `default`, `destructive`, `outline`, `secondary`, `ghost`, `link`, `success` |
| Dialog | `@/components/ui/dialog` | All modal overlays |
| Tabs | `@/components/ui/tabs` | Tab navigation in main layout and sub-panels |
| Sheet | `@/components/ui/sheet` | Slide-in panels (CapabilityDrawer) |
| Switch | `@/components/ui/switch` | Toggle switches |
| Checkbox | `@/components/ui/checkbox` | Checkbox inputs |
| Input | `@/components/ui/input` | Text input fields |
| Select | `@/components/ui/select` | Dropdown selects |
| Label | `@/components/ui/label` | Form labels |
| Badge | `@/components/ui/badge` | Status/count indicators |
| ScrollArea | `@/components/ui/scroll-area` | Styled scrollable regions |
| Accordion | `@/components/ui/accordion` | Collapsible sections |
| Skeleton | `@/components/ui/skeleton` | Loading placeholders |
| Tooltip | `@/components/ui/tooltip` | Hover tooltips |

### Adding a New shadcn Component

```bash
cd frontend && npx shadcn@latest add <component-name>
```

Available components: https://ui.shadcn.com/docs/components

### Conventions

- **Always use `<Button>`** — never raw `<button>` elements. Pick the closest variant.
- **Always use `<ScrollArea>`** for scrollable panel containers — never raw `overflow-y-auto`.
- **Always use `<Badge>`** for status/count indicators — never raw styled `<span>` elements.
- **Loading states** — use `<Skeleton>` placeholders, not CSS spinner spans.
- **Dialogs** — use `<Dialog>` + `<DialogContent>`, not custom overlays.
- **Slide-in panels** — use `<Sheet>`, not custom fixed-position divs.
```

- [ ] **Step 2: Verify the skill file is well-formed**

```bash
grep -c '^---$' .claude/skills/vol-web-dev/SKILL.md
# Should output 2 (frontmatter delimiters)
```

- [ ] **Step 3: Commit**

```bash
git add .claude/skills/vol-web-dev/SKILL.md
git commit -m "docs: add shadcn/ui usage section to vol-web-dev skill"
```

---

### Task 9: Final verification — full build + visual check

- [ ] **Step 1: Full TypeScript check**

```bash
cd /root/vol/frontend && npx tsc -b --noEmit
```

Expected: zero type errors.

- [ ] **Step 2: Production build**

```bash
make web-build
```

Expected: successful Vite build, output in `frontend/dist/`.

- [ ] **Step 3: Run unit tests**

```bash
npm --prefix frontend run test:run
```

Expected: all tests pass (no functional changes).

- [ ] **Step 4: Commit any remaining changes**

```bash
git add -A
git commit -m "chore: final verification after shadcn/ui optimization"
```

---

## Self-Review

1. **Spec coverage:** Buttons ✓, Tabs ✓, CapabilityDrawer (Sheet+Switch+Accordion+Input) ✓, Checkbox ✓, Badge ✓, ScrollArea ✓, Skeleton ✓, Skill update ✓. Not covered by spec: Tooltip install is included in Task 1 install but no explicit tooltip usage task. Acceptable — Tooltip is installed and available for future use.

2. **Placeholder scan:** No TBD, TODO, "implement later", or vague "add appropriate error handling" statements. All code blocks are concrete.

3. **Type consistency:** `activeTabAtom` / `ActiveTab` types used consistently across Tasks 5 (Tabs rewrite). `Button` variants (`success`, `destructive`, `ghost`, `link`, `outline`) used consistently. All shadcn component imports follow the `@/components/ui/<name>` convention.
