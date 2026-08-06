// frontend/src/components/panels/ContextPanel.tsx
// Context tab: contributor list for the selected agent with anchor-zone
// badges and token/message counts; clicking a contributor fetches
// agent.context_snapshot and opens the ContextDialog. Fetches
// agent.context_config on mount and whenever the selected agent changes
// (stale-response guard drops responses from a previous agent). Port of
// context_panel.rs.
import { useCallback, useEffect, useRef } from 'react'
import { useAtom } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import {
  contributorsAtom, contextLoadingAtom, contextErrorAtom, contextDialogAtom,
} from '@/stores/context'
import { selectedAgentIdAtom } from '@/stores/agents'
import { ContextDialog } from '@/components/dialogs/ContextDialog'
import { Button } from '@/components/ui/button'
import { Badge } from '@/components/ui/badge'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { RpcMethods } from '@/lib/protocol'
import type { ContributorInfoEntry } from '@/types'

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

/** Anchor zone badge color: head blue, middle gold, tail green. */
export function anchorZoneColor(zone: string): string {
  switch (zone) {
    case 'head': return '#4080ff'
    case 'middle': return '#c0a040'
    case 'tail': return '#40c040'
    default: return '#888'
  }
}

export function ContextPanel() {
  const [contributors, setContributors] = useAtom(contributorsAtom)
  const [loading, setLoading] = useAtom(contextLoadingAtom)
  const [error, setError] = useAtom(contextErrorAtom)
  const [, setDialog] = useAtom(contextDialogAtom)
  const [selectedAgentId] = useAtom(selectedAgentIdAtom)

  // Live agent mirror for the stale-response guard in async callbacks.
  const agentIdRef = useRef(selectedAgentId)
  useEffect(() => { agentIdRef.current = selectedAgentId }, [selectedAgentId])

  // Fetch the contributor list; writes are dropped once the selected agent no
  // longer matches the agent this fetch was started for.
  const loadContributors = useCallback(async (agentId: string | null) => {
    if (!agentId) {
      setContributors([])
      setError(null)
      setLoading(false)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const res = await getPanelClient().call<RpcMethods['agent.context_config']['result']>(
        'agent.context_config',
        { agent_id: agentId }
      )
      if (agentIdRef.current !== agentId) return
      setContributors(res.contributors ?? [])
    } catch (err) {
      if (agentIdRef.current !== agentId) return
      setError(errMsg(err))
    } finally {
      if (agentIdRef.current === agentId) setLoading(false)
    }
  }, [setContributors, setError, setLoading])

  // Fetch on mount and whenever the selected agent changes; a new agent also
  // closes any open snapshot dialog (it belongs to the previous agent).
  useEffect(() => {
    setDialog({ open: false, contributorName: '', messages: [], loading: false })
    void loadContributors(selectedAgentId)
  }, [loadContributors, selectedAgentId, setDialog])

  // Click a contributor: fetch its message snapshot into the dialog.
  const openSnapshot = useCallback((contributor: ContributorInfoEntry) => {
    const agentId = agentIdRef.current
    if (!agentId) return
    const name = contributor.name
    setDialog({ open: true, contributorName: name, messages: [], loading: true })
    getPanelClient().call<RpcMethods['agent.context_snapshot']['result']>('agent.context_snapshot', {
      agent_id: agentId,
      contributor_name: name,
    })
      .then((res) => {
        setDialog((d) =>
          d.open && d.contributorName === name
            ? { ...d, messages: res.messages ?? [], loading: false }
            : d
        )
      })
      .catch((err) => {
        setDialog((d) =>
          d.open && d.contributorName === name
            ? { ...d, loading: false, error: `Failed to load snapshot: ${errMsg(err)}` }
            : d
        )
      })
  }, [setDialog])

  return (
    <div className="flex flex-col flex-1 min-h-0 overflow-hidden">
      {loading ? (
        <div className="flex items-center justify-center h-full text-muted-foreground/70 text-[14px]">
          Loading contributors...
        </div>
      ) : error !== null ? (
        <ScrollArea className="flex-1">
          <div className="h-full flex flex-col items-center gap-3 p-3">
            <div className="text-destructive text-[14px]">Failed to load context</div>
            <div className="text-muted-foreground text-[12px] max-w-[300px] break-words text-center">{error}</div>
            <Button variant="outline" size="sm" className="cursor-pointer" onClick={() => void loadContributors(selectedAgentId)}>
              Retry
            </Button>
          </div>
        </ScrollArea>
      ) : contributors.length === 0 ? (
        <div className="flex items-center justify-center h-full text-muted-foreground text-[14px]">
          No contributors configured
        </div>
      ) : (
        <ScrollArea className="flex-1">
          <div>
            {contributors.map((c) => (
              <div
                key={c.name}
                className="flex items-center gap-3 px-3 py-2 border-b border-[#2a2a44] cursor-pointer hover:bg-secondary"
                onClick={() => openSnapshot(c)}
              >
                <Badge variant="outline" className="cursor-pointer text-[9px] font-bold flex-shrink-0"
                  style={{ color: anchorZoneColor(c.anchor_zone), borderColor: anchorZoneColor(c.anchor_zone) }}>
                  {c.anchor_zone}
                </Badge>
                <span className="font-semibold text-[13px] text-foreground flex-1 min-w-0 truncate">
                  {c.name}
                </span>
                <span className="text-[11px] text-muted-foreground flex-shrink-0">{c.estimated_tokens} tokens</span>
                <span className="text-[11px] text-muted-foreground/70 flex-shrink-0">{c.message_count} msg</span>
              </div>
            ))}
          </div>
        </ScrollArea>
      )}
      <ContextDialog />
    </div>
  )
}
