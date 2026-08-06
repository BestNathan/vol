// frontend/src/components/panels/ToolsTab.tsx
// Tools tab: DP node tool listing with search filter and inline Run actions.
import { useCallback, useEffect, useMemo, useState } from 'react'
import { useAtom } from 'jotai'
import { Search, Wrench } from 'lucide-react'
import { getPanelClient } from '@/lib/panel-client'
import { systemToolsAtom, toolsLoadingAtom } from '@/stores/tools'
import { ToolDetailDialog, type ToolCallOutcome } from '@/components/dialogs/ToolDetailDialog'
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
    (t) => t.name.toLowerCase().includes(q) || (t.description ?? '').toLowerCase().includes(q),
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
            aria-label="Search tools"
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
              <ToolRow key={tool.name} tool={tool} onClick={() => setDialogTool(tool)} />
            ))}
          </div>
        )}

        {/* Tool detail dialog */}
        <ToolDetailDialog
          open={dialogTool !== null}
          tool={dialogTool}
          onClose={() => setDialogTool(null)}
          onExecute={executeTool}
        />
      </div>
    </ScrollArea>
  )
}

// ── Sub-components ────────────────────────────────────────────────────────

function ToolRow({ tool, onClick }: { tool: SystemTool; onClick: () => void }) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-center gap-2 px-2 py-1.5 rounded-md hover:bg-secondary/50 transition-colors w-full text-left cursor-pointer"
    >
      <Wrench className="h-3.5 w-3.5 text-muted-foreground flex-shrink-0" />
      <div className="min-w-0 flex-1">
        <span className="text-[13px] font-semibold text-foreground truncate block">
          {tool.name}
        </span>
        {tool.description && (
          <div className="text-[11px] text-muted-foreground truncate" title={tool.description}>
            {tool.description}
          </div>
        )}
      </div>
    </button>
  )
}

