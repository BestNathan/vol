# Tools Tab Redesign — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rewrite the Tools tab to show only DP node tool listing with search filtering, inline Run actions, and remove per-agent Call History.

**Architecture:** Single-file component rewrite (`ToolsTab.tsx`) with a search-filtered tool list. Reuses existing `ToolCallDialog` for Run. Removes `CallHistoryItem`, `statusBadge`, and the tab's read of `toolCallsAtom`. No backend changes.

**Tech Stack:** React 18, TypeScript, jotai (atoms), shadcn/ui (Input, Button, Badge, ScrollArea, Skeleton), lucide-react (icons)

## Global Constraints

- No backend changes — `tool.list` and `tool.call` RPCs are already routed through the active DP node
- Reuse existing `ToolCallDialog` component without modification
- Keep `toolCallsAtom` and `event-handlers.ts` writes as-is (harmless, cleanup deferred)
- No per-node tool cache (same as current behavior)

---

### Task 1: Update existing unit tests

**Files:**
- Modify: `frontend/tests/unit/tools-tab.test.ts`

**Interfaces:**
- Consumes: Current `formatToolCallResult` export from `ToolsTab.tsx`
- Produces: Tests for `formatToolCallResult` (kept) plus new tests for the search filter function

- [ ] **Step 1: Remove `statusBadge` tests and add filterToolList test**

Replace the file content with tests covering the remaining export (`formatToolCallResult`) and a new `filterToolList` function:

```typescript
// frontend/tests/unit/tools-tab.test.ts
import { describe, it, expect } from 'vitest'
import { formatToolCallResult, filterToolList } from '@/components/panels/ToolsTab'

function tool(name: string, description: string) {
  return { name, description, parameters: undefined }
}

describe('formatToolCallResult', () => {
  it('extracts the content string from the tool.call result envelope', () => {
    const result = {
      tool_name: 'bash',
      result: { success: true, content: 'hello world', error: null, data: null },
    }
    expect(formatToolCallResult(result)).toBe('hello world')
  })

  it('falls back to pretty-printed JSON when content is absent', () => {
    const result = {
      tool_name: 'bash',
      result: { success: false, content: null, error: 'boom', data: { code: 1 } },
    }
    const text = formatToolCallResult(result)
    expect(text).toContain('"tool_name": "bash"')
    expect(text).toContain('"error": "boom"')
    expect(text).toContain('\n')
  })

  it('handles non-object results without throwing', () => {
    expect(formatToolCallResult(null)).toBe('null')
    expect(formatToolCallResult('raw')).toBe('"raw"')
    expect(formatToolCallResult(42)).toBe('42')
  })
})

describe('filterToolList', () => {
  const tools = [
    tool('bash', 'Execute shell commands'),
    tool('read', 'Read a file from disk'),
    tool('grep', 'Search file contents with regex'),
  ]

  it('returns all tools when search is empty', () => {
    expect(filterToolList(tools, '')).toHaveLength(3)
  })

  it('matches by tool name (case-insensitive)', () => {
    expect(filterToolList(tools, 'BASH')).toHaveLength(1)
    expect(filterToolList(tools, 'BASH')[0].name).toBe('bash')
  })

  it('matches by description (case-insensitive)', () => {
    expect(filterToolList(tools, 'regex')).toHaveLength(1)
    expect(filterToolList(tools, 'regex')[0].name).toBe('grep')
  })

  it('returns empty array when nothing matches', () => {
    expect(filterToolList(tools, 'nonexistent')).toHaveLength(0)
  })
})
```

- [ ] **Step 2: Run tests to verify they fail (new functions not yet exported)**

Run: `cd frontend && npx vitest run tests/unit/tools-tab.test.ts`
Expected: FAIL — `filterToolList is not exported from ToolsTab.tsx`

- [ ] **Step 3: Commit**

```bash
git add frontend/tests/unit/tools-tab.test.ts
git commit -m "test(tools-tab): update tests for redesigned ToolsTab — add filterToolList, remove statusBadge"
```

---

### Task 2: Rewrite ToolsTab component

