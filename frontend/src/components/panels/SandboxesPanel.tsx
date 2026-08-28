// Sandboxes panel: shows spec profiles (templates) and running instances.
// Specs section lists available sandbox configurations.
// Instances section lists currently running sandbox instances.
import { useCallback, useEffect, useRef } from 'react'
import { useAtom, useAtomValue } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { sandboxesStateAtom, sandboxSpecsStateAtom } from '@/stores/sandboxes'
import { activeNodeIdAtom } from '@/stores/ui'
import { ScrollArea } from '@/components/ui/scroll-area'
import { Skeleton } from '@/components/ui/skeleton'
import { Button } from '@/components/ui/button'
import { Separator } from '@/components/ui/separator'
import type { RpcMethods } from '@/lib/protocol'
import type { SandboxInfo, SandboxSpecInfo } from '@/types'

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
  const [instancesState, setInstancesState] = useAtom(sandboxesStateAtom)
  const [specsState, setSpecsState] = useAtom(sandboxSpecsStateAtom)
  const nodeIdRef = useRef(nodeId)

  useEffect(() => {
    nodeIdRef.current = nodeId
  }, [nodeId])

  const loadInstances = useCallback(
    async (target: string | null) => {
      if (!target) {
        setInstancesState({ sandboxes: [], loading: false, error: null })
        return
      }
      setInstancesState((s) => ({ ...s, loading: true, error: null }))
      try {
        const res =
          await getPanelClient().call<RpcMethods['sandbox.list']['result']>('sandbox.list')
        if (nodeIdRef.current !== target) return
        setInstancesState({ sandboxes: res.sandboxes ?? [], loading: false, error: null })
      } catch (err) {
        if (nodeIdRef.current !== target) return
        setInstancesState((s) => ({ ...s, loading: false, error: errMsg(err) }))
      }
    },
    [setInstancesState],
  )

  const loadSpecs = useCallback(
    async (target: string | null) => {
      if (!target) {
        setSpecsState({ specs: [], loading: false, error: null })
        return
      }
      setSpecsState((s) => ({ ...s, loading: true, error: null }))
      try {
        const res =
          await getPanelClient().call<RpcMethods['sandbox.list_specs']['result']>(
            'sandbox.list_specs',
          )
        if (nodeIdRef.current !== target) return
        setSpecsState({ specs: res.specs ?? [], loading: false, error: null })
      } catch (err) {
        if (nodeIdRef.current !== target) return
        setSpecsState((s) => ({ ...s, loading: false, error: errMsg(err) }))
      }
    },
    [setSpecsState],
  )

  useEffect(() => {
    void loadInstances(nodeId)
    void loadSpecs(nodeId)
  }, [loadInstances, loadSpecs, nodeId])

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

  const allLoading =
    (instancesState.loading || specsState.loading) &&
    instancesState.sandboxes.length === 0 &&
    specsState.specs.length === 0 &&
    instancesState.error === null &&
    specsState.error === null

  if (allLoading) {
    return (
      <div className="flex-1 flex flex-col items-center justify-center gap-3 p-4">
        <Skeleton className="h-4 w-48" />
        <Skeleton className="h-4 w-32" />
        <Skeleton className="h-4 w-40" />
      </div>
    )
  }

  return (
    <ScrollArea className="flex-1">
      <div className="p-2 flex flex-col gap-2">
        <SpecsSection
          specs={specsState.specs}
          loading={specsState.loading}
          error={specsState.error}
          onRetry={() => void loadSpecs(nodeId)}
        />
        <Separator />
        <InstancesSection
          sandboxes={instancesState.sandboxes}
          loading={instancesState.loading}
          error={instancesState.error}
          onRetry={() => void loadInstances(nodeId)}
        />
      </div>
    </ScrollArea>
  )
}

function SectionHeader({ title, count }: { title: string; count?: number }) {
  return (
    <div className="flex items-center gap-2 px-1 pt-1">
      <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground">
        {title}
      </span>
      {count !== undefined && <span className="text-[10px] text-muted-foreground/60">{count}</span>}
    </div>
  )
}

