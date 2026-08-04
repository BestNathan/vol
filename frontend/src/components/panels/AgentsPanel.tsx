// frontend/src/components/panels/AgentsPanel.tsx
import { useCallback, useEffect } from 'react'
import { useAtom, useAtomValue, useSetAtom, useStore } from 'jotai'
import { JsonRpcClient } from '@/lib/jsonrpc-client'
import { deriveWsUrl } from '@/lib/ws-url'
import { formatToolArgs, truncatePreview } from '@/lib/event-handlers'
import { cn } from '@/lib/utils'
import { ConversationView } from '@/components/panels/ConversationView'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import {
  agentsAtom, selectedAgentIdAtom, agentsLoadingAtom, agentsErrorAtom,
  agentSubTabAtom, agentStatusMapAtom,
} from '@/stores/agents'
import { conversationMapAtom, activeAgentIdAtom } from '@/stores/conversation'
import { activeNodeIdAtom } from '@/stores/ui'
import { runMapAtom, runningAgentsAtom } from '@/stores/connection'
import type { AgentListEntry, AgentSubTab, ConversationEntry, SessionListEntry } from '@/types'
import type { SessionEntry } from '@/lib/protocol'

// --- Shared client -----------------------------------------------------------
// Temporary: this panel creates its own JsonRpcClient against the same WS URL.
// The shared client pattern (useClient hook) lands in a later task.
// autoSubscribe is off because App.tsx's client already subscribes to the event
// stream; this client is used purely for RPC calls.
let rpcClient: JsonRpcClient | null = null
function getClient(): JsonRpcClient {
  if (!rpcClient) rpcClient = new JsonRpcClient(deriveWsUrl(), { autoSubscribe: false })
  return rpcClient
}

// --- Session entry conversion -------------------------------------------------
// Raw session entry data shapes (see crates/vol-llm-ui/src/web/client.rs and
// the Dioxus sessions_panel.rs::session_entries_to_conversation, which this
// mirrors so resumed/running sessions render as the same timeline entries).
interface SessionMsg {
  role?: string
  name?: string
  content?: unknown
  thinking?: unknown
  tool_calls?: unknown
}

function messageText(content: unknown): string {
  if (typeof content === 'string') return content
  if (Array.isArray(content)) {
    return content
      .map((part) => {
        if (part && typeof part === 'object') {
          const rec = part as Record<string, unknown>
          if (typeof rec.text === 'string') return rec.text
          if (typeof rec.type === 'string') return rec.type
        }
        return ''
      })
      .filter(Boolean)
      .join('\n')
  }
  return ''
}

export function sessionEntriesToConversation(entries: SessionEntry[]): ConversationEntry[] {
  const out: ConversationEntry[] = []
  for (const e of entries) {
    const data = (e.data ?? {}) as {
      message?: { message?: SessionMsg }
      checkpoint?: { reason?: string; note?: string | null }
    }
    switch (e.type) {
      case 'message': {
        const msg = data.message?.message
        if (!msg) break
        const role = msg.role ?? ''
        const text = messageText(msg.content)
        if (role === 'user') {
          out.push({ type: 'UserInput', text })
        } else if (role === 'assistant') {
          const thinking = typeof msg.thinking === 'string' ? msg.thinking : ''
          if (thinking) out.push({ type: 'Thinking', content: thinking })
          if (Array.isArray(msg.tool_calls)) {
            for (const tc of msg.tool_calls) {
              const t = tc as { name?: string; arguments?: unknown }
              const fullArguments =
                typeof t.arguments === 'string' ? t.arguments : JSON.stringify(t.arguments ?? {})
              out.push({
                type: 'ToolCall',
                toolName: t.name ?? 'tool',
                argPreview: formatToolArgs(fullArguments),
                fullArguments,
              })
            }
          }
          out.push({ type: 'AgentAnswer', text })
        } else if (role === 'tool') {
          out.push({
            type: 'ToolResult',
            toolName: msg.name ?? 'tool',
            preview: truncatePreview(text, 200),
            fullResult: text,
            success: true,
          })
        }
        break
      }
      case 'checkpoint': {
        const cp = data.checkpoint
        out.push({
          type: 'EntryCheckpoint',
          reason: cp?.reason ?? 'Checkpoint',
          note: cp?.note ?? null,
          createdAt: e.created_at,
        })
        break
      }
      case 'summary': {
        out.push({ type: 'RunSummary', iterations: 0, toolCalls: 0, elapsedMs: 0 })
        break
      }
      default: break
    }
  }
  return out
}

