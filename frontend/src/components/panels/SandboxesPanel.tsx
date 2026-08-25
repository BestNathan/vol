// Sandboxes panel: lists all registered sandboxes on the active data-plane node.
// Shows name, kind, and root_path for each sandbox.
import { useCallback, useEffect, useRef } from 'react'
import { useAtom, useAtomValue } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { sandboxesStateAtom } from '@/stores/sandboxes'
import { activeNodeIdAtom } from '@/stores/ui'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import { Button } from '@/components/ui/button'
import type { RpcMethods } from '@/lib/protocol'
import type { SandboxInfo } from '@/types'

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

/** Kind badge color — local=green, ssh=blue, tmp=gray, firecracker=orange, wasm=purple */
export function kindBadgeClass(kind: string): string {
  switch (kind) {
    case 'local':
      return 'bg-emerald-900/40 text-emerald-300 border-emerald-700/50'
    case 'ssh':
      return 'bg-blue-900/40 text-blue-300 border-blue-700/50'
    case 'tmp':
      return 'bg-secondary text-muted-foreground border-border'
    case 'firecracker':
      return 'bg-orange-900/40 text-orange-300 border-orange-700/50'
    case 'wasm':
      return 'bg-purple-900/40 text-purple-300 border-purple-700/50'
    default:
      return 'bg-secondary text-muted-foreground border-border'
  }
}

export function SandboxesPanel() {
  const nodeId = useAtomValue(activeNodeIdAtom)
  const [state, setState] = useAtom(sandboxesStateAtom)
  const nodeIdRef = useRef(nodeId)

  useEffect(() => {
    nodeIdRef.current = nodeId
  }, [nodeId])

  const load = useCallback(
    async (target: string | null) => {
      if (!target) {
        setState({ sandboxes: [], loading: false, error: null })
        return
      }
      setState((s) => ({ ...s, loading: true, error: null }))
      try {
        const res =
          await getPanelClient().call<RpcMethods['sandbox.list']['result']>('sandbox.list')
        if (nodeIdRef.current !== target) return
        setState({ sandboxes: res.sandboxes ?? [], loading: false, error: null })
      } catch (err) {
        if (nodeIdRef.current !== target) return
        setState((s) => ({ ...s, loading: false, error: errMsg(err) }))
      }
    },
    [setState],
  )

  useEffect(() => {
    void load(nodeId)
  }, [load, nodeId])

  if (!nodeId) {
    return (
      <ScrollArea className="flex-1">
        <div className="h-full p-3 flex items-center justify-center">
          <div className="text-center">
            <div className="text-muted-foreground text-[14px]">Select a node to view sandboxes</div>
            <div className="text-muted-foreground/70 text-[12px] mt-1">
              Select a node from the dropdown above.
            </div>
          </div>
        </div>
      </ScrollArea>
    )
  }

  if (state.loading && state.sandboxes.length === 0 && state.error === null) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 p-4">
        <Skeleton className="h-4 w-48" />
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-4 w-40" />
      </div>
    )
  }

  if (state.error !== null && state.sandboxes.length === 0) {
    return (
      <ScrollArea className="flex-1">
        <div className="h-full p-3 flex items-center justify-center">
          <div className="flex flex-col items-center gap-3 text-center">
            <div className="text-destructive text-[14px]">Failed to load sandboxes</div>
            <div className="text-muted-foreground text-[12px] max-w-[300px] break-words">
              {state.error}
            </div>
            <Button variant="outline" size="sm" onClick={() => void load(nodeId)}>
              Retry
            </Button>
          </div>
        </div>
      </ScrollArea>
    )
  }

  return (
    <ScrollArea className="flex-1">
      <SandboxList
        sandboxes={state.sandboxes}
        error={state.error}
        onRetry={() => void load(nodeId)}
      />
    </ScrollArea>
  )
}

function SandboxList({
  sandboxes,
  error,
  onRetry,
}: {
  sandboxes: SandboxInfo[]
  error: string | null
  onRetry: () => void
}) {
  if (sandboxes.length === 0 && error === null) {
    return (
      <div className="text-muted-foreground/70 text-center p-4 text-[13px]">
        No sandboxes registered
      </div>
    )
  }
  return (
    <div className="p-2">
      {/* Mobile: sandbox cards */}
      <div className="sm:hidden flex flex-col gap-2">
        {sandboxes.map((s) => (
          <div key={s.name} className="rounded-lg border border-border bg-secondary p-3">
            <div className="flex items-center justify-between">
              <span className="text-[14px] font-bold text-foreground truncate">{s.name}</span>
              <span
                className={`text-[10px] px-1.5 py-0.5 rounded border flex-shrink-0 ml-2 ${kindBadgeClass(s.kind)}`}
              >
                {s.kind}
              </span>
            </div>
            <div className="text-[11px] text-muted-foreground/70 font-mono truncate mt-1">
              {s.root_path}
            </div>
          </div>
        ))}
      </div>
      {/* Desktop: sandbox rows */}
      <div className="hidden sm:block font-mono text-[13px]">
        {sandboxes.map((s) => (
          <div
            key={s.name}
            className="flex items-center justify-between py-1.5 border-b border-[#2a2a44]"
          >
            <div className="flex items-center gap-3 min-w-0 flex-1">
              <span className="text-[13px] text-foreground truncate">{s.name}</span>
              <span
                className={`text-[10px] px-1.5 py-0.5 rounded border flex-shrink-0 ${kindBadgeClass(s.kind)}`}
              >
                {s.kind}
              </span>
            </div>
            <div className="text-[11px] text-muted-foreground/70 truncate max-w-[300px] ml-3">
              {s.root_path}
            </div>
          </div>
        ))}
      </div>
      {error !== null && (
        <div className="text-destructive p-2 text-[12px] bg-red-950/30 border border-destructive/50 rounded mt-2">
          Error: {error}
          <Button variant="outline" size="sm" className="ml-2" onClick={onRetry}>
            Retry
          </Button>
        </div>
      )}
    </div>
  )
}