**Files:**
- Modify: `frontend/src/components/panels/ToolsTab.tsx` (full rewrite)

**Interfaces:**
- Consumes: `systemToolsAtom`, `toolsLoadingAtom` from `@/stores/tools`; `getPanelClient` from `@/lib/panel-client`; `ToolCallDialog` from `@/components/dialogs/ToolCallDialog`; shadcn components; lucide icons
- Produces: `ToolsTab` (default export), `filterToolList` (named export), `formatToolCallResult` (named export, kept from current)

- [ ] **Step 1: Write the new ToolsTab component**

Replace the entire file:

```typescript
// frontend/src/components/panels/ToolsTab.tsx
// Tools tab: DP node tool listing with search filter and inline Run actions.
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useAtom, useAtomValue } from 'jotai'
import { Search, Wrench } from 'lucide-react'
import { getPanelClient } from '@/lib/panel-client'
import { systemToolsAtom, toolsLoadingAtom } from '@/stores/tools'
import { ToolCallDialog, type ToolCallOutcome } from '@/components/dialogs/ToolCallDialog'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import type { RpcMethods } from '@/lib/protocol'

/** System tool row as stored in systemToolsAtom (description coerced to string). */
export interface SystemTool {
  name: string
  description: string
  parameters?: unknown
}

/**
 * Extract displayable content from a `tool.call` result
 * (`{ tool_name, result: { success, content, error, data } }`); falls back to
 * pretty-printed JSON when `content` is absent.
 */
export function formatToolCallResult(result: unknown): string {
  if (result && typeof result === 'object' && !Array.isArray(result)) {
    const inner = (result as Record<string, unknown>).result
    if (inner && typeof inner === 'object' && !Array.isArray(inner)) {
      const content = (inner as Record<string, unknown>).content
      if (typeof content === 'string') return content
    }
  }
  try {
    return JSON.stringify(result, null, 2)
  } catch {
    return String(result)
  }
}

/**
 * Filter tools by search query (case-insensitive match on name and description).
 */
export function filterToolList(tools: SystemTool[], search: string): SystemTool[] {
  const q = search.trim().toLowerCase()
  if (q === '') return tools
  return tools.filter(
    (t) => t.name.toLowerCase().includes(q) || t.description.toLowerCase().includes(q),
  )
}

export function ToolsTab() {
  const [tools, setTools] = useAtom(systemToolsAtom)
  const [loading, setLoading] = useAtom(toolsLoadingAtom)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [search, setSearch] = useState('')
  const [dialogTool, setDialogTool] = useState<SystemTool | null>(null)

  const loadTools = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const res = await getPanelClient().call<RpcMethods['tool.list']['result']>('tool.list')
      setTools(
        (res.tools ?? []).map((t) => ({
          name: t.name,
          description: typeof t.description === 'string' ? t.description : '',
          parameters: t.parameters,
        })),
      )
    } catch (err) {
      setTools([])
      setLoadError((err as { message?: string } | null)?.message ?? String(err))
    } finally {
      setLoading(false)
    }
  }, [setTools, setLoading])

  // Fetch on mount if not cached; Refresh re-fetches.
  useEffect(() => {
    if (tools.length === 0) {
      void loadTools()
    }
  }, [loadTools])

  const executeTool = useCallback(
    async (args: Record<string, unknown>): Promise<ToolCallOutcome> => {
      const tool = dialogTool
      if (!tool) return { ok: false, error: 'No tool selected' }
      try {
        const res = await getPanelClient().call<RpcMethods['tool.call']['result']>('tool.call', {
          tool_name: tool.name,
          arguments: args,
        })
        return { ok: true, content: formatToolCallResult(res) }
      } catch (err) {
        return { ok: false, error: (err as { message?: string } | null)?.message ?? String(err) }
      }
    },
    [dialogTool],
  )

  const filteredTools = useMemo(() => filterToolList(tools, search), [tools, search])

  return (
    <ScrollArea className="flex-1 min-h-0">
      <div className="p-2 space-y-2">
        {/* Header */}
        <div className="flex items-center justify-between">
          <div className="flex items-center gap-1.5 px-1">
            <Wrench className="h-3.5 w-3.5 text-muted-foreground" />
            <span className="text-[12px] font-semibold text-muted-foreground uppercase tracking-[0.5px]">
              Tools ({tools.length})
            </span>
          </div>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => void loadTools()}
            disabled={loading}
          >
            Refresh
          </Button>
        </div>

        {/* Search */}
        <div className="relative">
          <Search className="absolute left-2.5 top-1/2 -translate-y-1/2 h-3.5 w-3.5 text-muted-foreground/60 pointer-events-none" />
          <Input
            type="text"
            placeholder="Search tools..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="pl-8 bg-[#12121e] border-[#2a2a44] text-[13px] text-foreground/80 placeholder:text-muted-foreground/60 h-8"
          />
        </div>

        {/* Content area */}
        {loading ? (
          <div className="space-y-2 py-1">
            {[1, 2, 3, 4].map((i) => (
              <Skeleton key={i} className="h-[44px] w-full rounded-md" />
            ))}
          </div>
        ) : loadError ? (
          <div className="flex flex-col items-center gap-2 py-8">
            <div className="text-[13px] text-destructive">Error: {loadError}</div>
            <Button variant="secondary" size="sm" onClick={() => void loadTools()}>
              Retry
            </Button>
          </div>
        ) : tools.length === 0 ? (
          <div className="flex items-center justify-center h-[200px] text-[14px] text-muted-foreground/70">
            No tools available
          </div>
        ) : filteredTools.length === 0 ? (
          <div className="flex items-center justify-center h-[200px] text-[14px] text-muted-foreground/70">
            No tools match "{search}"
          </div>
        ) : (
          <div className="space-y-0.5">
            {filteredTools.map((tool) => (
              <ToolRow key={tool.name} tool={tool} onRun={() => setDialogTool(tool)} />
            ))}
          </div>
        )}

        {/* Run dialog */}
        <ToolCallDialog
          open={dialogTool !== null}
          toolName={dialogTool?.name ?? ''}
          description={dialogTool?.description || undefined}
          schema={toolSchemaToRecord(dialogTool?.parameters)}
          onClose={() => setDialogTool(null)}
          onExecute={executeTool}
        />
      </div>
    </ScrollArea>
  )
}

// ── Sub-components ────────────────────────────────────────────────────────

function ToolRow({ tool, onRun }: { tool: SystemTool; onRun: () => void }) {
  return (
    <div className="flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-secondary/50 transition-colors group">
      {/* Icon */}
      <Wrench className="h-3.5 w-3.5 text-muted-foreground flex-shrink-0" />

      {/* Name + description */}
      <div className="min-w-0 flex-1">
        <span className="text-[13px] font-semibold text-foreground truncate block">
          {tool.name}
        </span>
        {tool.description && (
          <div
            className="text-[11px] text-muted-foreground truncate"
            title={tool.description}
          >
            {tool.description}
          </div>
        )}
      </div>

      {/* Run button — visible on hover (desktop) */}
      <Button
        variant="secondary"
        size="sm"
        className="opacity-0 group-hover:opacity-100 transition-opacity flex-shrink-0"
        onClick={onRun}
      >
        Run
      </Button>
    </div>
  )
}

// ── Helpers ────────────────────────────────────────────────────────────────

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined
}

function toolSchemaToRecord(params: unknown): Record<string, unknown> | undefined {
  return asRecord(params)
}
```

- [ ] **Step 2: Run tests to verify the filterToolList tests pass**

Run: `cd frontend && npx vitest run tests/unit/tools-tab.test.ts`
Expected: PASS (10 tests)

- [ ] **Step 3: Verify TypeScript compiles**

Run: `cd frontend && npx tsc --noEmit`
Expected: No errors

- [ ] **Step 4: Commit**

```bash
git add frontend/src/components/panels/ToolsTab.tsx
git commit -m "refactor(frontend): redesign Tools tab — search filter, remove call history, shadcn styling"
```