// --- Agent card ----------------------------------------------------------------
function AgentCard({
  agent, isSelected, onSelect,
}: {
  agent: AgentListEntry
  isSelected: boolean
  onSelect: () => void
}) {
  const scopeStr = agent.scope ?? 'unknown'
  const scopeColor = scopeStr === 'repo' ? '#4080ff' : '#40c040'

  return (
    <button
      type="button"
      onClick={onSelect}
      className={cn(
        'flex items-center gap-2 px-2.5 py-2 rounded-lg cursor-pointer border w-full sm:w-auto text-left',
        isSelected
          ? 'border-[#80a0ff] bg-[#1a2a44]'
          : 'border-[#2a2a44] bg-[#1e1e36] hover:bg-[#222240]'
      )}
    >
      <span className="w-2 h-2 rounded-full bg-[#40c040] flex-shrink-0" />
      <span className="flex flex-col min-w-0 flex-1">
        <span className="flex items-center gap-1.5">
          <span className="font-semibold text-[13px] text-[#e0e0e0] truncate">{agent.name}</span>
          <span
            className="text-[9px] px-1 py-0.5 rounded-[2px] font-bold whitespace-nowrap flex-shrink-0"
            style={{ background: scopeColor, color: '#1a1a2e' }}
          >
            {scopeStr}
          </span>
        </span>
        <span className="text-[11px] text-[#666] truncate">{agent.description ?? ''}</span>
      </span>
    </button>
  )
}

// --- Placeholders (CapabilityBar + InputArea land in Task 2.5) ------------------
function CapabilityBarPlaceholder() {
  return (
    <div className="flex items-center gap-2 px-3 py-1.5 bg-[#1e1e36] border-t border-[#333355] text-[12px] text-[#888] flex-shrink-0">
      Capabilities — coming soon
    </div>
  )
}

function InputAreaPlaceholder() {
  return (
    <div className="flex-shrink-0 px-3 py-2 bg-[#252540] border-t border-[#333355]">
      <div className="rounded-lg border border-[#333355] bg-[#1a1a2e] px-3 py-2 text-[13px] text-[#666]">
        Input — coming soon
      </div>
    </div>
  )
}

// --- Panel ----------------------------------------------------------------------
const SUB_TABS: { id: AgentSubTab; label: string }[] = [
  { id: 'conversation', label: 'Conversation' },
  { id: 'sessions', label: 'Sessions' },
  { id: 'context', label: 'Context' },
  { id: 'tasks', label: 'Tasks' },
]

