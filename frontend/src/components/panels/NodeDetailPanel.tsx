// frontend/src/components/panels/NodeDetailPanel.tsx
// CP-scoped detail view for a single data-plane node. Four sections:
// Overview (id/name/version/status/last_seen/capability_revision),
// Resource Usage (running/queued stat cards), Agents on Node (via a direct DP
// connection created from the node's ws_url), Capabilities (badge counts +
// lists from control.capability_list). node_get is re-polled every 5 s; the
// interval is cleared on unmount / node switch. Port of node_detail_panel.rs.
import { useEffect, useState } from 'react'
import { useAtomValue, useSetAtom } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { dpPool } from '@/lib/dp-pool'
import { viewingNodeDetailAtom } from '@/stores/ui'
import type { AgentListEntry, NodeListEntry } from '@/types'
import type { CapabilitySnapshot, RpcMethods } from '@/lib/protocol'

const REFRESH_MS = 5_000

interface NodeDetailState {
  node: NodeListEntry | null
  agents: AgentListEntry[]
  capabilities: CapabilitySnapshot | null
  loading: boolean
  error: string | null
}

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

/** Format a millisecond timestamp as a human-readable age ("3m ago"). */
export function formatAge(ms: number): string {
  const diffSecs = Math.max(0, Math.floor((Date.now() - ms) / 1000))
  if (diffSecs < 60) return `${diffSecs}s ago`
  if (diffSecs < 3600) return `${Math.floor(diffSecs / 60)}m ago`
  if (diffSecs < 86400) return `${Math.floor(diffSecs / 3600)}h ago`
  return `${Math.floor(diffSecs / 86400)}d ago`
}

export function NodeDetailPanel() {
  const nodeId = useAtomValue(viewingNodeDetailAtom)
  const setViewingNodeDetail = useSetAtom(viewingNodeDetailAtom)
  const [state, setState] = useState<NodeDetailState>({
    node: null, agents: [], capabilities: null, loading: true, error: null,
  })

  useEffect(() => {
    if (!nodeId) return
    let alive = true
    const client = getPanelClient()

    setState({ node: null, agents: [], capabilities: null, loading: true, error: null })

    // 1. Fetch node detail; 2. agents via DP connection; 3. capabilities.
    const loadAll = async () => {
      let node: NodeListEntry | null = null
      try {
        const res = await client.call<RpcMethods['control.node_get']['result']>('control.node_get', { node_id: nodeId })
        node = res.node ?? null
      } catch (err) {
        if (alive) setState((s) => ({ ...s, error: errMsg(err), loading: false }))
        return
      }
      if (!alive) return
      if (!node) {
        setState((s) => ({ ...s, error: 'Node not found', loading: false }))
        return
      }
      setState((s) => ({ ...s, node, loading: false, error: null }))

      // Agents on this node — direct DP connection (best-effort: nodes
      // without a ws_url simply skip the agents section).
      if (node.ws_url) {
        const dp = dpPool.getOrCreate(node.node_id, node.ws_url)
        try {
          const res = await dp.call<RpcMethods['agent.list']['result']>('agent.list')
          if (alive) setState((s) => ({ ...s, agents: res.agents ?? [] }))
        } catch {
          // Transient — keep the rest of the panel usable.
        }
      }

      // Capability snapshot for this node.
      try {
        const res = await client.call<RpcMethods['control.capability_list']['result']>('control.capability_list', { node_id: nodeId })
        if (alive) setState((s) => ({ ...s, capabilities: res.snapshots?.[0] ?? null }))
      } catch {
        // Best-effort — capabilities section shows "no data" on failure.
      }
    }

    // Auto-refresh: re-poll node_get (keeps load counts fresh). Transient
    // errors keep the last-known-good data; a disappeared node surfaces an
    // error and polling stops on the next tick via the !alive guard.
    const refreshNode = async () => {
      try {
        const res = await client.call<RpcMethods['control.node_get']['result']>('control.node_get', { node_id: nodeId })
        if (!alive) return
        if (!res.node) {
          setState((s) => ({ ...s, error: 'Node not found' }))
          return
        }
        setState((s) => ({ ...s, node: res.node, error: null }))
      } catch {
        // Ignore transient poll errors — retain last-known-good.
      }
    }

    void loadAll()
    const timer = setInterval(() => { void refreshNode() }, REFRESH_MS)
    return () => { alive = false; clearInterval(timer) }
  }, [nodeId])

  if (!nodeId) return null

  const { node, agents, capabilities, loading, error } = state

  return (
    <div className="flex flex-col h-full p-3 overflow-auto">
      <button
        type="button"
        onClick={() => setViewingNodeDetail(null)}
        className="self-start flex items-center gap-1 px-2 py-1 mb-3 text-xs text-[#80a0ff] bg-transparent border border-[#333355] rounded cursor-pointer hover:bg-[#2a2a44]"
      >
        ← Back
      </button>

      {loading && <div className="text-[#888] text-sm">Loading node detail...</div>}
      {!loading && error && <div className="text-red-400 text-sm">Error: {error}</div>}
      {!loading && !error && node && (
        <>
          <OverviewSection node={node} />
          <ResourceSection node={node} />
          <AgentsSection agents={agents} />
          <CapabilitiesSection capabilities={capabilities} />
        </>
      )}
    </div>
  )
}