function SpecsSection({
  specs,
  loading,
  error,
  onRetry,
}: {
  specs: SandboxSpecInfo[]
  loading: boolean
  error: string | null
  onRetry: () => void
}) {
  if (loading && specs.length === 0) {
    return (
      <div>
        <SectionHeader title="Specs" />
        <div className="flex flex-col gap-1 p-2">
          <Skeleton className="h-6 w-40" />
          <Skeleton className="h-6 w-32" />
        </div>
      </div>
    )
  }
  if (error !== null && specs.length === 0) {
    return (
      <div>
        <SectionHeader title="Specs" />
        <div className="flex items-center gap-2 p-2">
          <span className="text-destructive text-[12px]">Failed to load specs</span>
          <Button variant="outline" size="sm" onClick={onRetry}>
            Retry
          </Button>
        </div>
      </div>
    )
  }
  return (
    <div>
      <SectionHeader title="Specs" count={specs.length} />
      {specs.length === 0 ? (
        <div className="text-muted-foreground/50 text-center p-3 text-[12px]">
          No spec profiles configured
        </div>
      ) : (
        <SpecList specs={specs} />
      )}
    </div>
  )
}

function SpecList({ specs }: { specs: SandboxSpecInfo[] }) {
  return (
    <div className="flex flex-col gap-0.5">
      {specs.map((s) => (
        <div
          key={s.name}
          className="flex items-center justify-between px-2 py-1.5 rounded-md hover:bg-secondary/50"
        >
          <span className="text-[13px] text-foreground truncate">{s.name}</span>
          <span
            className={`text-[10px] px-1.5 py-0.5 rounded border flex-shrink-0 ${kindBadgeClass(s.kind)}`}
          >
            {s.kind}
          </span>
        </div>
      ))}
    </div>
  )
}

function InstancesSection({
  sandboxes,
  loading,
  error,
  onRetry,
}: {
  sandboxes: SandboxInfo[]
  loading: boolean
  error: string | null
  onRetry: () => void
}) {
  if (loading && sandboxes.length === 0) {
    return (
      <div>
        <SectionHeader title="Instances" />
        <div className="flex flex-col gap-1 p-2">
          <Skeleton className="h-6 w-40" />
          <Skeleton className="h-6 w-32" />
        </div>
      </div>
    )
  }
  if (error !== null && sandboxes.length === 0) {
    return (
      <div>
        <SectionHeader title="Instances" />
        <div className="flex flex-col items-center gap-2 p-3">
          <span className="text-destructive text-[12px]">Failed to load instances</span>
          <Button variant="outline" size="sm" onClick={onRetry}>
            Retry
          </Button>
        </div>
      </div>
    )
  }
  return (
    <div>
      <SectionHeader title="Instances" count={sandboxes.length} />
      {sandboxes.length === 0 ? (
        <div className="text-muted-foreground/50 text-center p-3 text-[12px]">
          No running sandbox instances
        </div>
      ) : (
        <InstanceList sandboxes={sandboxes} error={error} onRetry={onRetry} />
      )}
    </div>
  )
}

function InstanceList({
  sandboxes,
  error,
  onRetry,
}: {
  sandboxes: SandboxInfo[]
  error: string | null
  onRetry: () => void
}) {
  return (
    <div className="flex flex-col gap-0.5">
      {sandboxes.map((s) => (
        <div
          key={s.name}
          className="flex items-center justify-between px-2 py-1.5 rounded-md hover:bg-secondary/50"
        >
          <div className="flex items-center gap-2 min-w-0 flex-1">
            <span className="text-[13px] text-foreground truncate">{s.name}</span>
            <span
              className={`text-[10px] px-1.5 py-0.5 rounded border flex-shrink-0 ${kindBadgeClass(s.kind)}`}
            >
              {s.kind}
            </span>
          </div>
          <div className="text-[11px] text-muted-foreground/70 truncate max-w-[200px] ml-2 font-mono">
            {s.root_path}
          </div>
        </div>
      ))}
      {error !== null && (
        <div className="text-destructive p-2 text-[12px] bg-red-950/30 border border-destructive/50 rounded mt-1">
          Error: {error}
          <Button variant="outline" size="sm" className="ml-2" onClick={onRetry}>
            Retry
          </Button>
        </div>
      )}
    </div>
  )
}
