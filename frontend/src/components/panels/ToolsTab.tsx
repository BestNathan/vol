// frontend/src/components/panels/ToolsTab.tsx
// Tools tab: system tool listing (tool.list) with Run → ToolCallDialog, plus
// tool call history fed by the event handlers. Port of tools_tab.rs.
import { useCallback, useEffect, useState } from 'react'
import { useAtom, useAtomValue } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { systemToolsAtom, toolCallsAtom, toolsLoadingAtom } from '@/stores/tools'
import { ToolCallDialog, type ToolCallOutcome } from '@/components/dialogs/ToolCallDialog'
import { Button } from '@/components/ui/button'
import { cn } from '@/lib/utils'
import type { RpcMethods } from '@/lib/protocol'
import type { ToolCallEntry, ToolCallStatus } from '@/types'

/** System tool row as stored in systemToolsAtom (description coerced to string). */
interface SystemTool {
  name: string
  description: string
  parameters?: unknown
}

/**
 * Status badge mapping: label + colors (OK green / ERR red / SKIP yellow /
 * "..." grey for a running call).
 */
export function statusBadge(status: ToolCallStatus): { label: string; className: string } {
  switch (status) {
    case 'Success':
      return { label: 'OK', className: 'text-emerald-400 bg-emerald-950/30' }
    case 'Error':
      return { label: 'ERR', className: 'text-destructive bg-red-950/30' }
    case 'Skipped':
      return { label: 'SKIP', className: 'text-yellow-400 bg-[#2a2a1a]' }
    case 'Running':
      return { label: '...', className: 'text-muted-foreground bg-secondary' }
  }
}

/**
 * Extract displayable content from a `tool.call` result
 * (`{ tool_name, result: { success, content, error, data } }`); falls back to
 * pretty-printed JSON when `content` is absent. Mirrors tool_dialog.rs.
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

function asRecord(v: unknown): Record<string, unknown> | undefined {
  return typeof v === 'object' && v !== null && !Array.isArray(v)
    ? (v as Record<string, unknown>)
    : undefined
}

export function ToolsTab() {
  const [tools, setTools] = useAtom(systemToolsAtom)
  const [loading, setLoading] = useAtom(toolsLoadingAtom)
  const calls = useAtomValue(toolCallsAtom)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [dialogTool, setDialogTool] = useState<SystemTool | null>(null)

  const loadTools = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    try {
      const res = await getPanelClient().call<RpcMethods['tool.list']['result']>('tool.list')
      // The wire shape may omit or null description/parameters — coerce so the
      // atom type (description: string) stays honest.
      setTools(
        (res.tools ?? []).map((t) => ({
          name: t.name,
          description: typeof t.description === 'string' ? t.description : '',
          parameters: t.parameters,
        }))
      )
    } catch (err) {
      setTools([])
      setLoadError((err as { message?: string } | null)?.message ?? String(err))
    } finally {
      setLoading(false)
    }
  }, [setTools, setLoading])

  // Fetch the tool list on mount (skip if already cached); Refresh re-fetches.
  useEffect(() => {
    if (tools.length === 0) {
      void loadTools()
    }
  }, [loadTools])

  const executeTool = useCallback(async (args: Record<string, unknown>): Promise<ToolCallOutcome> => {
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
  }, [dialogTool])

  return (
    <div className="flex-1 overflow-y-auto p-2">
      {/* System Tools section */}
      <div className="mb-3">
        <div className="flex items-center justify-between mb-1">
          <div className="px-2.5 py-1 text-[12px] font-semibold text-muted-foreground uppercase tracking-[0.5px]">
            System Tools ({tools.length})
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
        {loading && <div className="text-[12px] text-muted-foreground px-2">Loading...</div>}
        {loadError && (
          <div className="text-[12px] text-destructive px-2 break-words">Error: {loadError}</div>
        )}
        {/* Mobile: tool cards */}
        <div className="sm:hidden flex flex-col gap-2 mb-2">
          {tools.map((tool) => (
            <div key={tool.name} className="rounded-lg border border-border bg-secondary p-3">
              <div className="flex items-center justify-between gap-2">
                <div className="min-w-0">
                  <div className="truncate text-[14px] font-bold text-foreground">{tool.name}</div>
                  {tool.description && (
                    <div className="mt-0.5 text-[11px] text-[#777] truncate">{tool.description}</div>
                  )}
                </div>
                <Button size="sm" className="flex-shrink-0" onClick={() => setDialogTool(tool)}>
                  Run
                </Button>
              </div>
            </div>
          ))}
        </div>
        {/* Desktop: tool rows */}
        <div className="hidden sm:block">
          {tools.map((tool) => (
            <div key={tool.name} className="border-b border-[#2a2a44] py-1 px-2">
              <div className="flex items-center justify-between gap-2">
                <div className="min-w-0 flex items-baseline gap-2">
                  <span className="text-[13px] font-semibold text-foreground truncate">
                    {tool.name}
                  </span>
                  {tool.description && (
                    <span className="text-[12px] text-muted-foreground truncate">- {tool.description}</span>
                  )}
                </div>
                <Button
                  variant="secondary"
                  size="sm"
                  className="flex-shrink-0"
                  onClick={() => setDialogTool(tool)}
                >
                  Run
                </Button>
              </div>
            </div>
          ))}
        </div>
      </div>

      <div className="border-t border-[#333] my-2" />

      {/* Call History section */}
      {calls.length === 0 ? (
        <div className="flex items-center justify-center h-[200px] text-muted-foreground/70 text-[14px]">
          {loading
            ? 'Loading tools...'
            : loadError
              ? 'Failed to load tools'
              : tools.length > 0
                ? 'No tool calls yet — click Run on a tool above'
                : 'No tools available'}
        </div>
      ) : (
        <>
          <div className="px-2.5 pt-1 pb-2 text-[12px] font-semibold text-muted-foreground uppercase tracking-[0.5px]">
            Call History ({calls.length})
          </div>
          {/* Mobile: history cards */}
          <div className="sm:hidden flex flex-col gap-2">
            {calls.map((entry) => (
              <CallHistoryItem key={entry.sequence} entry={entry} variant="card" />
            ))}
          </div>
          {/* Desktop: history rows */}
          <div className="hidden sm:block">
            {calls.map((entry) => (
              <CallHistoryItem key={entry.sequence} entry={entry} variant="row" />
            ))}
          </div>
        </>
      )}

      <ToolCallDialog
        open={dialogTool !== null}
        toolName={dialogTool?.name ?? ''}
        description={dialogTool?.description || undefined}
        schema={asRecord(dialogTool?.parameters)}
        onClose={() => setDialogTool(null)}
        onExecute={executeTool}
      />
    </div>
  )
}

