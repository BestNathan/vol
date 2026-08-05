// frontend/src/components/panels/SessionsPanel.tsx
// Sessions sub-tab (port of sessions_panel.rs): lists persisted sessions for
// the selected agent — cards on mobile, rows on desktop, each showing the
// truncated id, entry count and age. Clicking a session opens
// SessionDetailOverlay (fetch + parse of session.entries); Resume swaps the
// session into the agent's conversation and returns to the Conversation tab.
import { useCallback, useEffect, useRef, useState } from 'react'
import { useAtom, useAtomValue, useSetAtom } from 'jotai'
import { getPanelClient } from '@/lib/panel-client'
import { sessionEntriesToConversation } from '@/lib/session-conversion'
import { sessionsAtom, sessionsErrorAtom, sessionsLoadingAtom } from '@/stores/sessions'
import { agentSubTabAtom, selectedAgentIdAtom } from '@/stores/agents'
import { activeTabAtom } from '@/stores/ui'
import { conversationMapAtom } from '@/stores/conversation'
import { SessionDetailOverlay } from '@/components/dialogs/SessionDetailOverlay'
import { Button } from '@/components/ui/button'
import type { RpcMethods } from '@/lib/protocol'
import type { SessionListEntry } from '@/types'

function errMsg(err: unknown): string {
  return (err as { message?: string } | null)?.message ?? String(err)
}

