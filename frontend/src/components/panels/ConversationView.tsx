// frontend/src/components/panels/ConversationView.tsx
import { useAtomValue } from 'jotai'
import { useState } from 'react'
import { conversationByAgentAtom } from '@/stores/conversation'
import { isRunningAtom } from '@/stores/connection'
import { Markdown } from '@/components/shared/Markdown'
import { useAutoScroll } from '@/hooks/useAutoScroll'
import { Button } from '@/components/ui/button'
import { ScrollArea } from '@/components/ui/scroll-area'
import type { ConversationEntry } from '@/types'

function ToolDetailModal({
  entry, onClose
}: {
  entry: { toolCall: ConversationEntry & { type: 'ToolCall' }; result?: ConversationEntry & { type: 'ToolResult' } }
  onClose: () => void
}) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="bg-card border border-border rounded-lg p-4 max-w-2xl w-full mx-4 max-h-[80vh] overflow-y-auto"
        onClick={e => e.stopPropagation()}>
        <h3 className="text-lg font-bold mb-2">Tool: {entry.toolCall.toolName}</h3>
        <div className="mb-4">
          <div className="text-xs text-muted-foreground mb-1">Arguments</div>
          <pre className="bg-background p-2 rounded text-xs overflow-x-auto whitespace-pre-wrap">
            {entry.toolCall.fullArguments}
          </pre>
        </div>
        {entry.result && (
          <div>
            <div className="text-xs text-muted-foreground mb-1">
              Result {entry.result.success
                ? <span className="text-emerald-400">OK</span>
                : <span className="text-destructive">ERR</span>}
            </div>
            <Markdown content={entry.result.fullResult} />
          </div>
        )}
        <Button variant="outline" size="sm" className="mt-4" onClick={onClose}>Close</Button>
      </div>
    </div>
  )
}

function TimelineEntry({
  entry, index, entries, isLast: _isLast
}: {
  entry: ConversationEntry; index: number; entries: ConversationEntry[]; isLast: boolean
}) {
  const [detailOpen, setDetailOpen] = useState(false)

  // Find matching ToolResult after a ToolCall
  const toolDetail = entry.type === 'ToolCall' ? (() => {
    const resultEntry = entries.slice(index + 1).find(
      e => e.type === 'ToolResult' && e.toolName === entry.toolName
    )
    return { toolCall: entry, result: resultEntry as (ConversationEntry & { type: 'ToolResult' }) | undefined }
  })() : null

  const dotColor = entry.type === 'UserInput' ? '#80a0ff' :
    entry.type === 'Error' ? '#c04040' : '#888'

  return (
    <div className="flex gap-2">
      {/* Left rail */}
      <div className="flex flex-col items-center w-5 flex-shrink-0 pt-1">
        {entry.type === 'UserInput'
          ? <span className="text-primary text-xs">❯</span>
          : <span className="w-2 h-2 rounded-full" style={{ backgroundColor: dotColor, boxShadow: `0 0 3px ${dotColor}` }} />
        }
        {index < entries.length - 1 && <div className="w-px flex-1 bg-[#333355] mt-1" />}
      </div>

      {/* Content */}
      <div className="flex-1 min-w-0 pb-3">
        {entry.type === 'UserInput' && (
          <div className="text-foreground whitespace-pre-wrap">{entry.text}</div>
        )}
        {entry.type === 'Thinking' && (
          <div className="text-muted-foreground italic text-sm">{entry.content || 'Thinking...'}</div>
        )}
        {entry.type === 'ContentStreaming' && (
          <Markdown content={entry.content} />
        )}
        {entry.type === 'ToolCall' && (
          <div className="flex items-center gap-2 min-w-0 cursor-pointer group" onClick={() => setDetailOpen(true)}>
            <span className="text-yellow-400 text-xs flex-shrink-0">[tool]</span>
            <span className="text-foreground text-sm flex-shrink-0">{entry.toolName}</span>
            <span className="text-muted-foreground text-xs truncate min-w-0">{entry.argPreview}</span>
            <span className="hidden group-hover:inline text-muted-foreground text-xs">more »</span>
          </div>
        )}
        {entry.type === 'ToolResult' && (
          <div className="cursor-pointer" onClick={() => setDetailOpen(true)}>
            <span className={`text-xs px-1 py-0.5 rounded mr-1 ${entry.success ? 'text-emerald-400 bg-emerald-950/30' : 'text-destructive bg-red-950/30'}`}>
              {entry.success ? 'OK' : 'ERR'}
            </span>
            <span className="text-foreground text-sm line-clamp-2">{entry.preview}</span>
          </div>
        )}
        {entry.type === 'AgentAnswer' && <Markdown content={entry.text} />}
        {entry.type === 'Error' && (
          <div className="text-destructive text-sm">{entry.message}</div>
        )}
        {entry.type === 'RunningBanner' && (
          <div className="text-yellow-400 text-xs italic">Agent running (run: {entry.runId.slice(0, 8)}...)</div>
        )}
        {entry.type === 'RunSummary' && (
          <div className="text-muted-foreground text-xs">
            Done | {entry.iterations} iterations | {entry.toolCalls} tool calls | {entry.elapsedMs}ms
          </div>
        )}
        {entry.type === 'EntryCheckpoint' && (
          <div className="text-muted-foreground text-xs italic">Checkpoint: {entry.reason}</div>
        )}
      </div>

      {/* Tool detail modal */}
      {detailOpen && toolDetail && (
        <ToolDetailModal entry={toolDetail} onClose={() => setDetailOpen(false)} />
      )}
    </div>
  )
}

export function ConversationView() {
  const conv = useAtomValue(conversationByAgentAtom)
  const isRunning = useAtomValue(isRunningAtom)
  // Trigger auto-scroll on entry count AND content changes (entries ref always
  // changes on every content_delta because updateConversation recreates the array).
  const { containerRef, handleScroll, scrollToBottom, autoScrollRef } = useAutoScroll(
    [conv.entries.length, conv.entries]
  )

  const entries = conv.entries

  if (entries.length === 0) {
    return (
      <div className="flex-1 flex items-center justify-center text-muted-foreground/70 text-sm">
        Select an agent and start a conversation
      </div>
    )
  }

  return (
    <div className="flex-1 relative min-h-0">
      <ScrollArea className="h-full" ref={containerRef} onScroll={handleScroll}>
        <div className="p-3 sm:p-4">
          {entries.map((entry, i) => (
            <TimelineEntry
              key={i}
              entry={entry}
              index={i}
              entries={entries}
              isLast={i === entries.length - 1}
            />
          ))}
          {isRunning && entries.length > 0 && entries[entries.length - 1].type === 'RunningBanner' && (
            <div className="flex items-center gap-2 text-yellow-400 text-xs">
              <span className="w-2 h-2 rounded-full bg-yellow-400 animate-pulse" />
              Running...
            </div>
          )}
        </div>
      </ScrollArea>
      {/* Scroll-to-bottom button when user has scrolled up */}
      <button
        type="button"
        onClick={scrollToBottom}
        className="absolute bottom-3 right-4 z-10 bg-primary text-primary-foreground rounded-full w-8 h-8 flex items-center justify-center shadow-lg opacity-80 hover:opacity-100 transition-opacity cursor-pointer"
        style={{ display: autoScrollRef.current ? 'none' : 'flex' }}
        aria-label="Scroll to bottom"
      >
        ↓
      </button>
    </div>
  )
}