// --- Overview -----------------------------------------------------------------
function OverviewSection({ node }: { node: NodeListEntry }) {
  const statusCls = node.status === 'online' ? 'text-green-400' : 'text-red-400'
  const lastSeenLabel = node.last_seen_at_ms != null ? formatAge(node.last_seen_at_ms) : 'never'
  return (
    <div className="mb-4">
      <h3 className="text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1">Overview</h3>
      <div className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1 text-sm">
        <span className="text-[#888]">Node ID:</span>
        <span className="text-[#e0e0e0] font-mono text-xs">{node.node_id}</span>
        <span className="text-[#888]">Name:</span>
        <span className="text-[#e0e0e0]">{node.name}</span>
        <span className="text-[#888]">Version:</span>
        <span className="text-[#e0e0e0]">v{node.version}</span>
        <span className="text-[#888]">Status:</span>
        <span className={statusCls}>{node.status}</span>
        <span className="text-[#888]">Last Seen:</span>
        <span className="text-[#e0e0e0]">{lastSeenLabel}</span>
        <span className="text-[#888]">Cap Revision:</span>
        <span className="text-[#e0e0e0]">{node.capability_revision}</span>
      </div>
    </div>
  )
}

// --- Resource Usage -----------------------------------------------------------
function ResourceSection({ node }: { node: NodeListEntry }) {
  return (
    <div className="mb-4">
      <h3 className="text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1">Resource Usage</h3>
      <div className="flex gap-6">
        <div className="flex flex-col items-center px-4 py-2 rounded bg-[#1a1a2e] border border-[#2a2a44]">
          <span className="text-2xl font-bold text-[#80a0ff]">{node.load.running}</span>
          <span className="text-xs text-[#888]">Running</span>
        </div>
        <div className="flex flex-col items-center px-4 py-2 rounded bg-[#1a1a2e] border border-[#2a2a44]">
          <span className="text-2xl font-bold text-[#f0c040]">{node.load.queued}</span>
          <span className="text-xs text-[#888]">Queued</span>
        </div>
      </div>
    </div>
  )
}

// --- Agents on Node -----------------------------------------------------------
function AgentsSection({ agents }: { agents: AgentListEntry[] }) {
  return (
    <div className="mb-4">
      <h3 className="text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1">
        Agents on this Node ({agents.length})
      </h3>
      {agents.length === 0 ? (
        <div className="text-[#888] text-sm">No agents on this node</div>
      ) : (
        <div className="flex flex-col gap-1">
          {agents.map((agent) => (
            <AgentRow key={agent.id} agent={agent} />
          ))}
        </div>
      )}
    </div>
  )
}