export function AgentsPanel() {
  const store = useStore()
  const nodeId = useAtomValue(activeNodeIdAtom)
  const [agents, setAgents] = useAtom(agentsAtom)
  const [selectedAgentId, setSelectedAgentId] = useAtom(selectedAgentIdAtom)
  const [loading, setLoading] = useAtom(agentsLoadingAtom)
  const [error, setError] = useAtom(agentsErrorAtom)
  const [subTab, setSubTab] = useAtom(agentSubTabAtom)
  const setActiveAgentId = useSetAtom(activeAgentIdAtom)
  const setConversationMap = useSetAtom(conversationMapAtom)

  const loadAgents = useCallback(async () => {
    if (!nodeId) {
      setAgents([])
      setError(null)
      setLoading(false)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const res = await getClient().call<{ agents: AgentListEntry[] }>('agent.list', { node_id: nodeId })
      setAgents(res.agents ?? [])
    } catch (err) {
      setAgents([])
      const message = (err as { message?: string } | null)?.message ?? String(err)
      setError(message)
    } finally {
      setLoading(false)
    }
  }, [nodeId, setAgents, setError, setLoading])

  // Fetch agent list on mount and whenever the active node changes. A node
  // switch clears stale per-node state (agents, selection, conversations).
  useEffect(() => {
    setSelectedAgentId(null)
    setActiveAgentId(null)
    setConversationMap(new Map())
    void loadAgents()
  }, [loadAgents, setSelectedAgentId, setActiveAgentId, setConversationMap])

  // When selecting an agent that is mid-run, load its latest session into the
  // conversation with a RunningBanner prepended (mirrors agents_panel.rs).
  const checkAgentRunning = useCallback(async (agentId: string) => {
    try {
      const status = await getClient().call<{ status: string; run_id?: string }>('agent.status', { agent_id: agentId })
      if (status.status !== 'running' || !status.run_id) return
      const sessions = await getClient().call<{ sessions: SessionListEntry[] }>('session.list', { agent_id: agentId })
      const latest = sessions.sessions?.[0]
      if (latest) {
        const res = await getClient().call<{ entries: SessionEntry[] }>('session.entries', {
          session_id: latest.id,
          agent_id: agentId,
        })
        const convEntries = sessionEntriesToConversation(res.entries ?? [])
        const map = new Map(store.get(conversationMapAtom))
        map.set(agentId, {
          entries: [{ type: 'RunningBanner', runId: status.run_id }, ...convEntries],
          autoScroll: true,
        })
        store.set(conversationMapAtom, map)
      }
      // Mark the agent running so event attribution (runMap) stays consistent.
      const statusMap = { ...store.get(agentStatusMapAtom) }
      statusMap[agentId] = { status: 'running', runId: status.run_id }
      store.set(agentStatusMapAtom, statusMap)
      const runMap = new Map(store.get(runMapAtom))
      runMap.set(status.run_id, agentId)
      store.set(runMapAtom, runMap)
      const runningAgents = new Set(store.get(runningAgentsAtom))
      runningAgents.add(agentId)
      store.set(runningAgentsAtom, runningAgents)
    } catch {
      // Best-effort: a failed status check must not block agent selection.
    }
  }, [store])

  const handleAgentClick = useCallback((agent: AgentListEntry) => {
    if (selectedAgentId === agent.id) {
      setSelectedAgentId(null)
      setActiveAgentId(null)
      return
    }
    setSelectedAgentId(agent.id)
    setActiveAgentId(agent.id)
    setSubTab('conversation')
    void checkAgentRunning(agent.id)
  }, [selectedAgentId, setSelectedAgentId, setActiveAgentId, setSubTab, checkAgentRunning])

  const selectedAgent = agents.find((a) => a.id === selectedAgentId) ?? null

  // Empty states
  if (!nodeId) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="text-center">
          <div className="text-[#888] text-[14px]">Select a node to view agents</div>
          <div className="text-[#666] text-[12px] mt-1">Select a node from the dropdown above to view its agents.</div>
        </div>
      </div>
    )
  }

  if (loading && agents.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2 text-[#888] text-[14px]">
        <span className="w-4 h-4 rounded-full border-2 border-[#333355] border-t-[#80a0ff] animate-spin" />
        Loading agents...
      </div>
    )
  }

  if (error && agents.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="text-[#ff6060] text-[14px]">Failed to load agents</div>
          <div className="text-[#888] text-[12px] max-w-[300px] break-words">{error}</div>
          <Button variant="outline" size="sm" onClick={() => void loadAgents()}>Retry</Button>
        </div>
      </div>
    )
  }

  if (agents.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center text-[#888] text-[14px]">
        No agents available
      </div>
    )
  }

  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
      {/* Card grid — scrollable, stacks on mobile, wraps on desktop */}
      <div className="flex flex-col sm:flex-row sm:flex-wrap gap-2 p-2 border-b border-[#333355] overflow-y-auto max-h-[200px] min-h-[60px] flex-shrink-0">
        {agents.map((agent) => (
          <AgentCard
            key={agent.id}
            agent={agent}
            isSelected={agent.id === selectedAgentId}
            onSelect={() => handleAgentClick(agent)}
          />
        ))}
      </div>

      {/* Info bar: selected agent name + description */}
      {selectedAgent && (
        <div className="flex items-center gap-2 px-3 py-1.5 bg-[#1a2a44] border-b border-[#333355] flex-shrink-0">
          <span className="font-bold text-[13px] text-[#e0e0e0] truncate">{selectedAgent.name}</span>
          <span className="text-[12px] text-[#888] truncate hidden sm:inline">
            {selectedAgent.description ?? ''}
          </span>
        </div>
      )}

      {selectedAgent ? (
        <Tabs
          value={subTab}
          onValueChange={(v) => setSubTab(v as AgentSubTab)}
          className="flex-1 min-h-0 flex flex-col overflow-hidden"
        >
          <TabsList className="h-9 justify-start w-full gap-0 p-0 rounded-none bg-[#252540] border-b border-[#333355] flex-shrink-0">
            {SUB_TABS.map((t) => (
              <TabsTrigger
                key={t.id}
                value={t.id}
                className="h-9 rounded-none px-3 py-1.5 text-[12px] font-semibold border-b-2 border-transparent data-[state=active]:bg-[#1a1a2e] data-[state=active]:text-[#e0e0e0] data-[state=active]:border-[#80a0ff] data-[state=active]:shadow-none"
              >
                {t.label}
              </TabsTrigger>
            ))}
          </TabsList>

          <TabsContent value="conversation" className="flex-1 min-h-0 flex flex-col overflow-hidden mt-0">
            <ConversationView />
            <CapabilityBarPlaceholder />
            <InputAreaPlaceholder />
          </TabsContent>
          <TabsContent value="sessions" className="flex-1 min-h-0 mt-0">
            <div className="flex items-center justify-center h-full text-[#666] text-sm">Sessions — coming soon</div>
          </TabsContent>
          <TabsContent value="context" className="flex-1 min-h-0 mt-0">
            <div className="flex items-center justify-center h-full text-[#666] text-sm">Context — coming soon</div>
          </TabsContent>
          <TabsContent value="tasks" className="flex-1 min-h-0 mt-0">
            <div className="flex items-center justify-center h-full text-[#666] text-sm">Tasks — coming soon</div>
          </TabsContent>
        </Tabs>
      ) : (
        <div className="flex-1 flex items-center justify-center text-[#666] text-[14px]">
          Select an agent to start
        </div>
      )}
    </div>
  )
}
