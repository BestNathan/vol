// frontend/src/components/panels/NodesPanel.tsx
// Data-plane node list, fetched from the CP via control.node_list. When
// viewingNodeDetailAtom is set (from the NodesDropdown name click), renders
// NodeDetailPanel instead; its "← Back" button clears the atom. Port of
// nodes_panel.rs.
import { useEffect, useState } from 'react'
import { useAtomValue } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { viewingNodeDetailAtom } from '@/stores/ui'
import { NodeDetailPanel } from '@/components/panels/NodeDetailPanel'
import type { NodeListEntry } from '@/types'
import type { RpcMethods } from '@/lib/protocol'

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

export function NodesPanel() {
  const viewingNodeDetail = useAtomValue(viewingNodeDetailAtom)
  const [nodes, setNodes] = useState<NodeListEntry[]>([])
  const [error, setError] = useState<string | null>(null)

  // While the detail view is open it owns the screen (and its own fetches);
  // the list is only loaded when showing the list itself.
  useEffect(() => {
    if (viewingNodeDetail) return
    let alive = true
    getPanelClient()
      .call<RpcMethods['control.node_list']['result']>('control.node_list')
      .then((res) => {
        if (!alive) return
        setNodes(res.nodes ?? [])
        setError(null)
      })
      .catch((err) => {
        if (alive) setError(errMsg(err))
      })
    return () => { alive = false }
  }, [viewingNodeDetail])

  if (viewingNodeDetail) {
    return <NodeDetailPanel />
  }

  return (
    <div className="flex flex-col h-full p-3 overflow-auto">
      <h2 className="text-lg font-bold mb-3 text-[#e0e0e0]">Nodes</h2>
      {error && <div className="text-red-400 text-sm">Error: {error}</div>}
      {!error && nodes.length === 0 && <div className="text-[#888] text-sm">No nodes connected</div>}
      {!error && nodes.map((node) => (
        <div key={node.node_id} className="flex items-center gap-3 p-2 border-b border-[#333355] hover:bg-[#2a2a44] rounded">
          <span className={'w-2 h-2 rounded-full flex-shrink-0 ' + (node.status === 'online' ? 'bg-green-500' : 'bg-[#666]')} />
          <span className="flex-1 min-w-0">
            <span className="block text-[#e0e0e0] text-sm font-medium truncate">{node.name}</span>
            <span className="block text-[#888] text-xs">id: {node.node_id} · v{node.version}</span>
          </span>
          {node.agent_count != null && (
            <span className="text-[#888] text-xs flex-shrink-0">{node.agent_count} agents</span>
          )}
        </div>
      ))}
    </div>
  )
}
