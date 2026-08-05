// frontend/src/components/shared/NodesDropdown.tsx
// Collapsible data-plane node selector for the StatusBar. Visible only in
// ControlPlane mode. Fetches the node list from the CP via control.node_list,
// refreshes whenever the dropdown opens (plus a 10 s interval while open so
// status dots and load counts stay live). Port of nodes_dropdown.rs.
import { useCallback, useEffect, useRef, useState } from 'react'
import { useAtomValue, useSetAtom } from 'jotai'
import { getControlClient } from '@/lib/panel-client'
import { dpPool } from '@/lib/dp-pool'
import { activeNodeIdAtom, viewingNodeDetailAtom } from '@/stores/ui'
import { serverModeAtom } from '@/stores/connection'
import type { NodeListEntry } from '@/types'
import type { RpcMethods } from '@/lib/protocol'

/** A node row is selectable (active target for agent RPCs) only when it is
 * online and exposes a direct ws_url for a DP connection. */
export function isNodeSelectable(node: NodeListEntry): boolean {
  return node.status === 'online' && !!node.ws_url
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function NodesDropdown() {
  const serverMode = useAtomValue(serverModeAtom)
  const activeNodeId = useAtomValue(activeNodeIdAtom)
  const setActiveNodeId = useSetAtom(activeNodeIdAtom)
  const setViewingNodeDetail = useSetAtom(viewingNodeDetailAtom)
  const [open, setOpen] = useState(false)
  const [nodes, setNodes] = useState<NodeListEntry[]>([])
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(false)
  const [panelPos, setPanelPos] = useState<{ top: number; left: number } | null>(null)
  const buttonRef = useRef<HTMLButtonElement>(null)

  // Find the active node's name for display in the trigger button.
  const activeNode = nodes.find(n => n.node_id === activeNodeId)

  const loadNodes = useCallback(async () => {
    setLoading(true)
    try {
      const res = await getControlClient().call<RpcMethods['control.node_list']['result']>('control.node_list')
      setNodes(res.nodes ?? [])
      setError(null)
    } catch (err) {
      setError(errMsg(err))
    } finally {
      setLoading(false)
    }
  }, [])

  // Fetch on mount; refresh on open and every 10 s while open.
  useEffect(() => {
    if (serverMode !== 'ControlPlane') return
    void loadNodes()
  }, [serverMode, loadNodes])
  useEffect(() => {
    if (!open || serverMode !== 'ControlPlane') return
    void loadNodes()
    const timer = setInterval(() => { void loadNodes() }, 10_000)
    return () => clearInterval(timer)
  }, [open, serverMode, loadNodes])

  // Auto-select first online node with ws_url after nodes load.
  useEffect(() => {
    if (activeNodeId || nodes.length === 0) return
    const first = nodes.find(n => isNodeSelectable(n))
    if (first) {
      dpPool.getOrCreate(first.node_id, first.ws_url!)
      setActiveNodeId(first.node_id)
    }
  }, [nodes, activeNodeId, setActiveNodeId])

  // Close on Escape, return focus to trigger button.
  useEffect(() => {
    if (!open) return
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setOpen(false)
        buttonRef.current?.focus()
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [open])

  // Only meaningful when connected to the control plane.
  if (serverMode !== 'ControlPlane') return null

  const handleSelect = (node: NodeListEntry) => {
    if (!isNodeSelectable(node)) return
    dpPool.getOrCreate(node.node_id, node.ws_url!)
    setActiveNodeId(node.node_id)
    setOpen(false)
  }

  const handleViewDetail = (nodeId: string) => {
    setViewingNodeDetail(nodeId)
    setOpen(false)
  }

  return (
    <div className="relative inline-block">
      <button
        ref={buttonRef}
        type="button"
        title={activeNode ? `Active: ${activeNode.name}` : 'Select data-plane node'}
        onClick={() => {
          const next = !open
          setOpen(next)
          if (next && buttonRef.current) {
            const r = buttonRef.current.getBoundingClientRect()
            setPanelPos({ top: r.bottom + 4, left: r.left })
          }
        }}
        className="flex items-center gap-1 px-2 py-0.5 text-xs rounded hover:bg-border cursor-pointer text-foreground bg-transparent border-none whitespace-nowrap"
      >
        {open ? '▴' : '▾'} {activeNode ? activeNode.name : `Nodes(${nodes.length})`}
      </button>
      {open && (
        <>
          {/* Transparent overlay — clicking outside closes the dropdown. */}
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          <div
            className="fixed min-w-[280px] max-w-[calc(100vw-1rem)] bg-card border border-border rounded shadow-lg z-50 max-h-[400px] overflow-y-auto"
            style={panelPos ? { top: panelPos.top, left: panelPos.left } : undefined}
            onClick={(e) => e.stopPropagation()}
          >
            {loading && nodes.length === 0 && (
              <div className="flex items-center gap-2 px-3 py-2 text-muted-foreground text-xs">
                <span className="w-3 h-3 rounded-full border-2 border-primary/30 border-t-primary animate-spin" />
                Loading nodes...
              </div>
            )}
            {error && (
              <div className="px-3 py-2 text-destructive text-xs">Failed to load nodes: {error}</div>
            )}
            {!loading && !error && nodes.length === 0 && (
              <div className="px-3 py-2 text-muted-foreground text-xs">No nodes available</div>
            )}
            {nodes.map((node) => {
              const selectable = isNodeSelectable(node)
              const selected = activeNodeId === node.node_id
              return (
                <div
                  key={node.node_id}
                  role="button"
                  tabIndex={0}
                  title={selectable ? `Select ${node.name}` : 'Node offline or no ws_url'}
                  onClick={() => handleSelect(node)}
                  onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') handleSelect(node) }}
                  className={
                    'flex items-center gap-2 px-3 py-2 cursor-pointer border-b border-border last:border-b-0 ' +
                    (selectable
                      ? (selected ? 'bg-primary/20' : 'hover:bg-secondary')
                      : 'opacity-50 cursor-not-allowed')
                  }
                >
                  {/* Status dot: green = online, red = offline */}
                  <span
                    className={'w-2 h-2 rounded-full flex-shrink-0 ' + (node.status === 'online' ? 'bg-emerald-500 shadow-[0_0_4px] shadow-emerald-500/50' : 'bg-destructive shadow-[0_0_4px] shadow-destructive/50')}
                  />
                  <span className="flex-1 min-w-0">
                    <span className="flex items-center gap-2">
                      <span
                        title="Click to view node detail"
                        onClick={(e) => { e.stopPropagation(); handleViewDetail(node.node_id) }}
                        className="text-foreground text-sm font-medium truncate cursor-pointer hover:text-primary"
                      >
                        {node.name}
                      </span>
                      {selected && <span className="text-emerald-400 text-xs flex-shrink-0">✓</span>}
                    </span>
                    <span className="text-muted-foreground text-xs truncate block">
                      {node.node_id} · v{node.version}
                    </span>
                  </span>
                  <span className="flex-shrink-0 text-right">
                    <span className="block text-muted-foreground text-xs">
                      R:{node.load.running} Q:{node.load.queued}
                    </span>
                    {node.agent_count != null && (
                      <span className="block text-muted-foreground/70 text-xs">{node.agent_count} agents</span>
                    )}
                  </span>
                </div>
              )
            })}
          </div>
        </>
      )}
    </div>
  )
}
