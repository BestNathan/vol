// frontend/src/components/dialogs/DebugPanel.tsx
// WS message inspector modal: full-screen overlay toggled by the 🐛 button
// in the StatusBar. Lists captured WebSocket messages (inbound/outbound) with
// direction arrows, elapsed timestamps (HH:MM:SS.mmm since first capture),
// method names, and expandable pretty-printed JSON payloads. Port of
// crates/vol-llm-ui/src/web/components/debug_panel.rs.
import { useEffect, useState } from 'react'
import { useAtom } from 'jotai'
import { debugPanelAtom, type DebugMessage } from '@/stores/dialogs'

/** Elapsed time since the first captured message, as HH:MM:SS.mmm. */
export function formatElapsed(ms: number): string {
  const hours = Math.floor(ms / 3_600_000)
  const mins = Math.floor(ms / 60_000) % 60
  const secs = Math.floor(ms / 1000) % 60
  const millis = Math.floor(ms) % 1000
  return `${String(hours).padStart(2, '0')}:${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}.${String(millis).padStart(3, '0')}`
}

/** Pretty-print a JSON payload string; fall back to the raw text. */
export function formatJsonPretty(raw: string): string {
  if (!raw) return ''
  try {
    return JSON.stringify(JSON.parse(raw), null, 2)
  } catch {
    return raw
  }
}

export function DebugPanel() {
  const [panel, setPanel] = useAtom(debugPanelAtom)
  const { open, messages } = panel

  const close = () => setPanel({ open: false, messages })

  // Close on Escape (must be before any early return — hooks order is fixed)
  useEffect(() => {
    if (!open) return
    const handleKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') close()
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [open])

  if (!open) return null

  // Responsive: centered modal on mobile, right-side docked panel on desktop.
  return (
    <div
      className="fixed inset-0 bg-black/50 z-50 flex items-center justify-center p-4 sm:justify-end sm:p-0"
      onClick={close}
    >
      <div
        className="bg-background border border-border rounded-lg flex flex-col shadow-2xl w-[92vw] max-w-[640px] h-[85vh] sm:max-w-none sm:w-[420px] sm:h-full sm:rounded-none sm:border-0 sm:border-l"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="flex items-center justify-between px-4 py-2 border-b border-[#333] shrink-0">
          <div className="flex items-center gap-3">
            <span className="text-foreground font-bold text-sm">Debug Panel</span>
            <div className="flex gap-1">
              <button
                type="button"
                className="px-3 py-1 text-[12px] font-semibold cursor-pointer border-b-2 border-primary text-foreground"
              >
                WS
              </button>
            </div>
          </div>
          <button
            type="button"
            aria-label="Close debug panel"
            onClick={close}
            className="text-muted-foreground hover:text-white text-lg leading-none px-1 cursor-pointer"
          >
            ×
          </button>
        </div>
        <div className="flex-1 overflow-hidden">
          <WsTab messages={messages} />
        </div>
      </div>
    </div>
  )
}

function WsTab({ messages }: { messages: DebugMessage[] }) {
  const [expanded, setExpanded] = useState<number | null>(null)

  return (
    <div className="flex flex-col h-full">
      <div className="flex-1 overflow-y-auto font-mono text-xs">
        {messages.length === 0 ? (
          <div className="flex items-center justify-center h-full text-muted-foreground/70 text-sm">
            No messages yet. Open the panel while the agent is active to capture WS traffic.
          </div>
        ) : (
          messages.map((msg, i) => (
            <WsMessageRow
              key={i}
              index={i}
              message={msg}
              expanded={expanded}
              onToggle={() => setExpanded(expanded === i ? null : i)}
            />
          ))
        )}
      </div>
      <div className="px-3 py-1.5 border-t border-[#333] text-[10px] text-muted-foreground/70 shrink-0 flex items-center justify-between">
        <span>{messages.length} messages</span>
        <span>Recording since page load</span>
      </div>
    </div>
  )
}

function WsMessageRow({
  index,
  message,
  expanded,
  onToggle,
}: {
  index: number
  message: DebugMessage
  expanded: number | null
  onToggle: () => void
}) {
  const isExpanded = expanded === index
  const arrow = message.direction === 'in' ? '←' : '→'
  const arrowColor = message.direction === 'in' ? '#40c040' : '#80a0ff'

  return (
    <div
      className="border-b border-[#222] hover:bg-secondary/50 cursor-pointer"
      onClick={onToggle}
    >
      <div className="flex items-center gap-2 px-3 py-1.5">
        <span className="text-muted-foreground/60 w-[100px] shrink-0">{formatElapsed(message.elapsedMs)}</span>
        <span style={{ color: arrowColor, fontWeight: 'bold' }}>{arrow}</span>
        <span className="text-foreground/80 font-bold truncate">{message.method}</span>
      </div>
      {isExpanded && (
        <div className="px-3 pb-2 pl-[120px]">
          <pre className="text-muted-foreground text-[11px] bg-[#111128] rounded p-2 whitespace-pre-wrap break-all max-h-[300px] overflow-y-auto">
            {formatJsonPretty(message.payload)}
          </pre>
        </div>
      )}
    </div>
  )
}
