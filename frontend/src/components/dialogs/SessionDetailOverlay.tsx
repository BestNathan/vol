// frontend/src/components/dialogs/SessionDetailOverlay.tsx
// Session detail modal — renders a session's converted entries as a timeline.
// Port of sessions_panel.rs::SessionDetailOverlay. Owns the session.entries
// fetch (with parse-failure detection); entries are cached per session id so
// re-opening the same session is instant (mirrors the Rust
// `if entries.is_empty() && !loading` guard).
import { useEffect, useRef, useState } from 'react'
import { Dialog, DialogContent, DialogHeader, DialogTitle } from '@/components/ui/dialog'
import { getPanelClient } from '@/lib/panel-client'
import { sessionEntriesToConversation } from '@/lib/session-conversion'
import { ImageGallery } from '@/components/shared/ImageGallery'
import type { RpcMethods } from '@/lib/protocol'
import type { ConversationEntry, SessionListEntry } from '@/types'

/** Cap a tool-result preview to a few lines / a max char count. */
function truncateLines(s: string, maxLines: number, maxChars: number): string {
  let result = s.split('\n').slice(0, maxLines).join('\n')
  if (result.length > maxChars) {
    result = `${result.slice(0, maxChars - 3)}...`
  }
  return result
}

/** Single timeline entry, styled like the Dioxus overlay's per-type cards. */
function EntryView({ entry }: { entry: ConversationEntry }) {
  switch (entry.type) {
    case 'UserInput':
      return (
        <div className="mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#1a2a44] border-l-[3px] border-[#4080ff]">
          <div className="text-[#4080ff] font-bold">&gt;&gt;&gt; </div>
          {entry.text}
          {entry.images && entry.images.length > 0 && <ImageGallery images={entry.images} />}
        </div>
      )
    case 'Thinking':
      return (
        <div className="mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-muted-foreground italic text-[12px] leading-[1.5]">
          {entry.content}
        </div>
      )
    case 'ToolCall':
      return (
        <div className="mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-foreground text-[13px]">
          <span className="text-yellow-400 font-bold">[tool]</span>{' '}
          <span className="font-semibold">{entry.toolName}</span>{' '}
          <span className="text-muted-foreground">{entry.argPreview}</span>
        </div>
      )
    case 'ToolResult': {
      const cls = entry.success
        ? 'mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-emerald-950/30 border-l-[3px] border-emerald-500/50'
        : 'mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-red-950/30 border-l-[3px] border-destructive/50'
      const color = entry.success ? '#40c040' : '#c04040'
      return (
        <div className={cls}>
          <div>
            <span className="font-bold" style={{ color }}>
              [{entry.success ? 'OK' : 'ERR'}]{' '}
            </span>
            <span style={{ color, fontWeight: 'bold' }}>{entry.toolName}</span>
          </div>
          <div className="text-muted-foreground text-[12px] mt-1 pl-1 max-h-[120px] overflow-y-auto font-mono">
            {truncateLines(entry.preview, 6, 90)}
          </div>
        </div>
      )
    }
    case 'EntryCheckpoint': {
      const note = entry.note ? ` (${entry.note})` : ''
      return (
        <div className="mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap bg-[#2a2a20] border-l-[3px] border-[#c0a040] text-foreground/70 text-[12px] italic">
          [Checkpoint] {entry.reason}
          {note}
        </div>
      )
    }
    case 'RunSummary': {
      const iw = entry.iterations === 1 ? 'iteration' : 'iterations'
      const tw = entry.toolCalls === 1 ? 'tool call' : 'tool calls'
      return (
        <div className="mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-emerald-400 font-bold py-1.5">
          Done | {entry.iterations} {iw} | {entry.toolCalls} {tw} | {entry.elapsedMs}ms
        </div>
      )
    }
    case 'RunningBanner':
      return (
        <div className="mb-2 px-3 py-2 rounded-md bg-[#1a2a44] border border-[#3a5a7a] text-sm">
          <span className="text-[#c0d0e0]">&#9679; Running [{entry.runId}]</span>
        </div>
      )
    case 'AgentAnswer':
      return (
        <div className="mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap text-foreground leading-[1.5]">
          {entry.text}
        </div>
      )
    default:
      return (
        <div className="mb-2.5 px-2.5 py-2 rounded-md max-w-full break-words whitespace-pre-wrap">
          Entry
        </div>
      )
  }
}

export function SessionDetailOverlay({
  session,
  agentId,
  open,
  onClose,
}: {
  session: SessionListEntry | null
  agentId: string | null
  open: boolean
  onClose: () => void
}) {
  const [entries, setEntries] = useState<ConversationEntry[]>([])
  const [loading, setLoading] = useState(false)
  const [parseFailure, setParseFailure] = useState(false)
  const loadedIdRef = useRef<string | null>(null)
  const inflightRef = useRef(false)

  // Clear cached entries when a different session is opened.
  useEffect(() => {
    setEntries([])
    setParseFailure(false)
  }, [session?.id])

  // Fetch entries once per session id on first open; re-opening the same
  // session reuses the cached entries without refetching.
  useEffect(() => {
    if (!open || !session) return
    if (loadedIdRef.current === session.id || inflightRef.current) return
    inflightRef.current = true
    setLoading(true)
    setParseFailure(false)
    getPanelClient()
      .call<RpcMethods['session.entries']['result']>('session.entries', {
        session_id: session.id,
        agent_id: agentId ?? undefined,
      })
      .then((res) => {
        const raw = res.entries ?? []
        const converted = sessionEntriesToConversation(raw)
        if (raw.length > 0 && converted.length === 0) setParseFailure(true)
        setEntries(converted)
        loadedIdRef.current = session.id
      })
      .catch((err) => {
        console.error('Failed to load session entries:', err)
        setParseFailure(true)
      })
      .finally(() => {
        inflightRef.current = false
        setLoading(false)
      })
  }, [open, session, agentId])

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => {
        if (!next) onClose()
      }}
    >
      <DialogContent className="sm:max-w-[900px] h-[70vh] max-h-[70vh] flex flex-col gap-0 p-0 overflow-hidden">
        <DialogHeader className="flex flex-row items-center justify-between px-3 py-2 border-b border-[#2a2a44] space-y-0 text-left">
          <DialogTitle className="font-mono text-[13px] text-foreground truncate">
            Session: {session?.id ?? ''}
          </DialogTitle>
        </DialogHeader>
        {loading ? (
          <div className="flex items-center justify-center flex-1 text-muted-foreground/70">
            Loading...
          </div>
        ) : parseFailure && entries.length === 0 ? (
          <div className="flex-1 flex items-center justify-center flex-col text-destructive p-5 text-center">
            <div className="text-[14px] font-semibold mb-2">Failed to parse session entries</div>
            <div className="text-[12px] text-muted-foreground">
              Check browser console (F12) for details
            </div>
          </div>
        ) : entries.length === 0 ? (
          <div className="flex items-center justify-center flex-1 text-muted-foreground/70">
            No entries
          </div>
        ) : (
          <div className="flex-1 overflow-y-auto p-2">
            {entries.map((entry, i) => (
              <EntryView key={i} entry={entry} />
            ))}
          </div>
        )}
      </DialogContent>
    </Dialog>
  )
}
