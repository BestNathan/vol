// frontend/src/components/panels/AgentsPanel.tsx
import { useCallback, useEffect } from 'react'
import { useAtom, useAtomValue, useSetAtom, useStore } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { sessionEntriesToConversation } from '@/lib/session-conversion'
import { cn } from '@/lib/utils'
import { ConversationView } from '@/components/panels/ConversationView'
import { SessionsPanel } from '@/components/panels/SessionsPanel'
import { ContextPanel } from '@/components/panels/ContextPanel'
import { InputArea } from '@/components/inputs/InputArea'
import { CapabilityBar } from '@/components/inputs/CapabilityBar'
import { CapabilityDrawer } from '@/components/inputs/CapabilityDrawer'
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import {
  agentsAtom, selectedAgentIdAtom, agentsLoadingAtom, agentsErrorAtom,
  agentSubTabAtom, agentStatusMapAtom,
} from '@/stores/agents'
import { conversationMapAtom, activeAgentIdAtom } from '@/stores/conversation'
import { activeNodeIdAtom } from '@/stores/ui'
import { runMapAtom, runningAgentsAtom } from '@/stores/connection'
import type { AgentListEntry, AgentSubTab, SessionListEntry } from '@/types'
import type { SessionEntry } from '@/lib/protocol'

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
    <Button
      type="button"
      variant="ghost"
      onClick={onSelect}
      className={cn(
        'cursor-pointer flex items-center gap-2 px-2.5 py-2 rounded-lg border w-full sm:w-auto text-left h-auto justify-start',
        isSelected
          ? 'border-primary bg-[#1a2a44] hover:bg-[#1a2a44]'
          : 'border-[#2a2a44] bg-card hover:bg-secondary/50'
      )}
    >
      <span className="w-2 h-2 rounded-full bg-[#40c040] flex-shrink-0" />
      <span className="flex flex-col min-w-0 flex-1">
        <span className="flex items-center gap-1.5">
          <span className="font-semibold text-[13px] text-foreground truncate">{agent.name}</span>
          <Badge variant="outline" className="text-[9px] font-bold whitespace-nowrap flex-shrink-0"
            style={{ color: scopeColor, borderColor: scopeColor }}>
            {scopeStr}
          </Badge>
        </span>
        <span className="text-[11px] text-muted-foreground/70 truncate">{agent.description ?? ''}</span>
      </span>
    </Button>
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
      const res = await getPanelClient().call<{ agents: AgentListEntry[] }>('agent.list', { node_id: nodeId })
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
      const status = await getPanelClient().call<{ status: string; run_id?: string }>('agent.status', { agent_id: agentId })
      if (status.status !== 'running' || !status.run_id) return
      const sessions = await getPanelClient().call<{ sessions: SessionListEntry[] }>('session.list', { agent_id: agentId })
      const latest = sessions.sessions?.[0]
      if (latest) {
        const res = await getPanelClient().call<{ entries: SessionEntry[] }>('session.entries', {
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
          <div className="text-muted-foreground text-[14px]">Select a node to view agents</div>
          <div className="text-muted-foreground/70 text-[12px] mt-1">Select a node from the dropdown above to view its agents.</div>
        </div>
      </div>
    )
  }

  if (loading && agents.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2 text-muted-foreground text-[14px]">
        <span className="w-4 h-4 rounded-full border-2 border-border border-t-[#80a0ff] animate-spin" />
        Loading agents...
      </div>
    )
  }

  if (error && agents.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="text-destructive text-[14px]">Failed to load agents</div>
          <div className="text-muted-foreground text-[12px] max-w-[300px] break-words">{error}</div>
          <Button variant="outline" size="sm" className="cursor-pointer" onClick={() => void loadAgents()}>Retry</Button>
        </div>
      </div>
    )
  }

  if (agents.length === 0) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center text-muted-foreground text-[14px]">
        No agents available
      </div>
    )
  }

  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
      {/* Fixed right-side overlay — rendered at panel level so it stays open
          across sub-tab switches while an agent is selected. */}
      <CapabilityDrawer />
      {/* Card grid — scrollable, stacks on mobile, wraps on desktop */}
      <div className="flex flex-col sm:flex-row sm:flex-wrap gap-2 p-2 border-b border-border overflow-y-auto max-h-[200px] min-h-[60px] flex-shrink-0">
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
        <div className="flex items-center gap-2 px-3 py-1.5 bg-[#1a2a44] border-b border-border flex-shrink-0">
          <span className="font-bold text-[13px] text-foreground truncate">{selectedAgent.name}</span>
          <span className="text-[12px] text-muted-foreground truncate hidden sm:inline">
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
          <TabsList className="h-9 justify-start w-full gap-0 p-0 rounded-none bg-card border-b border-border flex-shrink-0 overflow-x-auto">
            {SUB_TABS.map((t) => (
              <TabsTrigger
                key={t.id}
                value={t.id}
                className="h-9 rounded-none px-3 py-1.5 text-[12px] font-semibold border-b-2 border-transparent data-[state=active]:bg-background data-[state=active]:text-foreground data-[state=active]:border-primary data-[state=active]:shadow-none"
              >
                {t.label}
              </TabsTrigger>
            ))}
          </TabsList>

          <TabsContent value="conversation" className="flex-1 min-h-0 flex flex-col overflow-hidden mt-0">
            <ConversationView />
            <CapabilityBar />
            <InputArea />
          </TabsContent>
          <TabsContent value="sessions" className="flex-1 min-h-0 mt-0 flex flex-col overflow-hidden">
            <SessionsPanel />
          </TabsContent>
          <TabsContent value="context" className="flex-1 min-h-0 mt-0 flex flex-col overflow-hidden">
            <ContextPanel />
          </TabsContent>
          <TabsContent value="tasks" className="flex-1 min-h-0 mt-0">
            <div className="flex items-center justify-center h-full text-muted-foreground/70 text-sm">Tasks — coming soon</div>
          </TabsContent>
        </Tabs>
      ) : (
        <div className="flex-1 flex items-center justify-center text-muted-foreground/70 text-[14px]">
          Select an agent to start
        </div>
      )}
    </div>
  )
}