/** Unix-seconds timestamp as a human-readable age label (sessions_panel.rs::format_age). */
export function formatAge(ts: number): string {
  const now = Math.floor(Date.now() / 1000)
  const diff = Math.max(0, now - ts)
  if (diff < 60) return `${diff}s ago`
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`
  return `${Math.floor(diff / 86400)}d ago`
}

/** Session ids are UUIDs; show the first 12 chars (sessions_panel.rs::truncate_id). */
export function truncateId(id: string): string {
  return id.length > 12 ? `${id.slice(0, 12)}...` : id
}

export function SessionsPanel() {
  const selectedAgentId = useAtomValue(selectedAgentIdAtom)
  const [sessions, setSessions] = useAtom(sessionsAtom)
  const [loading, setLoading] = useAtom(sessionsLoadingAtom)
  const [error, setError] = useAtom(sessionsErrorAtom)
  const [overlaySession, setOverlaySession] = useState<SessionListEntry | null>(null)
  const [resumingId, setResumingId] = useState<string | null>(null)
  const setConversationMap = useSetAtom(conversationMapAtom)
  const setSubTab = useSetAtom(agentSubTabAtom)
  const setActiveTab = useSetAtom(activeTabAtom)

  // Live mirror of the agent the panel currently shows, for the stale-response
  // guard in async callbacks (agent switch mid-flight must drop the response).
  const agentRef = useRef(selectedAgentId)
  useEffect(() => { agentRef.current = selectedAgentId }, [selectedAgentId])

  const loadSessions = useCallback(async (target: string | null) => {
    if (!target) {
      setSessions([])
      setLoading(false)
      setError(null)
      return
    }
    setLoading(true)
    setError(null)
    try {
      const res = await getPanelClient().call<RpcMethods['session.list']['result']>('session.list', { agent_id: target })
      if (agentRef.current !== target) return
      setSessions(res.sessions ?? [])
    } catch (err) {
      if (agentRef.current !== target) return
      setError(errMsg(err))
    } finally {
      if (agentRef.current === target) setLoading(false)
    }
  }, [setSessions, setLoading, setError])

  // Fetch on mount and whenever the selected agent changes; close any overlay
  // that belongs to a previous agent.
  useEffect(() => {
    setOverlaySession(null)
    void loadSessions(selectedAgentId)
  }, [loadSessions, selectedAgentId])

  // Resume: swap the persisted session into the agent's conversation, then
  // return to the Conversation sub-tab. The button self-resets after 15s even
  // if the response never arrives (safety timeout, mirrors the Rust
  // TimeoutFuture::new(15_000)).
  const handleResume = useCallback(async (session: SessionListEntry) => {
    if (!selectedAgentId || resumingId !== null) return
    setResumingId(session.id)
    const reset = () => setResumingId((id) => (id === session.id ? null : id))
    const timer = window.setTimeout(reset, 15_000)
    try {
      const res = await getPanelClient().call<RpcMethods['session.resume']['result']>('session.resume', {
        session_id: session.id,
        agent_id: selectedAgentId,
      })
      const convEntries = sessionEntriesToConversation(res.entries ?? [])
      setConversationMap((prev) => {
        const map = new Map(prev)
        map.set(selectedAgentId, { entries: convEntries, autoScroll: true })
        return map
      })
      setSubTab('conversation')
      setActiveTab('agents')
    } catch (err) {
      // Non-fatal: keep the list visible; the button resets via the timeout/finally.
      console.error('Failed to resume session:', err)
    } finally {
      window.clearTimeout(timer)
      reset()
    }
  }, [selectedAgentId, resumingId, setConversationMap, setSubTab, setActiveTab])

  const resumeButton = (session: SessionListEntry) => (
    <button
      type="button"
      className="px-2.5 py-0.5 bg-[#408040] text-foreground border-none rounded-[3px] cursor-pointer text-[12px] flex-shrink-0 hover:bg-[#50a050] disabled:bg-[#333355] disabled:cursor-not-allowed"
      disabled={resumingId !== null}
      onClick={(e) => {
        e.stopPropagation()
        void handleResume(session)
      }}
    >
      {resumingId === session.id ? 'Resuming...' : 'Resume'}
    </button>
  )

  if (loading && sessions.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center gap-2 text-muted-foreground text-[14px]">
        <span className="w-4 h-4 rounded-full border-2 border-border border-t-[#80a0ff] animate-spin" />
        Loading sessions...
      </div>
    )
  }

  if (error !== null) {
    return (
      <div className="flex-1 overflow-y-auto p-3 flex items-center justify-center">
        <div className="flex flex-col items-center gap-3 text-center">
          <div className="text-destructive text-[14px]">Failed to load sessions</div>
          <div className="text-muted-foreground text-[12px] max-w-[300px] break-words">{error}</div>
          <Button variant="outline" size="sm" onClick={() => void loadSessions(selectedAgentId)}>Retry</Button>
        </div>
      </div>
    )
  }

  return (
    <div className="flex-1 min-h-0 overflow-y-auto p-2">
      <div className="px-2.5 pt-1 pb-2 text-[12px] font-semibold text-muted-foreground uppercase tracking-[0.5px]">Sessions</div>
      {sessions.length === 0 ? (
        <div className="flex items-center justify-center h-32 text-muted-foreground/70 text-[13px]">No sessions found</div>
      ) : (
        <>
          {/* Mobile: session cards */}
          <div className="sm:hidden flex flex-col gap-2">
            {sessions.map((s) => (
              <div
                key={s.id}
                className="cursor-pointer rounded-lg border border-border bg-secondary p-3 active:bg-secondary"
                onClick={() => setOverlaySession(s)}
              >
                <div className="flex items-center justify-between">
                  <span className="font-mono text-[13px] text-foreground font-semibold truncate">{truncateId(s.id)}</span>
                  <span className="bg-secondary text-foreground/70 rounded-full px-2 py-0.5 text-[11px] flex-shrink-0 ml-2">
                    {s.entry_count} entries
                  </span>
                </div>
                <div className="flex items-center justify-between mt-1.5">
                  <span className="text-[11px] text-muted-foreground/70">{formatAge(s.created_at)}</span>
                  {resumeButton(s)}
                </div>
              </div>
            ))}
          </div>
          {/* Desktop: session rows */}
          <div className="hidden sm:block">
            {sessions.map((s) => (
              <div
                key={s.id}
                className="flex items-center px-2.5 py-2 border-b border-[#2a2a44] cursor-pointer gap-2 hover:bg-secondary/50"
                onClick={() => setOverlaySession(s)}
              >
                <span className="font-mono text-[13px] text-foreground font-semibold min-w-[80px]">{truncateId(s.id)}</span>
                <span className="text-[11px] text-muted-foreground">{s.entry_count} entries</span>
                <span className="text-[11px] text-muted-foreground/70 ml-auto">{formatAge(s.created_at)}</span>
                {resumeButton(s)}
              </div>
            ))}
          </div>
        </>
      )}
      <SessionDetailOverlay
        session={overlaySession}
        agentId={selectedAgentId}
        open={overlaySession !== null}
        onClose={() => setOverlaySession(null)}
      />
    </div>
  )
}