/** One call-history entry: expandable row (desktop) or card (mobile). */
function CallHistoryItem({ entry, variant }: { entry: ToolCallEntry; variant: 'row' | 'card' }) {
  const [expanded, setExpanded] = useState(false)
  const badge = statusBadge(entry.status)
  const duration = entry.durationMs !== null ? `${entry.durationMs}ms` : ''
  const toggle = () => setExpanded((e) => !e)

  if (variant === 'card') {
    return (
      <button
        type="button"
        onClick={toggle}
        className="w-full text-left cursor-pointer rounded-lg border border-border bg-secondary p-3 active:bg-secondary"
      >
        <span className="flex items-center gap-2 w-full">
          <span className="text-muted-foreground/60 text-[11px]">{entry.sequence}.</span>
          <span className="font-semibold text-[13px] text-foreground truncate">
            [{entry.toolName}]
          </span>
          <span className={cn('text-[11px] px-1.5 py-0.5 rounded-[3px] font-semibold', badge.className)}>
            {badge.label}
          </span>
          {duration !== '' && <span className="text-[11px] text-muted-foreground/70 ml-auto">{duration}</span>}
        </span>
        {expanded && (
          <span className="mt-2 pt-2 border-t border-[#2a2a44] block text-[12px] font-mono text-muted-foreground whitespace-pre-wrap break-all">
            <span className="text-[#6090ff] font-semibold font-sans">Input: </span>
            {entry.argPreview}
          </span>
        )}
      </button>
    )
  }

  return (
    <div className="border-b border-[#2a2a44]">
      <button
        type="button"
        onClick={toggle}
        className="flex items-center gap-2 px-2.5 py-2 w-full text-left cursor-pointer hover:bg-secondary/50"
      >
        <span className="text-muted-foreground/60 text-[11px] min-w-[24px]">{entry.sequence}.</span>
        <span className="font-semibold text-[13px]">{entry.toolName}</span>
        <span className={cn('text-[11px] px-1.5 py-0.5 rounded-[3px] font-semibold', badge.className)}>
          {badge.label}
        </span>
        {duration !== '' && <span className="text-[11px] text-muted-foreground ml-auto">{duration}</span>}
        <span className="text-[10px] text-muted-foreground/70 ml-1">▾</span>
      </button>
      {expanded && (
        <div className="px-2.5 pb-2 pl-[42px] text-[12px] font-mono text-muted-foreground bg-[#16162a] whitespace-pre-wrap break-all">
          <span className="text-[#6090ff] font-semibold font-sans">Input: </span>
          {entry.argPreview}
        </div>
      )}
    </div>
  )
}
