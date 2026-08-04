// frontend/src/components/shared/NodesDropdown.tsx
// Collapsible data-plane node selector for the StatusBar. Visible only in
// ControlPlane mode. Fetches the node list from the CP via control.node_list,
// refreshes whenever the dropdown opens (plus a 10 s interval while open so
// status dots and load counts stay live). Port of nodes_dropdown.rs.
import { useCallback, useEffect, useState } from 'react'
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

  const loadNodes = useCallback(async () => {
    try {
      const res = await getControlClient().call<RpcMethods['control.node_list']['result']>('control.node_list')
      setNodes(res.nodes ?? [])
      setError(null)
    } catch (err) {
      setError(errMsg(err))
    }
  }, [])

  // Fetch once on mount; refresh on every open and every 10 s while open.
  useEffect(() => { void loadNodes() }, [loadNodes])
  useEffect(() => {
    if (!open) return
    void loadNodes()
    const timer = setInterval(() => { void loadNodes() }, 10_000)
    return () => clearInterval(timer)
  }, [open, loadNodes])

  // Only meaningful when connected to the control plane.
  if (serverMode !== 'ControlPlane') return null

  const handleSelect = (node: NodeListEntry) => {
    if (!isNodeSelectable(node)) return
    // Create (or reuse) the direct DP connection for this node, then make it
    // the active node for all panel RPCs.
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
        type="button"
        title="Select data-plane node"
        onClick={() => setOpen((o) => !o)}
        className="flex items-center gap-1 px-2 py-0.5 text-[11px] rounded hover:bg-[#3a3a55] cursor-pointer text-[#e0e0e0] bg-transparent border-none whitespace-nowrap"
      >
        ▾ Nodes({nodes.length})
      </button>
      {open && (
        <>
          {/* Transparent overlay — clicking outside the dropdown closes it.
              Fixed positioning escapes the StatusBar's overflow-hidden. */}
          <div className="fixed inset-0 z-40" onClick={() => setOpen(false)} />
          <div
            className="fixed right-4 top-12 min-w-[280px] bg-[#1e1e36] border border-[#333355] rounded shadow-lg z-50 max-h-[400px] overflow-y-auto"
            onClick={(e) => e.stopPropagation()}
          >
            {error && (
              <div className="px-3 py-2 text-[#ff6060] text-xs">Failed to load nodes: {error}</div>
            )}
            {!error && nodes.length === 0 && (
              <div className="px-3 py-2 text-[#888] text-xs">No nodes available</div>
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
                    'flex items-center gap-2 px-3 py-2 cursor-pointer border-b border-[#333355] last:border-b-0 ' +
                    (selectable
                      ? (selected ? 'bg-[#2a2a55]' : 'hover:bg-[#2a2a44]')
                      : 'opacity-50 cursor-not-allowed')
                  }
                >
                  {/* Status dot: green = online, grey = offline */}
                  <span className={'w-2 h-2 rounded-full flex-shrink-0 ' + (node.status === 'online' ? 'bg-green-500' : 'bg-[#666]')} />
                  <span className="flex-1 min-w-0">
                    <span className="flex items-center gap-2">
                      {/* Node name — opens the NodeDetailPanel (stopPropagation
                          so the row click doesn't also select the node). */}
                      <span
                        title="Click to view node detail"
                        onClick={(e) => { e.stopPropagation(); handleViewDetail(node.node_id) }}
                        className="text-[#e0e0e0] text-sm font-medium truncate cursor-pointer hover:text-[#80a0ff]"
                      >
                        {node.name}
                      </span>
                      {selected && <span className="text-[#80c080] text-xs flex-shrink-0">✓</span>}
                    </span>
                    <span className="text-[#888] text-xs truncate block">
                      {node.node_id} · v{node.version}
                    </span>
                  </span>
                  <span className="flex-shrink-0 text-right">
                    <span className="block text-[#888] text-xs">
                      R:{node.load.running} Q:{node.load.queued}
                    </span>
                    {node.agent_count != null && (
                      <span className="block text-[#666] text-xs">{node.agent_count} agents</span>
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