function AgentRow({ agent }: { agent: AgentListEntry }) {
  const scopeStr = agent.scope ?? 'unknown'
  const scopeColor = scopeStr === 'repo' ? '#4080ff' : scopeStr === 'user' ? '#40c040' : '#888'
  return (
    <div className="flex items-center gap-2 px-2 py-1.5 rounded border-b border-[#333355] hover:bg-[#2a2a44]">
      <span className="w-2 h-2 rounded-full bg-[#40c040] flex-shrink-0" />
      <span className="flex-1 min-w-0">
        <span className="flex items-center gap-1.5">
          <span className="text-[#e0e0e0] text-sm font-medium truncate">{agent.name}</span>
          <span
            className="text-[9px] px-1 py-0.5 rounded-[2px] font-bold whitespace-nowrap flex-shrink-0"
            style={{ background: scopeColor, color: '#1a1a2e' }}
          >
            {scopeStr}
          </span>
        </span>
        <span className="text-[#666] text-xs truncate block">{agent.description ?? ''}</span>
      </span>
    </div>
  )
}

// --- Capabilities -------------------------------------------------------------
function CapabilitiesSection({ capabilities }: { capabilities: CapabilitySnapshot | null }) {
  if (!capabilities) {
    return (
      <div className="mb-4">
        <h3 className="text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1">Capabilities</h3>
        <div className="text-[#888] text-sm">No capability data available</div>
      </div>
    )
  }
  return (
    <div className="mb-4">
      <h3 className="text-sm font-semibold text-[#e0e0e0] mb-2 border-b border-[#333355] pb-1">Capabilities</h3>
      <div className="flex gap-4 flex-wrap">
        <CapBadge label="Agents" count={capabilities.agents.length} color="#40c040" />
        <CapBadge label="Tools" count={capabilities.tools.length} color="#80a0ff" />
        <CapBadge label="Skills" count={capabilities.skills.length} color="#f0c040" />
        <CapBadge label="MCP Servers" count={capabilities.mcp_servers.length} color="#c080ff" />
      </div>
      {capabilities.tools.length > 0 && (
        <div className="mt-3">
          <div className="text-xs text-[#888] mb-1">Tools</div>
          <div className="flex flex-col gap-0.5">
            {capabilities.tools.map((tool) => (
              <div key={tool.name} className="text-sm text-[#e0e0e0] px-2 py-0.5 rounded hover:bg-[#2a2a44]">
                <span className="font-mono text-xs">{tool.name}</span>
                {tool.description && <span className="text-[#666] text-xs ml-2">{tool.description}</span>}
              </div>
            ))}
          </div>
        </div>
      )}
      {capabilities.skills.length > 0 && (
        <div className="mt-3">
          <div className="text-xs text-[#888] mb-1">Skills</div>
          <div className="flex flex-col gap-0.5">
            {capabilities.skills.map((skill) => (
              <div key={skill.name} className="text-sm text-[#e0e0e0] px-2 py-0.5 rounded hover:bg-[#2a2a44]">
                <span className="font-mono text-xs">{skill.name}</span>
                {skill.description && <span className="text-[#666] text-xs ml-2">{skill.description}</span>}
              </div>
            ))}
          </div>
        </div>
      )}
      {capabilities.mcp_servers.length > 0 && (
        <div className="mt-3">
          <div className="text-xs text-[#888] mb-1">MCP Servers</div>
          <div className="flex flex-col gap-0.5">
            {capabilities.mcp_servers.map((mcp) => (
              <div key={mcp.name} className="flex items-center gap-2 px-2 py-0.5 rounded hover:bg-[#2a2a44]">
                <span className="font-mono text-xs text-[#e0e0e0]">{mcp.name}</span>
                {mcp.status && <span className="text-xs text-[#888]">({mcp.status})</span>}
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  )
}

function CapBadge({ label, count, color }: { label: string; count: number; color: string }) {
  return (
    <div className="flex flex-col items-center px-3 py-1.5 rounded bg-[#1a1a2e] border border-[#2a2a44]">
      <span className="text-lg font-bold" style={{ color }}>{count}</span>
      <span className="text-[10px] text-[#888]">{label}</span>
    </div>
  )
}
